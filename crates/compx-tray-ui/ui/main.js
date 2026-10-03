const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const appWindow = window.__TAURI__.window.getCurrentWindow();

const $ = (id) => document.getElementById(id);

// --- i18n -------------------------------------------------------------------

const STRINGS = {
  en: {
    searching: "Looking for the mouse…", theme: "Toggle light/dark theme",
    tabMouse: "Mouse", tabAuto: "Automation", tabSystem: "System",
    profiles: "Profiles", profile: "Profile", sensor: "Profile settings", rename: "Rename",
    dpiHint: "Click a stage to use it; edit the value and press Enter",
    addStage: "Add stage", removeStage: "Remove last stage", stage: "Stage",
    polling: "Polling rate", pollingHint: "Reports per second",
    liftOff: "Lift-off distance", liftOffHint: "Height where the sensor stops tracking",
    debounce: "Click debounce", debounceHint: "Lower is faster, but may double-click",
    sensorMode: "Sensor mode", modeBasic: "Basic", modeCompetitive: "Competitive", modeCompetitiveMax: "Competitive max",
    motionSync: "Motion sync", motionSyncHint: "Aligns sensor reads with USB polls",
    angleSnap: "Angle snapping", angleSnapHint: "Straightens nearly straight lines",
    ripple: "Ripple control", rippleHint: "Smooths jitter at high DPI",
    retry: "Try again",
    autoSwitch: "Switch profile by program", autoSwitchHint: "Uses the rules below",
    ruleMode: "A rule applies while the program is", modeRunning: "Running", modeFocused: "Focused",
    modeRunningHint: "Open anywhere, even minimized (checked every 3 s)",
    modeFocusedHint: "In the foreground window (switches instantly)",
    defaultProfile: "Default profile", defaultProfileHint: "When no rule applies",
    rules: "Rules", rulesEmpty: "No rules yet. Add a game or app below.",
    rulePlaceholder: "Program (e.g. cs2.exe)", add: "Add", remove: "Remove",
    rulesHint: "When several programs match, the rule higher in the list wins.",
    autostart: "Start with Windows", autostartHint: "Only the tray icon, without this window",
    osd: "Show profile changes", osdHint: "A small toast above the taskbar",
    hotkeys: "Keyboard shortcuts", hotkeysHint: "Ctrl+Alt+Shift+1…4 switch profiles",
    backups: "Backups", backupNow: "Save this profile", backupHint: "DPI, polling, lift-off and sensor options",
    save: "Save", restore: "Restore", device: "Device",
    model: "Model", sensorName: "Sensor", connection: "Connection", firmware: "Mouse firmware", receiverFirmware: "Receiver firmware",
    footer: "Closing this window keeps COMPX Tray running in the tray.",
    trayOffline: "COMPX Tray is not running in the tray.", startTray: "Start COMPX Tray",
    disconnected: "Mouse not connected. Plug in the receiver or cable.",
    asleep: "The mouse is not responding. Move it to wake it up.",
    unknownSensor: "Unknown model: DPI is read-only.",
    backupSaved: "Backup saved", backupFailed: "Could not save the backup",
    restoredOk: "Profile restored", restoredFail: "Some settings could not be restored",
    writeFailed: "Could not save", charging: "charging",
    paused: "ATK HUB is open, so COMPX Tray is paused and won't interfere (e.g. with a firmware update).",
    updateMouse: "Mouse firmware update: {0} → {1}", updateReceiver: "Receiver firmware update: {0} → {1}",
    openHub: "Open ATK HUB", hubNotFound: "ATK HUB is not installed", upToDate: "up to date",
    conn: { dongle1k: "1K receiver", dongle2k: "2K receiver", dongle4k: "4K receiver", dongle8k: "8K receiver", wired1k: "Cable (1K)", wired2k: "Cable (2K)", wired4k: "Cable (4K)", wired8k: "Cable (8K)", nearlink: "NearLink" },
  },
  pt: {
    searching: "Procurando o mouse…", theme: "Alternar tema claro/escuro",
    tabMouse: "Mouse", tabAuto: "Automação", tabSystem: "Sistema",
    profiles: "Perfis", profile: "Perfil", sensor: "Configurações do perfil", rename: "Renomear",
    dpiHint: "Clique num estágio para usá-lo; edite o valor e aperte Enter",
    addStage: "Adicionar estágio", removeStage: "Remover último estágio", stage: "Estágio",
    polling: "Polling rate", pollingHint: "Relatórios por segundo",
    liftOff: "Lift-off", liftOffHint: "Altura em que o sensor para de ler",
    debounce: "Debounce do clique", debounceHint: "Menor é mais rápido, mas pode dar clique duplo",
    sensorMode: "Modo do sensor", modeBasic: "Básico", modeCompetitive: "Competitivo", modeCompetitiveMax: "Competitivo máx.",
    motionSync: "Motion sync", motionSyncHint: "Alinha a leitura do sensor com o USB",
    angleSnap: "Angle snapping", angleSnapHint: "Endireita linhas quase retas",
    ripple: "Ripple control", rippleHint: "Suaviza tremidas em DPI alto",
    retry: "Tentar de novo",
    autoSwitch: "Trocar perfil pelo programa", autoSwitchHint: "Usa as regras abaixo",
    ruleMode: "A regra vale enquanto o programa está", modeRunning: "Aberto", modeFocused: "Em foco",
    modeRunningHint: "Aberto em qualquer lugar, até minimizado (checa a cada 3 s)",
    modeFocusedHint: "Na janela em primeiro plano (troca na hora)",
    defaultProfile: "Perfil padrão", defaultProfileHint: "Quando nenhuma regra vale",
    rules: "Regras", rulesEmpty: "Nenhuma regra ainda. Adicione um jogo ou app abaixo.",
    rulePlaceholder: "Programa (ex.: cs2.exe)", add: "Adicionar", remove: "Remover",
    rulesHint: "Se vários programas baterem, vale a regra mais acima na lista.",
    autostart: "Iniciar com o Windows", autostartHint: "Fica só o ícone na bandeja, sem esta janela",
    osd: "Mostrar troca de perfil", osdHint: "Um aviso pequeno acima da barra de tarefas",
    hotkeys: "Atalhos de teclado", hotkeysHint: "Ctrl+Alt+Shift+1…4 trocam o perfil",
    backups: "Backups", backupNow: "Salvar este perfil", backupHint: "DPI, polling, lift-off e opções do sensor",
    save: "Salvar", restore: "Restaurar", device: "Dispositivo",
    model: "Modelo", sensorName: "Sensor", connection: "Conexão", firmware: "Firmware do mouse", receiverFirmware: "Firmware do receptor",
    footer: "Fechar esta janela não encerra o COMPX Tray: ele continua na bandeja.",
    trayOffline: "O COMPX Tray não está rodando na bandeja.", startTray: "Iniciar COMPX Tray",
    disconnected: "Mouse desconectado. Conecte o receptor ou o cabo.",
    asleep: "O mouse não está respondendo. Mexa nele para acordar.",
    unknownSensor: "Modelo desconhecido: DPI só leitura.",
    backupSaved: "Backup salvo", backupFailed: "Não foi possível salvar o backup",
    restoredOk: "Perfil restaurado", restoredFail: "Algumas configurações não foram restauradas",
    writeFailed: "Não foi possível gravar", charging: "carregando",
    paused: "O ATK HUB está aberto; o COMPX Tray pausou para não interferir (por exemplo, numa atualização de firmware).",
    updateMouse: "Atualização de firmware do mouse: {0} → {1}", updateReceiver: "Atualização de firmware do receptor: {0} → {1}",
    openHub: "Abrir ATK HUB", hubNotFound: "O ATK HUB não está instalado", upToDate: "atualizado",
    conn: { dongle1k: "Receptor 1K", dongle2k: "Receptor 2K", dongle4k: "Receptor 4K", dongle8k: "Receptor 8K", wired1k: "Cabo (1K)", wired2k: "Cabo (2K)", wired4k: "Cabo (4K)", wired8k: "Cabo (8K)", nearlink: "NearLink" },
  },
};
const LANG = navigator.language.toLowerCase().startsWith("pt") ? "pt" : "en";
const T = STRINGS[LANG];
document.documentElement.lang = LANG === "pt" ? "pt-BR" : "en";
for (const el of document.querySelectorAll("[data-i18n]")) el.textContent = T[el.dataset.i18n];
for (const el of document.querySelectorAll("[data-i18n-title]")) el.title = el.ariaLabel = T[el.dataset.i18nTitle];
for (const el of document.querySelectorAll("[data-i18n-placeholder]")) el.placeholder = T[el.dataset.i18nPlaceholder];

