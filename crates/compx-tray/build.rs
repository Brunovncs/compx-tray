// tray-icon's menus import TaskDialogIndirect, which only exists in comctl32 v6: without this
// manifest Windows loads v5 and the exe does not start ("entry point not found").
// Per-monitor DPI awareness keeps the OSD sharp.
use embed_manifest::manifest::DpiAwareness;

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest = embed_manifest::new_manifest("compx-tray").dpi_awareness(DpiAwareness::PerMonitorV2);
        embed_manifest::embed_manifest(manifest).expect("could not embed the manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
