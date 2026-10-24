//! Thread that owns the HID handle: runs commands, listens to the mouse and applies the rules.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use hidapi::HidApi;
use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

use compx_proto::info::Battery;
use compx_proto::ipc::{MouseState, Status};
use compx_proto::procs;
use compx_proto::rules::{Rule, RuleMode};
use compx_proto::sensor::Sensor;
use compx_proto::settings::{MouseSettings, SettingChange};
use compx_proto::{Error, Event, Mouse};

pub const WM_WORKER: u32 = WM_APP + 1;

const RULES_CHECK: Duration = Duration::from_secs(3);
const BATTERY_CHECK: Duration = Duration::from_secs(60);
const ASLEEP_RETRY: Duration = Duration::from_secs(2);
/// Right after a switch the mouse may not answer yet; read the profile again this much later.
const RECHECK_AFTER_SWITCH: Duration = Duration::from_secs(1);
/// Device arrival is event driven (`DevicesChanged`); this is only a fallback.
const RECONNECT_FALLBACK: Duration = Duration::from_secs(30);
const EVENT_WAIT_MS: i32 = 200;

#[derive(Clone, Debug, Default)]
pub struct AutoSwitch {
    pub enabled: bool,
    pub mode: RuleMode,
    pub rules: Vec<Rule>,
    pub default_profile: u8,
}

pub enum Command {
    SetProfile(u8),
    AutoSwitch(AutoSwitch),
    /// Lowercase exe of the new foreground window (focused mode).
    Foreground(Option<String>),
    DevicesChanged,
    ReadSettings,
    Write(SettingChange),
    /// Applies a saved copy of settings to the active profile.
    Restore(MouseSettings),
    Quit,
}

pub enum WorkerEvent {
    Status(Status),
    /// `Err` carries the message for the panel.
    Settings(Result<MouseSettings, String>),
    Restored(Result<(), String>),
}

pub fn spawn(rx: Receiver<Command>, tx: Sender<WorkerEvent>, ui_thread: u32, auto: AutoSwitch) {
    thread::spawn(move || {
        let Ok(api) = HidApi::new() else { return };
        Worker {
            api,
            rx,
            tx,
            ui_thread,
            auto,
            last_target: None,
            last_check: Instant::now() - RULES_CHECK,
            status: Status::DISCONNECTED,
            battery: None,
            last_battery: Instant::now(),
            recheck: None,
        }
        .run()
    });
}

struct Worker {
    api: HidApi,
    rx: Receiver<Command>,
    tx: Sender<WorkerEvent>,
    ui_thread: u32,
    auto: AutoSwitch,
    /// Profile requested by the rules at the last check (`None` = no rule matched).
    last_target: Option<u8>,
    last_check: Instant,
    status: Status,
    battery: Option<Battery>,
    last_battery: Instant,
    /// When to read the profile again after an unconfirmed switch.
    recheck: Option<Instant>,
}

/// Lift-off is the one setting stored raw, and raw values differ between sensors (0.7 mm is 3 on a
/// PAW3950 and 1 on a PAW3950 Ultra), so a backup is restored by height. `None` when the saved
/// height is unknown or the current sensor has no such height.
fn lift_off_raw(saved: &MouseSettings, raw: u8, sensor: Option<Sensor>) -> Option<u8> {
    let height = saved.lift_off_options.iter().find(|o| o.raw == raw)?.tenths_mm;
    sensor?.lift_off_options().iter().find(|l| l.tenths_mm == height).map(|l| l.raw)
}

/// Why `serve` stopped.
enum Exit {
    Quit,
    /// The device went away.
    Lost,
    /// ATK HUB started: let go of the device until it closes.
    Paused,
}

impl Worker {
    fn send(&self, event: WorkerEvent) {
        let _ = self.tx.send(event);
        unsafe { PostThreadMessageW(self.ui_thread, WM_WORKER, 0, 0) };
    }

    /// Always sends a Ready status, even if unchanged: it re-syncs the check mark the menu flips on click.
    fn publish(&mut self, status: Status) {
        if status == self.status && status.mouse != MouseState::Ready {
            return;
        }
        self.status = status.clone();
        self.send(WorkerEvent::Status(status));
    }

    fn run(mut self) {
        loop {
            if procs::scan(&[]).atk_hub {
                self.publish(Status::PAUSED);
                if self.wait(RULES_CHECK) {
                    return;
                }
                continue;
            }
            match Mouse::open(&mut self.api) {
                Ok(mut mouse) => {
                    self.battery = mouse.battery().ok();
                    self.last_battery = Instant::now();
                    self.refresh(&mut mouse);
                    match self.serve(&mut mouse) {
                        Err(Exit::Lost) => self.publish(Status::DISCONNECTED),
                        Err(Exit::Paused) => {}
                        Ok(()) | Err(Exit::Quit) => return,
                    }
                }
                Err(_) => {
                    self.publish(Status::DISCONNECTED);
                    if self.wait(RECONNECT_FALLBACK) {
                        return;
                    }
                }
            }
        }
    }