const fmt = new Intl.NumberFormat(document.documentElement.lang);
const hz = (v) => (v >= 1000 ? `${v / 1000}K` : `${v}`);
const mm = (tenths) => `${(tenths / 10).toLocaleString(document.documentElement.lang, { minimumFractionDigits: 1 })} mm`;

// --- helpers ----------------------------------------------------------------

function h(tag, props = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "class") el.className = v;
    else if (v !== undefined && v !== null && v !== false) el[k] = v;
  }
  el.append(...children.flat().filter((c) => c != null));
  return el;
}

// Same order as PROFILE_COLORS in the tray.
const PROFILE_COLORS = ["var(--mint)", "var(--lavender)", "var(--peach)", "var(--sky)"];
const MAX_PROFILES = 4;
const DEBOUNCE_MS = [0, 1, 2, 4, 8, 15, 20];

const state = { connected: false, status: null, prefs: null, settings: null, backups: [] };

const send = (msg) => invoke("send", { msg }).catch((e) => toast(e === "tray-offline" ? T.trayOffline : String(e), false));
const ready = () => state.connected && state.status?.mouse === "ready";
// The tray's "asleep" lags behind a mouse that was just moved, so profiles stay clickable.
const reachable = () => state.connected && ["ready", "asleep"].includes(state.status?.mouse);
const profileName = (i) => state.prefs?.profileNames?.[i]?.trim() || `${T.profile} ${i + 1}`;

