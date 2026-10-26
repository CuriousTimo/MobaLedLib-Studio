# MobaLedLib Studio

**A cross-platform desktop app (Windows, macOS, Linux) that replaces the Excel-based Program Generator of the [MobaLedLib](https://github.com/Hardi-St/MobaLedLib).**

> [!WARNING]
> **Beta, completely untested.** Version `0.1.0-beta.1` has not been tried on real hardware, and the
> generated `LEDs_AutoProg.h` has not yet been compared with the output of the Excel tool. Do not rely on
> it for your layout yet. Test reports are very welcome (see [Help wanted](#help-wanted-testing)).

> [!NOTE]
> This is an independent community project. It is **not** made or endorsed by the MobaLedLib authors.

MobaLedLib is an Arduino library that drives WS2811/WS2812 LEDs, servos and sound modules on model railways.
Until now its effects were configured in `Prog_Generator_MobaLedLib.xlsm`, which needs desktop Excel on
Windows with macros enabled. MobaLedLib Studio does the same job as a native app written in Rust
([Tauri](https://tauri.app)) and stores layouts as plain JSON files.

## Reference version

The behavior is ported from the VBA code and data sheets of:

| | |
|---|---|
| Excel workbook | `extras/Prog_Generator_MobaLedLib.xlsm`, **Program Generator 3.5.0** |
| MobaLedLib release | **3.5.0** |
| Upstream commit | [`a39a493`](https://github.com/Hardi-St/MobaLedLib/commit/a39a493d2177a70fae142c321b02b50072fd80f1) (2025-11-07, "Merge changes from beta 3.4.0D and change to release 3.5.0") |
| Workbook SHA-256 | `e419613c97e4419827298646bc9de536cd8e4088d6deb6bc8f0a0c8d1da82c59` |

The effect catalog in [`data/catalog.json`](data/catalog.json) (285 effects, 169 parameter descriptions,
3 platforms, texts in 7 languages) was extracted from that workbook's hidden sheets `Lib_Macros`,
`Par_Description` and `Platform_Parameters` with `mll extract-catalog`.

## Features

### Implemented in 0.1.0-beta.1

**Layouts and files**
- Layouts saved as readable JSON (`*.json`), with a format id and version for future migrations
- Several sheets per layout, each with its own command system (DCC, Selectrix, Märklin CAN, LocoNet)
- Import of Excel `.MLL_pgf` files (format V1.0 and V2.0, Windows-1252 or UTF-8, multi-line cells,
  input types in all 7 languages of the Excel tool)
- Export to `.MLL_pgf` V2.0, so a layout can go back into the Excel tool
- Recent files on the start screen, "unsaved changes" indicator
- Native menu bar with keyboard shortcuts, native dialogs, open a file by dropping it onto the window
- Asks to save unsaved changes before closing, quitting or opening another layout

**Editor**
- Effect list with on/off switch, trigger (address and input type), effect, output and plug columns
- Grouping by the "Filter" column, text filter
- Inspector to edit description, address, input type, start value, group, distributor/plug and LED channel
- Expert fields for the raw macro text, LEDs, InCnt and Loc InCh
- Move, duplicate and delete rows
- Effect gallery with all 285 library effects, categories, search, descriptions in English, German,
  Dutch, French, Italian, Spanish or Danish (all but English and German are machine translations from the Excel tool)
- Parameter forms generated from `Par_Description` (lists, `Cx` channel selection, defaults, value ranges, hints)
- Room and light editor for `House()` and `GasLights()`
- Servos and sound modules are shown as such, not as LEDs
- LED bus overview of channel 0

**Code generation** (port of `M06_Write_Header.bas`)
- Automatic LED numbering per LED channel, including single WS2811 channels (`C1-2`, `^`), several lines per
  cell and `// Set_LEDNr()` (port of `Update_Start_LedNr`)
- `LEDs_AutoProg.h` with `NUM_LEDS`, `LEDS_PER_CHANNEL`, the `Ext_Addr[]` table, `INCH_…` defines,
  `Set_Start_Values()`, the EEPROM `Store_Values[]` table and counters, `SETUP_FASTLED()` and the configuration array
- DCC address offset, Selectrix channel/bit addresses, duplicate address detection, Red/Green button pairs
- `DayAndNightTimer()`, `#LocInCh`, `// Define Input()` aliases, `House()` timing settings
- Platform limits from `Platform_Parameters` (LED pins, maximum LEDs, supported protocols)
- Live validation: problems are shown on the row they belong to
- View and copy the generated header

**Build and upload**
- Compile and upload with the official `arduino-cli` (found automatically, including the copy inside Arduino IDE 2)
- Serial port detection, board presets (Nano, Nano with old bootloader, Uno, ESP32, Pico)
- Live compiler output

**Command line tool `mll`**
- `mll generate` creates the header from a layout or `.MLL_pgf` file
- `mll convert` converts between JSON and `.MLL_pgf`
- `mll extract-catalog` rebuilds the effect catalog from a new Excel workbook

### Not supported yet

Rows that use one of these features are reported as an error instead of producing a wrong header.

| Feature of the Excel tool | Status |
|---|---|
| Switches and buttons on the main board and keyboards (`SwitchA`…`SwitchD`, CD4017 keyboards) | missing |
| Variables and return messages (`RM …`) in the address column | missing |
| Effects that set variables (`DstVar`): MonoFlop, flip-flops, Counter, Random, RandMux, Schedule, PushButton… | missing |
| `Mainboard_LED()` | missing |
| Including other sheets (`include()`) | missing |
| Multiplexer dialog and `Multiplexer…` macros | missing |
| `LED_to_Var()` | missing |
| Serial sound channels (`S` in the LEDs column, `SOUND_CHANNEL_DEFINITON`) | missing |
| DMX512 output, virtual LED channels, `Set_LED_OutpPinLst()` and other `Set_…_InpLst()` pin lists | missing |
| `#define ENABLE_STORE_STATUS` in the macro column | missing |
| `Logic()` in the effect dialog (it can be typed into the expert fields) | missing |
| ESP32 extensions (`EX.…` macros) | missing |
| Color table dialog (`Set_ColTab`) and the Pattern Configurator | missing |
| Uploading the DCC / Selectrix / LocoNet receiver program to the second Arduino | missing |
| LED simulator | missing |
| Test buttons on the sheet | missing |
| Import directly from `.xlsm` (use "Save data to file" in Excel and import the `.MLL_pgf`) | not planned |
| User interface in other languages than English (effect texts are already translated) | missing |
| Code-signed installers (macOS shows "unidentified developer", Windows SmartScreen warns) | missing |

### Planned

1. Verify the generated header against the Excel tool with real layouts (see below), then remove "untested"
2. Switches (`SwitchA`…`D`), variables as addresses and effects that set variables (`DstVar`)
3. Uploading the DCC/Selectrix receiver program
4. Simulator that shows the LEDs without hardware
5. Sound channels, multiplexer, include, DMX
6. Translations of the user interface (German first)
7. Arduino setup inside the app: on the first upload, download `arduino-cli` and install pinned versions of
   MobaLedLib, FastLED and the board package into the app's own folders, so it doesn't depend on (or change)
   the user's Arduino IDE setup

## Help wanted: testing

The most useful contribution right now:

1. In Excel, open your layout, press "Save data to file" to get a `.MLL_pgf`, and keep the
   `LEDs_AutoProg.h` that Excel generated for the same sheet.
2. Run `mll generate yourfile.MLL_pgf 0 studio.h` (or use *Code* in the app).
3. Compare both files. Apart from the program name and the creation time in the first lines and in
   `START_MSG`, they should be identical. Please open an issue with any difference.

## Install

Download the installer for your system from the [Releases](https://github.com/CuriousTimo/MobaLedLib-Studio/releases) page:

- **Windows:** `MobaLedLib Studio_x.y.z_x64-setup.exe`
- **macOS:** `.dmg` for Apple Silicon (`aarch64`) or Intel (`x64`). The app isn't signed yet: right-click it
  and choose *Open* the first time.
- **Linux:** `.deb`, `.rpm` or `.AppImage`

For uploading you also need:

1. [`arduino-cli`](https://arduino.github.io/arduino-cli/latest/installation/) (or Arduino IDE 2, which contains it)
2. The board support and the MobaLedLib library:
   ```sh
   arduino-cli core install arduino:avr
   arduino-cli lib install MobaLedLib@3.5.0
   arduino-cli lib install FastLED@3.9.12
   ```
   The second `lib install` matters: installing MobaLedLib pulls in the newest FastLED, and MobaLedLib 3.5.0
   doesn't compile with FastLED 3.10 (`using typedef-name 'using CRGB = struct fl::CRGB' after 'struct'`).
   3.9.12 is the version the Excel Program Generator installs.

## Build from source

Requirements: Rust (stable), and on Linux the WebKitGTK libraries
(`sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`). Node.js is **not** needed:
the frontend is plain HTML, CSS and JavaScript in `app/ui`.

```sh
cargo run -p mobaledlib-studio        # start the app
cargo test --workspace                # run the tests
cargo run -p mll-cli -- generate layout.json
```

Installers are built with the Tauri CLI: `cargo install tauri-cli --version "^2"`, then `cd app && cargo tauri build`.
On macOS, `cargo tauri build --bundles app` is enough to get `target/release/bundle/macos/MobaLedLib Studio.app`.
Start that, not the bare `target/release/mobaledlib-studio` binary: macOS opens a bare binary in Terminal, and it
has no app icon, because the icon and the app metadata live in the `.app` bundle.

## Project layout

```
crates/mll-core     data model, .MLL_pgf import/export, catalog, LED numbering, header generation
crates/mll-cli      the `mll` command line tool
app/src-tauri       the desktop app (Tauri 2): commands, file dialogs, arduino-cli integration
app/ui              the user interface (no build step)
data/catalog.json   effect catalog extracted from the Excel workbook
```

## Versioning and releases

The project follows [Semantic Versioning](https://semver.org). Until 1.0 the layout file format and the
features can still change; pre-releases carry a suffix such as `-beta.1`. The version is set once in the
workspace `Cargo.toml` and used by every crate and by the app. Changes are listed in [CHANGELOG.md](CHANGELOG.md).

To release, update the version in `Cargo.toml` and the changelog, then push a matching tag:

```sh
git tag v0.1.0-beta.1
git push origin v0.1.0-beta.1
```

GitHub Actions then builds the installers for Windows, macOS (Apple Silicon and Intel) and Linux and the `mll`
binaries, and attaches them to a draft release. Tags with a suffix are published as pre-releases.

## License and credits

Licensed under the [GNU Lesser General Public License v2.1](LICENSE) or later, like the MobaLedLib.

MobaLedLib and the Excel Program Generator are by Hardi Stengelin, Jürgen Winkler and the MobaLedLib community.
The effect catalog, effect descriptions and the code generation rules are taken from their work.