    /// Without a mouse: keeps taking commands until a device change or `timeout`.
    /// Returns `true` when asked to quit.
    fn wait(&mut self, timeout: Duration) -> bool {
        let until = Instant::now() + timeout;
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            match self.rx.recv_timeout(left) {
                Ok(Command::Quit) | Err(RecvTimeoutError::Disconnected) => return true,
                Ok(Command::DevicesChanged) => return false,
                Ok(Command::AutoSwitch(a)) => self.set_auto(a),
                Ok(Command::ReadSettings | Command::Write(_)) => {
                    self.send(WorkerEvent::Settings(Err(Error::NotFound.to_string())))
                }
                Ok(Command::Restore(_)) => self.send(WorkerEvent::Restored(Err(Error::NotFound.to_string()))),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => break,
            }
        }
        false
    }

    fn set_auto(&mut self, auto: AutoSwitch) {
        self.auto = auto;
        self.last_target = None;
        self.last_check = Instant::now() - RULES_CHECK;
    }

    fn refresh(&mut self, mouse: &mut Mouse) {
        // While asleep, one quick try: a full retry cycle would block the worker for seconds.
        let profile = if self.status.mouse == MouseState::Ready { mouse.profile() } else { mouse.profile_quick() }.ok();
        if profile.is_some() && mouse.identity.is_none() {
            let _ = mouse.identify();
        }
        self.publish(Status {
            mouse: if profile.is_some() { MouseState::Ready } else { MouseState::Asleep },
            profile,
            battery: profile.and(self.battery),
            device: mouse.identity.as_ref().map(|i| i.info()),
        });
    }

    fn read_settings(&mut self, mouse: &Mouse) -> Result<(), Exit> {
        match mouse.settings() {
            Ok(s) => self.send(WorkerEvent::Settings(Ok(s))),
            Err(Error::Hid(_)) => return Err(Exit::Lost),
            Err(e) => self.send(WorkerEvent::Settings(Err(e.to_string()))),
        }
        Ok(())
    }

    /// Writes, then sends the real state read back from the mouse (even when the write failed).
    fn write(&mut self, mouse: &Mouse, change: SettingChange) -> Result<(), Exit> {
        match mouse.apply(change) {
            Err(Error::Hid(_)) => return Err(Exit::Lost),
            Err(e) => self.send(WorkerEvent::Settings(Err(e.to_string()))),
            Ok(()) => {}
        }
        self.read_settings(mouse)
    }

    fn restore(&mut self, mouse: &Mouse, saved: MouseSettings) -> Result<(), Exit> {
        let mut changes = Vec::new();
        changes.extend(saved.polling_hz.map(|hz| SettingChange::Polling { hz }));
        if !saved.stages.is_empty() {
            changes.push(SettingChange::StageCount { count: saved.stage_count });
            for (i, s) in saved.stages.iter().enumerate() {
                changes.push(SettingChange::StageDpi { index: i as u8, dpi: s.x });
            }
        }
        changes.push(SettingChange::CurrentStage { index: saved.current_stage });
        let mut failed = 0;
        if let Some(raw) = saved.lift_off {
            match lift_off_raw(&saved, raw, mouse.identity.as_ref().and_then(|i| i.sensor())) {
                Some(raw) => changes.push(SettingChange::LiftOff { raw }),
                None => failed += 1,
            }
        }
        changes.extend(saved.debounce_ms.map(|ms| SettingChange::Debounce { ms }));
        changes.extend(saved.motion_sync.map(|on| SettingChange::MotionSync { on }));
        changes.extend(saved.angle_snap.map(|on| SettingChange::AngleSnap { on }));
        changes.extend(saved.ripple.map(|on| SettingChange::Ripple { on }));
        changes.extend(saved.sensor_mode.map(|mode| SettingChange::SensorMode { mode }));

        for change in changes {
            match mouse.apply(change) {
                Ok(()) => {}
                Err(Error::Hid(_)) => return Err(Exit::Lost),
                Err(_) => failed += 1,
            }
        }
        self.send(WorkerEvent::Restored(if failed == 0 { Ok(()) } else { Err(format!("{failed}")) }));
        self.read_settings(mouse)
    }

    fn set_profile(&mut self, mouse: &mut Mouse, index: u8) -> Result<(), Exit> {
        match mouse.set_profile(index) {
            Ok(()) => {
                let status = Status { mouse: MouseState::Ready, profile: Some(index), ..self.status.clone() };
                self.publish(status);
            }
            // Sent, but not confirmed yet: do not report the mouse as asleep over one missed reply.
            Err(Error::Timeout) => self.recheck = Some(Instant::now() + RECHECK_AFTER_SWITCH),
            Err(Error::Hid(_)) => return Err(Exit::Lost),
            Err(_) => self.refresh(mouse),
        }
        Ok(())
    }

    fn apply_target(&mut self, mouse: &mut Mouse, target: Option<u8>) -> Result<(), Exit> {
        if target == self.last_target {
            return Ok(());
        }
        self.last_target = target;
        self.set_profile(mouse, target.unwrap_or(self.auto.default_profile))
    }

    fn serve(&mut self, mouse: &mut Mouse) -> Result<(), Exit> {
        loop {
            loop {
                match self.rx.try_recv() {
                    Ok(Command::SetProfile(p)) => self.set_profile(mouse, p)?,
                    Ok(Command::AutoSwitch(a)) => self.set_auto(a),
                    Ok(Command::Foreground(exe)) => {
                        if self.auto.enabled && self.auto.mode == RuleMode::Focused {
                            let target = exe.and_then(|e| self.auto.rules.iter().find(|r| r.exe == e)).map(|r| r.profile);
                            self.apply_target(mouse, target)?;
                        }
                    }
                    Ok(Command::DevicesChanged) => {}
                    Ok(Command::ReadSettings) => self.read_settings(mouse)?,
                    Ok(Command::Write(change)) => self.write(mouse, change)?,
                    Ok(Command::Restore(saved)) => self.restore(mouse, saved)?,
                    Ok(Command::Quit) | Err(TryRecvError::Disconnected) => return Err(Exit::Quit),
                    Err(TryRecvError::Empty) => break,
                }
            }

            if self.last_check.elapsed() >= RULES_CHECK {
                self.last_check = Instant::now();
                let running = self.auto.enabled && self.auto.mode == RuleMode::Running;
                let scan = procs::scan(if running { &self.auto.rules } else { &[] });
                if scan.atk_hub {
                    return Err(Exit::Paused);
                }
                if running {
                    self.apply_target(mouse, scan.target)?;
                }
            }

            if self.recheck.is_some_and(|at| Instant::now() >= at) {
                self.recheck = None;
                self.refresh(mouse);
            }

            let interval = if self.status.mouse == MouseState::Ready { BATTERY_CHECK } else { ASLEEP_RETRY };
            if self.last_battery.elapsed() >= interval {
                self.last_battery = Instant::now();
                match mouse.battery() {
                    Ok(b) => self.battery = Some(b),
                    Err(Error::Hid(_)) => return Err(Exit::Lost),
                    Err(_) => {}
                }
                self.refresh(mouse);
            }

            match mouse.poll_event(EVENT_WAIT_MS) {
                Ok(Some(Event::ProfileChanged)) => self.refresh(mouse),
                Ok(None) => {}
                Err(_) => return Err(Exit::Lost),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved_on(sensor: Sensor) -> MouseSettings {
        MouseSettings {
            lift_off_options: sensor.lift_off_options().iter().map(|&l| l.into()).collect(),
            ..MouseSettings::default()
        }
    }

    #[test]
    fn lift_off_is_restored_by_height() {
        let ultra = saved_on(Sensor::Paw3950Ultra);
        assert_eq!(lift_off_raw(&ultra, 3, Some(Sensor::Paw3950Ultra)), Some(3));
        // 0.7 mm and 1 mm exist on both sensors, under different raw values.
        assert_eq!(lift_off_raw(&ultra, 1, Some(Sensor::Paw3950)), Some(3));
        assert_eq!(lift_off_raw(&ultra, 4, Some(Sensor::Paw3950)), Some(1));
        assert_eq!(lift_off_raw(&saved_on(Sensor::Paw3950), 3, Some(Sensor::Paw3950Ultra)), Some(1));
    }

    #[test]
    fn lift_off_without_a_matching_height_is_not_restored() {
        let ultra = saved_on(Sensor::Paw3950Ultra);
        assert_eq!(lift_off_raw(&ultra, 2, Some(Sensor::Paw3950)), None);
        assert_eq!(lift_off_raw(&ultra, 1, Some(Sensor::Paw3311)), None);
        assert_eq!(lift_off_raw(&ultra, 1, None), None);
        assert_eq!(lift_off_raw(&MouseSettings::default(), 1, Some(Sensor::Paw3950Ultra)), None);
    }
}