let toastTimer;
function toast(text, ok = true) {
  const el = $("toast");
  el.textContent = text;
  el.classList.toggle("bad", !ok);
  el.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (el.hidden = true), 2600);
}

// --- theme & tabs -------------------------------------------------------------

const darkQuery = matchMedia("(prefers-color-scheme: dark)");
function savedTheme() {
  try { return localStorage.getItem("theme"); } catch { return null; }
}
function applyTheme() {
  const saved = savedTheme();
  const root = document.documentElement;
  if (saved) root.dataset.theme = saved; else delete root.dataset.theme;
  const dark = saved ? saved === "dark" : darkQuery.matches;
  root.toggleAttribute("data-dark", dark);
  appWindow.setTheme(saved ?? null).catch(() => {});
}
$("theme").addEventListener("click", () => {
  const next = document.documentElement.hasAttribute("data-dark") ? "light" : "dark";
  // Picking what the system already uses goes back to following the system.
  try {
    if ((next === "dark") === darkQuery.matches) localStorage.removeItem("theme");
    else localStorage.setItem("theme", next);
  } catch {}
  applyTheme();
});
darkQuery.addEventListener("change", applyTheme);
applyTheme();

function showTab(name) {
  for (const b of document.querySelectorAll("[data-tab]")) b.setAttribute("aria-selected", b.dataset.tab === name);
  for (const p of document.querySelectorAll("[data-panel]")) p.hidden = p.dataset.panel !== name;
  if (name === "system") renderDeviceInfo();
}
for (const b of document.querySelectorAll("[data-tab]")) b.addEventListener("click", () => showTab(b.dataset.tab));

// --- header & banner ----------------------------------------------------------

function renderHeader() {
  const s = state.status;
  const d = s?.device;
  $("device").textContent = d ? [d.model, T.conn[d.connection]].filter(Boolean).join(" · ") : T.searching;

  const b = ready() ? s.battery : null;
  $("battery").hidden = !b;
  if (b) {
    $("battery-text").textContent = b.percent;
    $("battery-ring").style.strokeDashoffset = `${94.25 * (1 - b.percent / 100)}`;
    $("battery").classList.toggle("low", b.percent <= 20 && !b.charging);
    $("battery").classList.toggle("charging", b.charging);
    $("battery").title = `${b.percent}%${b.charging ? ` · ${T.charging}` : ""}`;
  }

  const accent = s?.profile != null ? PROFILE_COLORS[s.profile] : PROFILE_COLORS[0];
  document.documentElement.style.setProperty("--accent", accent);

  let text = null, action = null;
  if (!state.connected) { text = T.trayOffline; action = T.startTray; }
  else if (s?.mouse === "disconnected") text = T.disconnected;
  else if (s?.mouse === "asleep") text = T.asleep;
  else if (s?.mouse === "paused") text = T.paused;
  $("banner").hidden = !text;
  $("banner").classList.toggle("off", !state.connected || s?.mouse === "disconnected" || s?.mouse === "paused");
  $("banner-text").textContent = text ?? "";
  $("banner-action").hidden = !action;
  $("banner-action").textContent = action ?? "";
}
$("banner-action").addEventListener("click", () => invoke("start_tray"));

