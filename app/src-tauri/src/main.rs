// Prevents an additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod arduino;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use mll_core::catalog::{self, FilledMacro};
use mll_core::codegen::{self, Diagnostic, GenOptions};
use mll_core::leds;
use mll_core::model::{Platform, Protocol};
use mll_core::{pgf, Catalog, Layout};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(Catalog::bundled)
}

fn is_pgf(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("MLL_pgf"))
}

#[derive(Serialize)]
struct Opened {
    layout: Layout,
    /// Path to save to; None for imported files, which must be saved as JSON first
    path: Option<String>,
    imported: bool,
}

#[tauri::command]
fn get_catalog() -> &'static Catalog {
    catalog()
}

#[tauri::command]
fn new_layout(name: String, protocol: Protocol, platform: Platform) -> Layout {
    Layout::new(&name, protocol, platform)
}

fn read_layout(path: &Path) -> Result<Opened, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if is_pgf(path) {
        let layout = pgf::import(&bytes, &name).map_err(|e| e.to_string())?;
        Ok(Opened { layout, path: None, imported: true })
    } else {
        let text = String::from_utf8(bytes).map_err(|_| "the file is not valid UTF-8".to_string())?;
        let layout = Layout::from_json(&text).map_err(|e| e.to_string())?;
        Ok(Opened { layout, path: Some(path.display().to_string()), imported: false })
    }
}

/// Show the open dialog and load the chosen layout (.json or .MLL_pgf).
#[tauri::command]
async fn open_layout(app: AppHandle, import: bool) -> Result<Option<Opened>, String> {
    let dialog = app.dialog().file();
    let dialog = if import {
        dialog.add_filter("Program Generator file", &["MLL_pgf"])
    } else {
        dialog.add_filter("MobaLedLib Studio layout", &["json"])
    };
    let Some(file) = dialog.blocking_pick_file() else { return Ok(None) };
    let path = file.into_path().map_err(|e| e.to_string())?;
    read_layout(&path).map(Some)
}

#[tauri::command]
fn open_layout_path(path: String) -> Result<Opened, String> {
    read_layout(Path::new(&path))
}

fn pick_save(app: &AppHandle, name: &str, label: &str, ext: &str) -> Result<Option<PathBuf>, String> {
    let file =
        app.dialog().file().add_filter(label, &[ext]).set_file_name(format!("{name}.{ext}")).blocking_save_file();
    match file {
        None => Ok(None),
        Some(f) => f.into_path().map(Some).map_err(|e| e.to_string()),
    }
}

/// Save as JSON. Without a path (or with `save_as`) the save dialog is shown.
/// Returns the path that was written, None if the dialog was cancelled.
#[tauri::command]
async fn save_layout(app: AppHandle, layout: Layout, path: Option<String>) -> Result<Option<String>, String> {
    let path = match path {
        Some(p) => PathBuf::from(p),
        None => match pick_save(&app, &layout.name, "MobaLedLib Studio layout", "json")? {
            Some(p) => p,
            None => return Ok(None),
        },
    };
    std::fs::write(&path, layout.to_json() + "\n").map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
async fn export_pgf(app: AppHandle, layout: Layout) -> Result<Option<String>, String> {
    let Some(path) = pick_save(&app, &layout.name, "Program Generator file", "MLL_pgf")? else { return Ok(None) };
    std::fs::write(&path, pgf::export(&layout)).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

#[derive(Serialize)]
struct Analysis {
    /// Start LED of each row (None = inactive)
    led_numbers: Vec<Option<i64>>,
    leds_per_channel: Vec<i64>,
    total_leds: i64,
    /// Problems found by a trial generation of the header
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
    input_channels: i64,
}

fn gen_options() -> GenOptions {
    GenOptions { lib_version: catalog().lib_version.clone(), ..GenOptions::default() }
}

/// LED numbers and validation for one sheet. Called after every edit.
#[tauri::command]
fn analyze(layout: Layout, sheet: usize) -> Result<Analysis, String> {
    let sh = layout.sheets.get(sheet).ok_or("sheet not found")?;
    let numbers = leds::compute(sh);
    let (errors, warnings, input_channels) = match codegen::generate(&layout, sheet, catalog(), &gen_options()) {
        Ok(g) => (Vec::new(), g.warnings, g.input_channels),
        Err(e) => (e.errors, Vec::new(), 0),
    };
    Ok(Analysis {
        total_leds: numbers.total(),
        led_numbers: numbers.rows,
        leds_per_channel: numbers.per_channel.to_vec(),
        errors,
        warnings,
        input_channels,
    })
}

#[tauri::command]
fn generate_header(layout: Layout, sheet: usize) -> Result<String, Vec<Diagnostic>> {
    codegen::generate(&layout, sheet, catalog(), &gen_options()).map(|g| g.header).map_err(|e| e.errors)
}

#[tauri::command]
fn fill_macro(name: String, values: BTreeMap<String, String>, extra: Vec<String>) -> Result<FilledMacro, String> {
    let def = catalog().macros.iter().find(|m| m.name == name).ok_or_else(|| format!("unknown macro '{name}'"))?;
    catalog::fill_macro(def, &values, &extra).map_err(|e| e.to_string())
}

#[tauri::command]
async fn arduino_status() -> arduino::Status {
    arduino::status()
}

#[tauri::command]
async fn list_ports() -> Result<Vec<arduino::Port>, String> {
    arduino::ports()
}

/// Generate the header, compile and (with a port) upload. Progress arrives as "build-log" events.
#[tauri::command]
async fn build_and_upload(app: AppHandle, layout: Layout, sheet: usize, port: Option<String>) -> Result<(), String> {
    let header = codegen::generate(&layout, sheet, catalog(), &gen_options()).map_err(|e| e.to_string())?.header;
    let fqbn = layout.board.fqbn.clone();
    tauri::async_runtime::spawn_blocking(move || arduino::build(&app, &header, &fqbn, port.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_catalog,
            new_layout,
            open_layout,
            open_layout_path,
            save_layout,
            export_pgf,
            analyze,
            generate_header,
            fill_macro,
            arduino_status,
            list_ports,
            build_and_upload,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MobaLedLib Studio");
}
