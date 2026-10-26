//! Compiling and uploading through `arduino-cli`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

/// Name of the sketch the header belongs to; the folder must have the same name.
const SKETCH: &str = "LEDs_AutoProg";

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// Path of arduino-cli, None if it wasn't found
    pub cli: Option<String>,
    pub version: String,
    /// The LEDs_AutoProg sketch of the installed MobaLedLib, None if not installed
    pub sketch: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Port {
    pub address: String,
    pub label: String,
    /// Detected board name, "" if unknown (clones usually are)
    pub board: String,
    pub fqbn: String,
}

fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = which::which("arduino-cli") {
        v.push(p);
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        v.push("/opt/homebrew/bin/arduino-cli".into());
        v.push("/usr/local/bin/arduino-cli".into());
        v.push("/Applications/Arduino IDE.app/Contents/Resources/app/lib/backend/resources/arduino-cli".into());
    } else if cfg!(target_os = "windows") {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            v.push(
                PathBuf::from(local).join("Programs/Arduino IDE/resources/app/lib/backend/resources/arduino-cli.exe"),
            );
        }
        v.push("C:/Program Files/Arduino CLI/arduino-cli.exe".into());
    } else {
        v.push("/usr/bin/arduino-cli".into());
        v.push("/usr/local/bin/arduino-cli".into());
        if let Some(h) = &home {
            v.push(h.join("bin/arduino-cli"));
            v.push(h.join(".local/bin/arduino-cli"));
        }
    }
    v
}

pub fn find_cli() -> Option<PathBuf> {
    candidates().into_iter().find(|p| p.is_file())
}

fn run_json(cli: &Path, args: &[&str]) -> Result<Value, String> {
    let out = Command::new(cli).args(args).output().map_err(|e| format!("arduino-cli: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("arduino-cli output: {e}"))
}

/// The Arduino "sketchbook" folder, which contains `libraries/`.
fn user_dir(cli: &Path) -> Option<PathBuf> {
    let v = run_json(cli, &["config", "dump", "--format", "json"]).ok()?;
    let dirs = v.get("config").unwrap_or(&v).get("directories")?;
    dirs.get("user").and_then(|u| u.as_str()).map(PathBuf::from)
}

fn installed_sketch(cli: &Path) -> Option<PathBuf> {
    let lib = user_dir(cli)?.join("libraries");
    // The library folder is "MobaLedLib", the sketch folder is spelled "LEDs_Autoprog"
    for libname in ["MobaLedLib", "Mobaledlib", "mobaledlib"] {
        for dir in ["LEDs_Autoprog", SKETCH] {
            let p = lib.join(libname).join("extras").join(dir);
            if p.join(format!("{SKETCH}.ino")).is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn status() -> Status {
    let Some(cli) = find_cli() else {
        return Status { cli: None, version: String::new(), sketch: None };
    };
    let version = run_json(&cli, &["version", "--format", "json"])
        .ok()
        .and_then(|v| v.get("VersionString").and_then(|s| s.as_str()).map(|s| s.to_string()))
        .unwrap_or_default();
    let sketch = installed_sketch(&cli).map(|p| p.display().to_string());
    Status { cli: Some(cli.display().to_string()), version, sketch }
}

pub fn ports() -> Result<Vec<Port>, String> {
    let cli = find_cli().ok_or("arduino-cli was not found")?;
    let v = run_json(&cli, &["board", "list", "--format", "json"])?;
    // arduino-cli >= 1.0 wraps the list in "detected_ports"
    let list = v.get("detected_ports").unwrap_or(&v).as_array().cloned().unwrap_or_default();
    let mut ports = Vec::new();
    for entry in list {
        let port = entry.get("port").unwrap_or(&entry);
        if port.get("protocol").and_then(|p| p.as_str()) != Some("serial") {
            continue;
        }
        let address = port.get("address").and_then(|a| a.as_str()).unwrap_or("").to_string();
        let label = port.get("label").and_then(|a| a.as_str()).unwrap_or(&address).to_string();
        let board = entry.get("matching_boards").and_then(|b| b.as_array()).and_then(|b| b.first());
        ports.push(Port {
            address,
            label,
            board: board.and_then(|b| b.get("name")).and_then(|s| s.as_str()).unwrap_or("").to_string(),
            fqbn: board.and_then(|b| b.get("fqbn")).and_then(|s| s.as_str()).unwrap_or("").to_string(),
        });
    }
    Ok(ports)
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let target = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// Copy the sketch into the app's work folder and write the generated header next to it.
fn prepare_sketch(app: &AppHandle, sketch_src: &Path, header: &str) -> Result<PathBuf, String> {
    let work = app.path().app_data_dir().map_err(|e| e.to_string())?.join("build").join(SKETCH);
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(|e| format!("cleaning {}: {e}", work.display()))?;
    }
    copy_dir(sketch_src, &work).map_err(|e| format!("copying the sketch: {e}"))?;
    std::fs::write(work.join(format!("{SKETCH}.h")), header).map_err(|e| format!("writing the header: {e}"))?;
    Ok(work)
}

fn log(app: &AppHandle, line: impl Into<String>) {
    let _ = app.emit("build-log", line.into());
}

/// Compile the sketch with the header and, if a port is given, upload it.
/// Output lines are sent to the window as "build-log" events.
pub fn build(app: &AppHandle, header: &str, fqbn: &str, port: Option<&str>) -> Result<(), String> {
    let cli = find_cli().ok_or("arduino-cli was not found. Install it from https://arduino.github.io/arduino-cli/")?;
    let sketch_src = installed_sketch(&cli).ok_or(
        "The MobaLedLib Arduino library is not installed. Install it with: arduino-cli lib install MobaLedLib",
    )?;
    let work = prepare_sketch(app, &sketch_src, header)?;
    log(app, format!("› Sketch: {}", work.display()));

    let mut args: Vec<String> = vec!["compile".into(), "--fqbn".into(), fqbn.into()];
    if let Some(p) = port {
        args.push("--upload".into());
        args.push("-p".into());
        args.push(p.into());
    }
    args.push(work.display().to_string());
    log(app, format!("› arduino-cli {}", args.join(" ")));

    let mut child = Command::new(&cli)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("starting arduino-cli: {e}"))?;
    let stderr = child.stderr.take().unwrap();
    let app2 = app.clone();
    let err_thread = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            log(&app2, line);
        }
    });
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            log(app, line);
        }
    }
    let _ = err_thread.join();
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("arduino-cli failed ({status})"))
    }
}