// --- profiles -------------------------------------------------------------------

let renaming = null;

function renderProfiles() {
  if (renaming != null && $("profiles").contains(document.activeElement)) return;
  const on = reachable();
  const count = state.status?.device?.profiles ?? MAX_PROFILES;
  $("profiles").replaceChildren(
    ...Array.from({ length: count }, (_, i) => {
      const active = ready() && state.status.profile === i;
      const card = h("button", {
        class: `profile${active ? " active" : ""}`,
        disabled: !on && renaming !== i,
        onclick: () => {
          if (renaming === i || active || !on) return;
          card.classList.add("pending");
          send({ type: "setProfile", index: i });
        },
      });
      card.style.setProperty("--accent", PROFILE_COLORS[i]);
      const name = renaming === i
        ? h("input", {
            value: state.prefs?.profileNames?.[i] ?? "",
            placeholder: `${T.profile} ${i + 1}`,
            maxLength: 32,
            onclick: (e) => e.stopPropagation(),
            onkeydown: (e) => {
              if (e.key === "Enter") e.target.blur();
              if (e.key === "Escape") { renaming = null; renderProfiles(); }
            },
            onblur: (e) => {
              if (renaming !== i) return;
              renaming = null;
              const names = [...(state.prefs.profileNames ?? [])];
              names[i] = e.target.value.trim();
              setPrefs({ profileNames: names });
            },
          })
        : h("span", { class: "name", textContent: profileName(i) });
      const hotkey = state.prefs?.hotkeys ? ` · Ctrl+Alt+Shift+${i + 1}` : "";
      card.append(
        h("span", { class: "dot" }),
        name,
        h("span", { class: "sub", textContent: `${T.profile} ${i + 1}${hotkey}` }),
        h("span", {
          class: "rename", title: T.rename, role: "button", tabIndex: 0,
          onclick: (e) => {
            e.stopPropagation();
            renaming = i;
            renderProfiles();
            const input = $("profiles").querySelector("input");
            input?.focus();
            input?.select();
          },
          innerHTML: '<svg viewBox="0 0 24 24"><path d="M4 20h4L19 9l-4-4L4 16v4zM14 6l4 4"/></svg>',
        }),
      );
      return card;
    }),
  );
}

// --- profile settings -------------------------------------------------------------

let settingsFor = null;

function requestSettings() {
  $("settings").classList.add("busy");
  send({ type: "readSettings" });
}

function loadSettingsIfNeeded() {
  if (!ready()) {
    settingsFor = null;
    state.settings = null;
    renderSettings();
    return;
  }
  if (state.status.profile !== settingsFor) {
    settingsFor = state.status.profile;
    requestSettings();
  }
}

function write(change) {
  $("settings").classList.add("busy");
  send({ type: "write", change });
}

function snapDpi(v, st) {
  if (!Number.isFinite(v) || !st.dpiSteps.length) return null;
  const max = st.dpiSteps.at(-1)[0];
  v = Math.min(max, Math.max(st.dpiMin, Math.round(v)));
  const [, step] = st.dpiSteps.find(([upTo]) => v <= upTo);
  const snapped = Math.round(v / step) * step;
  return Math.min(max, Math.max(st.dpiMin, snapped));
}

function segmented(el, options, current, onPick) {
  el.replaceChildren(
    ...options.map(([value, label, disabled]) =>
      h("button", { textContent: label, disabled, ariaPressed: String(value === current), onclick: () => value !== current && onPick(value) }),
    ),
  );
}

