# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-beta.1] - 2026-10-27

First public beta. Untested on real hardware.

### Added
- Layout editor with sheets, effect list, inspector and effect gallery for all 285 MobaLedLib effects
- JSON layout files, import and export of Excel `.MLL_pgf` files
- Generation of `LEDs_AutoProg.h`, ported from the Excel Program Generator 3.5.0
- Compile and upload through `arduino-cli`
- `mll` command line tool: `generate`, `convert`, `extract-catalog`
- Test that adds every catalog effect with its form defaults and generates the header
- Native menu bar (File, Edit, Effect, Arduino, Window) with keyboard shortcuts; replaces the File menu inside the window
- Asks "Save / Don't Save / Cancel" before closing, quitting or opening another layout with unsaved changes
- Open a layout or `.MLL_pgf` file by dropping it onto the window
- Right-click menu on effects: Change, Duplicate, Move Up/Down, Delete
- Own About window with version, author and links to GitHub and the author's site (the native about panels can't show links)
- Delete Effect asks for confirmation
- Window › Close Window (⌘W)
- Effect texts in all 7 languages of the catalog (Italian, Spanish and Danish were missing)

### Changed
- README: install FastLED 3.9.12 with MobaLedLib 3.5.0; the newest FastLED doesn't compile with it
- Confirmations use native dialogs
- Buttons that only repeated a menu item are gone: Open, Save and Import (File menu), Code (Arduino menu), the settings gear (the board in the status bar and ⌘, open the settings) and the Move/Duplicate/Delete buttons in the inspector (right-click menu and Effect menu). Add effect, Change… and Upload stay as buttons
- The interface behaves like a desktop app: system font, arrow cursor, no text selection or browser context menu outside text fields, dark title bar

### Fixed
- Switching the effect text language in the effect gallery emptied the list when a category was selected
- Effect and parameter texts that are empty in the chosen language now fall back to English, then German
- The effect gallery offered only English and German, the layout settings four languages
- Windows: arduino-cli no longer opens a console window for every call
- Effects with an LED channel (Servo, Const, Blinker, SingleLedSignal, … — more than 40) had no C1/C2/C3 choice in the effect form; servos then got no LED number and were missing from the LED bus
- Effects with `#LocInCh`, `#LED` or `#InCh` in their definition (Flash, RandWelding, Def_Neon_Misha, Herz_MoRelais, PushButton2I_…, all MP3_… effects) asked for these placeholders instead of filling them in
- An effect without an LED channel is refused instead of being added without LEDs; existing rows like that get a warning
- Effects whose definition is a comment or `#define` (`// Set_LEDNr`, `// Next_LED`, `// Reserve LEDs`, `// Define Input`, the pin settings) lost their `//` and produced code that doesn't compile
- `ConstrWarnLight` produced a macro that doesn't exist in the library; it now becomes `ConstrWarnLight<n>` or `ConstrWarnLightFlash<n>` with LEDs `C1-<n>`, like Excel
- Color fields (`Col0`…`Col5` of the PushButton_RGB effects) are passed as separate numbers, like Excel
- Effects that set variables (MonoFlop, flip-flops, Counter, Random, RandMux, Schedule, PushButton…) are reported as not supported yet instead of producing a header that doesn't compile
- Garbled characters in the machine translated effect texts (`Ã¢ â‚¬ Å“`, `â€‹`, `%28%29`, invisible zero-width spaces); `mll extract-catalog` now repairs them

[Unreleased]: https://github.com/CuriousTimo/MobaLedLib-Studio/compare/v0.1.0-beta.1...HEAD
[0.1.0-beta.1]: https://github.com/CuriousTimo/MobaLedLib-Studio/releases/tag/v0.1.0-beta.1
