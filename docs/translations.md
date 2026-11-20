# Translations

MobaLedLib Studio translates only what the Excel tool (Program Generator 3.5.0) translates itself, with the
Excel tool's own texts from the hidden `Languages` sheet of the workbook. The sheet is extracted into
`data/catalog.json` (`languages`) and searched the way `Get_Language_Str` does
(`crates/mll-core/src/lang.rs`). One setting chooses the language for all of it (Layout settings →
"Change of language:"), as the Excel tool's `Config` page does.

Languages: German, English, Dutch, French, Italian, Spanish, Danish. Only German and English are written
by the MobaLedLib authors; the other languages are machine translations in the workbook.

## What is translated

| Texts | Source | Where in the code |
|---|---|---|
| Effect names, descriptions and parameter texts | `Lib_Macros`, `Par_Description` sheets | `data/catalog.json` |
| Errors and warnings of the header generation | `Get_Language_Str` calls in `M06_Write_Header*.bas`, `M09_SelectMacro.bas` | `crates/mll-core/src/gen_text.rs` |
| Column headers, input types, `Config` page labels, Cancel, Close | `Languages` sheet rows `Cell*`, `DialogBox`, `Config Sh` | `crates/mll-core/src/ui_text.rs` |

The texts are taken as the Excel tool shows them, including the flaws of the machine translations: English
shows "Activ", "Start- val", "Distribution- number"; some messages have a "|" without a line end, or miss a
space or a quote.

Three generator messages are not found in the sheet by the Excel tool either (`Cells.Find` with
`MatchCase:=True`; the sheet spells them differently) and are German in every language: the number of binary
states, the number of inputs to store, and the second part of "too many input channels"
(`gen_text::NOT_IN_SHEET`).

Two Selectrix messages are fixed English text in the Excel tool (`gen_text::WRONG_SX_CHANNEL`,
`gen_text::WRONG_BIT_POS`) and stay so.

## English only

Everything else is our own and stays English. This list is taken from `app/ui/app.js`,
`app/ui/about.html`, the native menu (`app/src-tauri/src/main.rs`) and `crates/mll-core/src/gen_text.rs`
(state of step 6.3).

### Generator messages without an Excel counterpart (`gen_text::OWN`)

- Too few parameters in '…'
- Internal error: undefined OutCnt entry '…' of macro '…'
- Row …: the effect uses an LED but its LEDs field is empty; choose the effect again with Change…
- '#LocInCh' is used but 'Loc InCh' is 0
- The input type (Red/Green/OnOff/Push) is missing
- … is not supported by MobaLedLib Studio yet
- Serial sound channels are not supported by MobaLedLib Studio yet
- Return messages (RM) as address are not supported by MobaLedLib Studio yet
- sheet not found

### Native menu

File, Edit, Effect, Arduino, Window, Help · New Layout · Open… · Save · Save As… · Import .MLL_pgf… ·
Export as .MLL_pgf… · Layout Settings… · Exit · Quit MobaLedLib Studio · About MobaLedLib Studio ·
Add Effect… · Change Effect… · Duplicate · Delete Effect · Move Up · Move Down · Show Generated Code ·
Upload…

### Main window

- Title bar: Unsaved changes · Upload
- Side bar: Sheets · New sheet · Groups · All effects · Fill in "Group" in the inspector to group effects.
- Sheet: Sheet name · Command system · Filter / Filter… · Add effect · "… LEDs · … effects · … input
  channels" · Output · No effect matches the filter. · This sheet is empty. · Add the first effect ·
  Untitled · always on · no output · Enable
- Protocols: LocoNet, and the descriptions of DCC, Selectrix, Märklin CAN and LocoNet
- LED bus: LED bus · LED bus · channel … · LED · Servo board · Sound board · "… of … positions on channel …"
- Status bar: Checking… · Ready to upload · "… problem(s) — row …: …"

### Inspector

Inspector · Select an effect to edit it. · Name / description · Effect · Custom macro · Change… · Trigger ·
always on · e.g. Station · "Uses … input channel(s). An empty address keeps the effect always on." · Wiring ·
LED channel · For experts: macro and counts · Macro (one per line)

### Start page and new layout

MobaLedLib Studio · Recent · "Layouts you open or save show up here. Open one with File › Open… or drop it
onto this window." · Coming from the Excel generator? · "In Excel, use Save data to file, then drop the
.MLL_pgf file onto this window or choose File › Import .MLL_pgf… All sheets and effects come across." ·
Start a new layout · "Two questions, then you're adding effects. You can change both later." · Name ·
e.g. Anlage Altstadt · 1 · How do you control your layout? · Command system · 2 · Which controller drives
the LEDs? · Controller · Create layout · the controller descriptions (Classic main board · up to 256 RGB
LEDs, …)

### New sheet

New sheet · Name · Command system · Each sheet is uploaded to its own LED Arduino. · Create sheet

### Effect picker

Add an effect · Change effect · Choose an effect · Search effects · Search: house, signal, servo, Blink… ·
Categories · Effects · All · Other · No effect found. · Choose an effect on the left to see its settings. ·
Settings · Remove · Rooms · Values · List · Lights · This effect needs the color table editor, which
MobaLedLib Studio doesn't have yet. · For experts: library macro · Add to sheet · Apply

### Generated code

Generating… · Generated code · Copy · Copied

### Layout settings

Layout settings · Layout name · Controller · Board (arduino-cli) and its board names · Command station ·
4 for Roco and some others · Simulate button release (Legacy, Off, Mode 1, Mode 2) · Use SPI between the DCC
and the LED Arduino · Only English and German are written by the MobaLedLib authors; the other languages are
machine translations from the Excel tool. · Danger zone · Delete sheet "…" · Done

### Upload

Upload to the LED Arduino · Sheet "…" · Looking for arduino-cli… · arduino-cli was not found. and its install
hint · The MobaLedLib library is not installed. and its install hint · Get the library bundle · "arduino-cli … · MobaLedLib sketch
found" · Fix these problems first: · Board · Port · No board found · Refresh · Generate · Compile · Upload ·
The compiler output appears here. · The DCC/Selectrix Arduino needs its own program once; v1 doesn't upload
it yet. · Compile only · Compile & upload · Working… · Compiled · Uploaded to the Arduino

### Questions and notices

Your changes will be lost if you don't save them. · Save · Don't Save · Delete sheet? · "… and its …
effects will be removed." · Delete effect? · "… will be removed from the sheet." · Delete · Saved ·
Exported … · Imported. Save it as a layout file to keep your changes. · Could not load the macro catalog: …

### About window

About MobaLedLib Studio · Configure MobaLedLib effects for your model railway and upload them to the
Arduino. · Made by · About me · Independent community project, not made or endorsed by the MobaLedLib
authors.