function renderSettings() {
  const st = state.settings;
  const show = ready() && st;
  $("settings").hidden = !show;
  $("settings-profile").textContent = ready() ? `· ${profileName(state.status.profile)}` : "";
  if (!show) return;
  $("settings").classList.remove("busy");
  $("settings-error").hidden = true;

  const editable = st.dpiSteps.length > 0;
  $("stage-add").disabled = !editable || st.stageCount >= 8;
  $("stage-remove").disabled = st.stageCount <= 1;
  if (!$("stages").contains(document.activeElement)) {
    $("stages").replaceChildren(
      ...(st.stages.length ? st.stages : Array.from({ length: st.stageCount }, () => null)).map((stage, i) => {
        const current = i === st.currentStage;
        const input = h("input", {
          type: "text", inputMode: "numeric", value: stage && stage.x ? stage.x : "—", disabled: !editable,
          ariaLabel: `DPI ${T.stage} ${i + 1}`,
          onclick: (e) => e.stopPropagation(),
          onfocus: (e) => e.target.select(),
          onkeydown: (e) => {
            if (e.key === "Enter") e.target.blur();
            if (e.key === "Escape") { e.target.value = stage?.x || "—"; e.target.blur(); }
          },
          onchange: (e) => {
            const dpi = snapDpi(Number(e.target.value.replace(/\D/g, "")), st);
            e.target.classList.toggle("invalid", dpi == null);
            if (dpi == null) return;
            e.target.value = dpi;
            if (dpi !== stage?.x || stage.x !== stage.y) write({ kind: "stageDpi", index: i, dpi });
          },
        });
        return h("li", {
          class: `stage${current ? " current" : ""}`,
          onclick: () => !current && write({ kind: "currentStage", index: i }),
        },
          h("span", { class: "n", textContent: `${T.stage} ${i + 1}` }),
          input,
          stage && stage.x !== stage.y ? h("span", { class: "xy", textContent: `X ${stage.x} · Y ${stage.y}` }) : null,
        );
      }),
    );
  }
  $("dpi-note").hidden = editable;

  segmented($("polling"), st.pollingOptions.map((v) => [v, hz(v)]), st.pollingHz, (hz) => write({ kind: "polling", hz }));

  $("lift-off-row").hidden = !st.liftOffOptions.length;
  $("lift-off").replaceChildren(...st.liftOffOptions.map((o) => new Option(mm(o.tenthsMm), o.raw, false, o.raw === st.liftOff)));

  const debounce = DEBOUNCE_MS.includes(st.debounceMs) ? DEBOUNCE_MS : [...DEBOUNCE_MS, st.debounceMs].filter((v) => v != null);
  $("debounce").replaceChildren(...debounce.map((v) => new Option(`${v} ms`, v, false, v === st.debounceMs)));

  $("sensor-mode-row").hidden = st.sensorMode == null;
  const modes = [[0, T.modeBasic], [1, T.modeCompetitive], [2, T.modeCompetitiveMax]].slice(0, st.maxSensorMode + 1);
  segmented($("sensor-mode"), modes, st.sensorMode, (mode) => write({ kind: "sensorMode", mode }));

  for (const [id, key] of [["motion-sync", "motionSync"], ["angle-snap", "angleSnap"], ["ripple", "ripple"]]) {
    $(id).checked = !!st[key];
    $(id).disabled = st[key] == null;
  }
}

$("lift-off").addEventListener("change", (e) => write({ kind: "liftOff", raw: Number(e.target.value) }));
$("debounce").addEventListener("change", (e) => write({ kind: "debounce", ms: Number(e.target.value) }));
$("motion-sync").addEventListener("change", (e) => write({ kind: "motionSync", on: e.target.checked }));
$("angle-snap").addEventListener("change", (e) => write({ kind: "angleSnap", on: e.target.checked }));
$("ripple").addEventListener("change", (e) => write({ kind: "ripple", on: e.target.checked }));
$("stage-add").addEventListener("click", () => write({ kind: "stageCount", count: state.settings.stageCount + 1 }));
$("stage-remove").addEventListener("click", () => write({ kind: "stageCount", count: state.settings.stageCount - 1 }));
$("settings-retry").addEventListener("click", requestSettings);

function showSettingsError(message) {
  $("settings").classList.remove("busy");
  $("settings-error").hidden = false;
  $("settings-error-text").textContent = message;
}

// --- automation -----------------------------------------------------------------

function setPrefs(patch) {
  state.prefs = { ...state.prefs, ...patch };
  renderPrefs();
  send({ type: "setPrefs", prefs: state.prefs });
}

function profileOptions(select, selected) {
  select.replaceChildren(
    ...Array.from({ length: MAX_PROFILES }, (_, i) => new Option(profileName(i), i, false, i === selected)),
  );
}

