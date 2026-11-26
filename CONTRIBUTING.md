# Contributing to MobaLedLib Studio

Thank you for helping. These rules apply to everyone, the maintainer included.

## Talk first

Before you start on something bigger than a small fix, say what you'd like to work on in the issue or in
[Discussions](https://github.com/CuriousTimo/MobaLedLib-Studio/discussions). Then nobody does the same
work twice, and we can agree on the approach before code is written.

## Pull requests

- Every change goes through a pull request and is reviewed before it is merged into `main`.
- Keep a pull request to one step: one module, one fix, one step of an issue.
- Run `scripts/check.sh` before you push. It runs the same checks as CI: `cargo fmt`, `cargo clippy`
  with `-D warnings`, the core, CLI and simulator tests, and the app build. A red check is fixed, not
  skipped.
- Code, comments, docs and commit messages are in English.
- Contributions are under the project's license, [LGPL-2.1](LICENSE).

## How Studio is built

- **The Excel tool is the reference.** The generated `LEDs_AutoProg.h` must be what Program Generator
  3.5.0 writes. What the workbook does not do, Studio does not do. Where the workbook is unclear, ask in
  the issue, with the sheet, macro or line.
- **The effect catalog** `data/catalog.json` is extracted from the workbook (`mll extract-catalog`). It is
  never edited by hand.
- **The table leads.** Views such as the chain view or a map show the table and warn when something
  doesn't add up. They never change the rows, their order or their checkboxes.

## Adding a module

A module is a drawing plus its data, both in `data/modules`:

1. **The drawing:** an SVG in the style of the existing ones (`mll201.svg` is a good example). It is a top
   view, drawn to scale. Every connector, and for a signal every lamp, is a group
   `<g class="port" data-port="…">`.
2. **The data:** an entry in `modules.json` with its id, name, SVG, category, LED numbers and channels, and
   its ports with their connector kind and direction. For a cuttable board, also list its sections.
   - Put facts from [wiki.mobaledlib.de](https://wiki.mobaledlib.de) or the board itself under `facts`.
   - Put anything not yet checked under `unconfirmed`.
3. **The tests** in `crates/mll-core/tests/module_library.rs` check the two against each other: unique ids,
   every port in the drawing listed in `modules.json` and the other way round, known connector kinds, and
   sections that add up to the LED count.

Module data lives in `data/modules` only, never in the page code.

## Testing on hardware

Studio has not been tested on real boards yet. Reports are very welcome; see
[Help wanted: testing](README.md#help-wanted-testing) in the README.
