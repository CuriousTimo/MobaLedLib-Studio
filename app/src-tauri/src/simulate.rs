//! The LED simulator in the app: makes the `Config[]` bytes of a sheet the way the Excel tool's simulator does
//! (`M39_Simulator.bas`: compile `Configuration.cpp` with the generated header and `CONFIG_ONLY` using the
//! Arduino AVR compiler, then cut the `.MLLLedConfig` section out with `avr-objcopy`), and runs them in
//! [`mll_sim::Simulator`] on its own thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::arduino;

/// One input channel of the sheet, from the `#define INCH_…` lines of the header.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Input {
    pub channel: u8,
    /// The define without `INCH_`, e.g. `DCC_1_ONOFF`
    pub name: String,
    /// The comment of the line (the description of the effect), "" if none
    pub comment: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Started {
    pub num_leds: u16,
    pub inputs: Vec<Input>,
    /// Per row of the sheet: its first input channel, if it has an address
    pub row_inputs: Vec<Option<u8>>,
}

/// The channel of each row's first input `#define` (from [`mll_core::codegen::Generated::row_inputs`]).
pub fn row_channels(inputs: &[Input], row_inputs: &[Option<String>]) -> Vec<Option<u8>> {
    row_inputs
        .iter()
        .map(|d| {
            let name = d.as_deref()?.strip_prefix("INCH_")?;
            inputs.iter().find(|i| i.name == name).map(|i| i.channel)
        })
        .collect()
}

/// The input channels named in the header.
pub fn inputs(header: &str) -> Vec<Input> {
    let mut v = Vec::new();
    for line in header.lines() {
        let Some(rest) = line.trim_start().strip_prefix("#define INCH_") else { continue };
        let (code, comment) = match rest.split_once("//") {
            Some((c, m)) => (c, m.trim()),
            None => (rest, ""),
        };
        let mut parts = code.split_whitespace();
        let (Some(name), Some(num)) = (parts.next(), parts.next()) else { continue };
        if let Ok(channel) = num.parse::<u8>() {
            v.push(Input { channel, name: name.to_string(), comment: comment.to_string() });
        }
    }
    v
}

/// The sort key of a version folder name such as `7.3.0-atmel3.6.1-arduino7`: its numbers.
fn version_key(name: &str) -> Vec<u32> {
    name.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect()
}

/// The newest version folder in `dir`.
fn newest(dir: &Path) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    v.retain(|p| p.is_dir());
    v.sort_by_key(|p| version_key(&p.file_name().unwrap_or_default().to_string_lossy()));
    v.pop()
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// The `Configuration.cpp` of the Excel tool (extras/Configuration), with the header and `MP3.h` next to it,
/// and the RAM size (`Config_RAM` of `MobaLedLib_Create`) and the LED count put in a section of their own.
const CONFIGURATION_CPP: &str = r#"#define CONFIG_ONLY

#include <Arduino.h>
#include "MP3.h"

#include "LEDs_AutoProg.h"

const uint16_t MLL_Sim_Info[2] __attribute__ ((section (".MLLSimInfo"))) = { __COUNTER__/2, NUM_LEDS };

int main()
{
#if defined(USE_EXT_ADDR)
    int ex = (int)&Ext_Addr[0];
#else
    int ex = 0;
#endif
#if defined(USE_LED_TO_VAR)
    int ldv = (int)&LED2Var_Tab[0];
#else
    int ldv = 0;
#endif
  return (int)&Config[0]+ex+ldv+(int)&MLL_Sim_Info[0];
}
"#;

pub struct SimConfig {
    pub config: Vec<u8>,
    pub ram_size: u16,
    pub num_leds: u16,
}

fn run(cmd: &mut std::process::Command, what: &str) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{what} failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
    }
}