function renderPrefs() {
  const p = state.prefs;
  const off = !state.connected || !p;
  for (const id of ["auto-switch", "autostart", "osd", "hotkeys"]) $(id).disabled = off;
  for (const el of $("add-rule").elements) el.disabled = off;
  $("default-profile").disabled = off;
  if (!p) return;

  $("auto-switch").checked = p.autoSwitch;
  $("autostart").checked = p.autostart;
  $("osd").checked = p.osd;
  $("hotkeys").checked = p.hotkeys;
  segmented($("rule-mode"), [["running", T.modeRunning], ["focused", T.modeFocused]], p.ruleMode, (ruleMode) => setPrefs({ ruleMode }));
  $("rule-mode-hint").textContent = p.ruleMode === "focused" ? T.modeFocusedHint : T.modeRunningHint;
  profileOptions($("default-profile"), p.defaultProfile);
  const addProfile = $("rule-profile").value === "" ? (p.defaultProfile === 0 ? 1 : 0) : Number($("rule-profile").value);
  profileOptions($("rule-profile"), addProfile);

  $("rules-empty").hidden = p.rules.length > 0;
  $("rules").replaceChildren(
    ...p.rules.map((rule, i) => {
      const select = h("select", {
        ariaLabel: `${T.profile} ${rule.exe}`, disabled: off,
        onchange: () => setPrefs({ rules: p.rules.map((r, j) => (j === i ? { ...r, profile: Number(select.value) } : r)) }),
      });
      profileOptions(select, rule.profile);
      const dot = h("span", { class: "dot" });
      dot.style.background = PROFILE_COLORS[rule.profile];
      return h("li", {},
        h("span", { class: "exe" }, dot, rule.exe.replace(/\.exe$/, ""), h("small", { textContent: rule.exe })),
        select,
        h("button", { class: "remove", textContent: "×", title: T.remove, onclick: () => setPrefs({ rules: p.rules.filter((_, j) => j !== i) }) }),
      );
    }),
  );
  renderProfiles();
}

$("auto-switch").addEventListener("change", (e) => setPrefs({ autoSwitch: e.target.checked }));
$("autostart").addEventListener("change", (e) => setPrefs({ autostart: e.target.checked }));
$("osd").addEventListener("change", (e) => setPrefs({ osd: e.target.checked }));
$("hotkeys").addEventListener("change", (e) => setPrefs({ hotkeys: e.target.checked }));
$("default-profile").addEventListener("change", (e) => setPrefs({ defaultProfile: Number(e.target.value) }));

$("rule-exe").addEventListener("focus", async () => {
  const programs = await invoke("running_programs");
  $("programs").replaceChildren(...programs.map((p) => new Option(p)));
});
$("add-rule").addEventListener("submit", (e) => {
  e.preventDefault();
  let exe = $("rule-exe").value.trim().toLowerCase();
  if (!exe || !state.prefs || /[|/\\]/.test(exe)) return;
  if (!exe.endsWith(".exe")) exe += ".exe";
  const profile = Number($("rule-profile").value);
  setPrefs({ rules: [...state.prefs.rules.filter((r) => r.exe !== exe), { exe, profile }] });
  $("rule-exe").value = "";
});

// --- system ---------------------------------------------------------------------

function renderBackups() {
  const dateFmt = new Intl.DateTimeFormat(document.documentElement.lang, { dateStyle: "medium", timeStyle: "short" });
  $("backup").disabled = !ready();
  $("backups").replaceChildren(
    ...state.backups.map((b) =>
      h("li", {},
        h("span", { class: "when", textContent: dateFmt.format(new Date(b.created * 1000)) },
          h("small", { textContent: `${profileName(b.profile)} · ${b.model}` })),
        h("button", { class: "btn small", textContent: T.restore, disabled: !ready(), onclick: () => send({ type: "restore", name: b.name }) }),
        h("button", { class: "remove", textContent: "×", title: T.remove, onclick: () => send({ type: "deleteBackup", name: b.name }) }),
      ),
    ),
  );
}
$("backup").addEventListener("click", () => send({ type: "backup" }));

