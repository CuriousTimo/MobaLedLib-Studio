# MobaLedLib Studio — rules for working on this project

A desktop app (Rust, Tauri) that replaces MobaLedLib's Excel Program Generator. See `README.md`.

## The reference, to the letter

The behaviour is ported from **one** reference: the VBA code and data sheets of the Excel workbook
`Prog_Generator_MobaLedLib.xlsm`, Program Generator **3.5.0** (MobaLedLib 3.5.0, upstream commit `a39a493`,
workbook SHA-256 in `README.md`). The generated `LEDs_AutoProg.h` must be what the Excel tool writes.

- No fusing with other sources, no invented values or behaviour. What the workbook does not do, we do not do.
- Where the workbook is unclear, stop and ask the owner with the evidence (sheet, macro, line).
- The effect catalog `data/catalog.json` is extracted from the workbook (`mll extract-catalog`), never edited by hand.

## Checks

`scripts/check.sh` runs the same checks as CI (fmt, clippy with `-D warnings`, the core and CLI tests,
the app build). Run it before every commit; a red check is fixed, not skipped.

## Standing rules

- Commit (and push) only when the owner asks.
- Leave no build litter: no extra `target-*` folders; remove an agent's worktree once its work is merged.
- Repository text (code, docs, commits) in English; the owner is answered in Dutch.
- Not tested on hardware and not compared with the Excel output yet: say so; never claim it is.