/// Compiles the header with the Arduino AVR compiler, with the options of the Excel tool's simulator.
pub fn config(app: &AppHandle, header: &str) -> Result<SimConfig, String> {
    let cli = arduino::find_cli().ok_or("arduino-cli was not found; the simulator compiles with it")?;
    let sketch = arduino::installed_sketch(&cli).ok_or(
        "The MobaLedLib Arduino library is not installed. Install the library bundle from https://github.com/CuriousTimo/MobaLedLib-Libraries",
    )?;
    let lib_src = sketch.parent().and_then(|p| p.parent()).ok_or("MobaLedLib folder not found")?.join("src");
    let packages = arduino::data_dir(&cli).ok_or("the Arduino data folder was not found")?.join("packages/arduino");
    let not_installed =
        || "The Arduino AVR boards are not installed: arduino-cli core install arduino:avr@1.8.6".to_string();
    let bin = newest(&packages.join("tools/avr-gcc")).ok_or_else(not_installed)?.join("bin");
    let platform = newest(&packages.join("hardware/avr")).ok_or_else(not_installed)?;

    let work = app.path().app_data_dir().map_err(|e| e.to_string())?.join("simulator");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    std::fs::write(work.join("LEDs_AutoProg.h"), header).map_err(|e| e.to_string())?;
    std::fs::copy(sketch.join("MP3.h"), work.join("MP3.h")).map_err(|e| format!("MP3.h: {e}"))?;
    std::fs::write(work.join("Configuration.cpp"), CONFIGURATION_CPP).map_err(|e| e.to_string())?;

    let inc = |p: PathBuf| format!("-I{}", p.display());
    run(
        arduino::cli_command(&bin.join(exe("avr-g++")))
            .current_dir(&work)
            .args("-c -g -Os -Wall -std=gnu++11 -fpermissive -fno-exceptions -ffunction-sections -fdata-sections -fno-threadsafe-statics -Wno-error=narrowing -MMD -flto -mmcu=atmega328p -DF_CPU=16000000L -DARDUINO=10813 -DARDUINO_AVR_NANO -DARDUINO_ARCH_AVR".split(' '))
            .arg(inc(platform.join("cores/arduino")))
            .arg(inc(platform.join("variants/eightanaloginputs")))
            .arg(inc(lib_src))
            .args(["Configuration.cpp", "-o", "Configuration.cpp.o"]),
        "Compiling the configuration",
    )?;
    run(
        arduino::cli_command(&bin.join(exe("avr-gcc")))
            .current_dir(&work)
            .args("-Wall -Os -g -flto -fuse-linker-plugin -Wl,--gc-sections -mmcu=atmega328p -o Configuration.elf Configuration.cpp.o -lm".split(' ')),
        "Linking the configuration",
    )?;
    let section = |name: &str, file: &str| -> Result<Vec<u8>, String> {
        run(
            arduino::cli_command(&bin.join(exe("avr-objcopy"))).current_dir(&work).args([
                "-O",
                "binary",
                "-j",
                name,
                "Configuration.elf",
                file,
            ]),
            "avr-objcopy",
        )?;
        std::fs::read(work.join(file)).map_err(|e| e.to_string())
    };
    let config = section(".MLLLedConfig", "LEDConfig.bin")?;
    let info = section(".MLLSimInfo", "SimInfo.bin")?;
    if info.len() < 4 {
        return Err("the configuration has no RAM size".into());
    }
    Ok(SimConfig {
        config,
        ram_size: u16::from_le_bytes([info[0], info[1]]),
        num_leds: u16::from_le_bytes([info[2], info[3]]),
    })
}

enum Cmd {
    Input(u8, bool),
    Stop,
}

/// The running simulation: its command channel and thread (the engine has global state: one at a time).
static RUNNING: Mutex<Option<(Sender<Cmd>, std::thread::JoinHandle<()>)>> = Mutex::new(None);

/// Every 10 ms one `Update()`, as the Arduino's loop does it all the time; every 40 ms the LEDs go to the
/// window as a "sim-leds" event (r, g, b for each LED).
fn run_loop(app: AppHandle, cfg: SimConfig, rx: Receiver<Cmd>) {
    let mut sim = match mll_sim::Simulator::new(&cfg.config, cfg.ram_size, cfg.num_leds) {
        Ok(s) => s,
        Err(e) => {
            let _ = app.emit("sim-error", e.to_string());
            return;
        }
    };
    let start = Instant::now();
    let mut last_emit = Duration::ZERO;
    loop {
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(Cmd::Input(ch, on)) => sim.set_input(ch, on),
            Ok(Cmd::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        let now = start.elapsed();
        sim.set_time(now.as_millis() as u32);
        sim.update();
        if now - last_emit >= Duration::from_millis(40) {
            last_emit = now;
            let leds: Vec<u8> = sim.leds().into_iter().flatten().collect();
            let _ = app.emit("sim-leds", leds);
        }
    }
}

pub fn start(app: &AppHandle, header: &str, row_inputs: &[Option<String>]) -> Result<Started, String> {
    stop();
    let cfg = config(app, header)?;
    let inputs = inputs(header);
    let row_inputs = row_channels(&inputs, row_inputs);
    let started = Started { num_leds: cfg.num_leds, inputs, row_inputs };
    let (tx, rx) = mpsc::channel();
    let app2 = app.clone();
    let thread = std::thread::spawn(move || run_loop(app2, cfg, rx));
    *RUNNING.lock().unwrap() = Some((tx, thread));
    Ok(started)
}

pub fn set_input(channel: u8, on: bool) {
    if let Some((tx, _)) = RUNNING.lock().unwrap().as_ref() {
        let _ = tx.send(Cmd::Input(channel, on));
    }
}

pub fn stop() {
    let running = RUNNING.lock().unwrap().take();
    if let Some((tx, thread)) = running {
        let _ = tx.send(Cmd::Stop);
        let _ = thread.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_channels_by_define() {
        let inputs = inputs("#define INCH_DCC_1_ONOFF  0 // A\n#define INCH_DCC_10_RED  1 // B\n");
        let rows = [Some("INCH_DCC_10_RED".to_string()), None, Some("INCH_DCC_99_RED".to_string())];
        assert_eq!(row_channels(&inputs, &rows), [Some(1), None, None]);
    }

    #[test]
    fn inputs_from_the_header() {
        let h = "#define INCH_DCC_1_ONOFF        0      // Station building\n#define INCH_DCC_10_GREEN       4      //     \"\n#define NUM_LEDS 25\n";
        assert_eq!(
            inputs(h),
            vec![
                Input { channel: 0, name: "DCC_1_ONOFF".into(), comment: "Station building".into() },
                Input { channel: 4, name: "DCC_10_GREEN".into(), comment: "\"".into() },
            ]
        );
    }

    #[test]
    fn newest_version_by_numbers() {
        let mut v = ["7.3.0-atmel3.6.1-arduino5", "7.3.0-atmel3.6.1-arduino7", "5.4.0-atmel3.6.1-arduino2"];
        v.sort_by_key(|s| version_key(s));
        assert_eq!(v[2], "7.3.0-atmel3.6.1-arduino7");
    }
}