function renderDeviceInfo() {
  const d = state.status?.device;
  const version = (current, check) =>
    current && (check?.update ? `${current} → ${check.latest}` : check ? `${current} · ${T.upToDate}` : current);
  const rows = d
    ? [[T.model, d.model], [T.sensorName, d.sensor], [T.connection, T.conn[d.connection]],
       [T.firmware, version(d.firmware, state.firmware?.mouse)],
       [T.receiverFirmware, d.receiverFirmware !== "0.00" && version(d.receiverFirmware, state.firmware?.receiver)]]
    : [[T.model, "—"]];
  $("device-info").replaceChildren(
    ...rows.filter(([, v]) => v).flatMap(([k, v]) => [h("dt", { textContent: k }), h("dd", { textContent: v, title: v })]),
  );
}

// --- firmware updates -------------------------------------------------------------

// Same service and comparison as ATK HUB: dot-separated numbers, update only if strictly newer.
const latest = {};
function latestVersion(url) {
  latest[url] ??= fetch(url).then((r) => r.json()).then((j) => j?.data?.version ?? null).catch(() => null);
  return latest[url];
}
function isNewer(candidate, current) {
  if (!candidate || !current) return false;
  const a = String(candidate).split(".").map(Number), b = String(current).split(".").map(Number);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const d = (a[i] || 0) - (b[i] || 0);
    if (d) return d > 0;
  }
  return false;
}

async function checkFirmware() {
  const d = state.status?.device;
  if (!d) return;
  const urls = d.firmwareUrls ?? {};
  const [mouse, receiver] = await Promise.all([
    urls.mouse ? latestVersion(urls.mouse) : null,
    urls.receiver && d.receiverFirmware !== "0.00" ? latestVersion(urls.receiver) : null,
  ]);
  // Status objects are replaced on every update; drop the result only if the device changed.
  if (JSON.stringify(state.status?.device ?? null) !== JSON.stringify(d)) return;
  state.firmware = {
    mouse: mouse && { latest: mouse, update: isNewer(mouse, d.firmware) },
    receiver: receiver && { latest: receiver, update: isNewer(receiver, d.receiverFirmware) },
  };
  renderFirmware();
}

function renderFirmware() {
  const d = state.status?.device, f = state.firmware;
  const lines = [];
  if (d && f?.mouse?.update) lines.push(T.updateMouse.replace("{0}", d.firmware).replace("{1}", f.mouse.latest));
  if (d && f?.receiver?.update) lines.push(T.updateReceiver.replace("{0}", d.receiverFirmware).replace("{1}", f.receiver.latest));
  $("update").hidden = !lines.length || state.status?.mouse === "paused";
  $("update-text").textContent = lines.join(" · ");
  renderDeviceInfo();
}
$("update-action").addEventListener("click", () =>
  invoke("open_atk_hub").catch((e) => toast(e === "atk-hub-not-found" ? T.hubNotFound : String(e), false)),
);

// --- messages from the tray -------------------------------------------------------

function handle(msg) {
  switch (msg.type) {
    case "status": {
      const before = JSON.stringify(state.status?.device ?? null);
      state.status = msg.status;
      if (JSON.stringify(msg.status.device ?? null) !== before) {
        state.firmware = null;
        checkFirmware();
      }
      renderFirmware();
      renderHeader();
      renderProfiles();
      renderBackups();
      renderDeviceInfo();
      loadSettingsIfNeeded();
      break;
    }
    case "prefs":
      state.prefs = msg.prefs;
      renderPrefs();
      renderSettings();
      break;
    case "settings":
      state.settings = msg.settings;
      renderSettings();
      break;
    case "settingsError":
      showSettingsError(`${T.writeFailed}: ${msg.message}`);
      break;
    case "backups":
      state.backups = msg.backups;
      renderBackups();
      break;
    case "notice": {
      const key = msg.message === "restored" ? (msg.ok ? "restoredOk" : "restoredFail") : msg.message;
      toast(T[key] ?? msg.message, msg.ok);
      break;
    }
  }
}

function setConnected(connected) {
  state.connected = connected;
  if (!connected) {
    state.status = null;
    state.prefs = null;
    settingsFor = null;
  }
  renderHeader();
  renderProfiles();
  renderPrefs();
  renderSettings();
  renderBackups();
}

listen("tray", (e) => handle(e.payload));
listen("link", (e) => setConnected(e.payload));
invoke("initial_state").then(({ connected, messages }) => {
  setConnected(connected);
  messages.forEach(handle);
});
