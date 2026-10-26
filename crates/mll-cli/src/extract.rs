//! Extract the macro catalog from the Excel workbook's hidden sheets.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook_auto, Data, Range, Reader};
use mll_core::catalog::{Catalog, MacroDef, MacroText, ParamDef, ParamText, LANGUAGES};

struct Sheet {
    range: Range<Data>,
}

impl Sheet {
    /// Cell text at absolute (row, col), "" if empty.
    fn get(&self, r: usize, c: usize) -> String {
        match self.range.get_value((r as u32, c as u32)) {
            None | Some(Data::Empty) => String::new(),
            Some(Data::Float(f)) if f.fract() == 0.0 => format!("{}", *f as i64),
            Some(v) => v.to_string(),
        }
        .trim()
        .to_string()
    }

    fn rows(&self) -> usize {
        self.range.end().map(|(r, _)| r as usize + 1).unwrap_or(0)
    }

    /// Find the first row whose cell in `col` equals `text`.
    fn find_row(&self, col: usize, text: &str) -> Option<usize> {
        (0..self.rows()).find(|&r| self.get(r, col) == text)
    }
}

fn sheet(wb: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>, name: &str) -> Result<Sheet> {
    let range = wb.worksheet_range(name).with_context(|| format!("sheet '{name}'"))?;
    Ok(Sheet { range })
}

fn lib_version(properties: &Path) -> Result<String> {
    let text = std::fs::read_to_string(properties)?;
    text.lines()
        .find_map(|l| l.strip_prefix("version="))
        .map(|v| v.trim().to_string())
        .ok_or_else(|| anyhow!("no version in {}", properties.display()))
}

pub fn extract(xlsm: &Path, properties: &Path) -> Result<Catalog> {
    let mut wb = open_workbook_auto(xlsm).with_context(|| format!("opening {}", xlsm.display()))?;

    // --- Lib_Macros ---
    let lm = sheet(&mut wb, "Lib_Macros")?;
    let head = lm.find_row(14, "Macro").ok_or_else(|| anyhow!("Lib_Macros: header row not found"))?;
    let mut macros = Vec::new();
    for r in head + 1..lm.rows() {
        let prototype = lm.get(r, 14);
        if prototype.is_empty() {
            continue; // tree view category rows
        }
        let mut text = BTreeMap::new();
        for (i, lang) in LANGUAGES.iter().enumerate() {
            let c = 17 + i * 4;
            let t = MacroText {
                group: lm.get(r, c),
                title: lm.get(r, c + 1),
                short: lm.get(r, c + 2),
                long: lm.get(r, c + 3),
            };
            if !(t.group.is_empty() && t.title.is_empty() && t.short.is_empty() && t.long.is_empty()) {
                text.insert(lang.to_string(), t);
            }
        }
        let name = {
            let n = lm.get(r, 16);
            if n.is_empty() {
                prototype.split('(').next().unwrap_or("").trim().to_string()
            } else {
                n
            }
        };
        macros.push(MacroDef {
            name,
            find: lm.get(r, 15),
            prototype,
            dialog: lm.get(r, 0),
            mode: lm.get(r, 1),
            leds: lm.get(r, 2),
            in_cnt: lm.get(r, 3),
            out_cnt: lm.get(r, 4),
            loc_in_ch: lm.get(r, 5),
            single_led_cnt: lm.get(r, 7),
            default_channel: lm.get(r, 8),
            store_type: lm.get(r, 9),
            list_sort: lm.get(r, 10).parse().unwrap_or(0),
            tree_sort: lm.get(r, 11).parse().unwrap_or(0),
            icons: lm.get(r, 13),
            text,
        });
    }
    macros.sort_by_key(|m| m.tree_sort);

    // --- Par_Description ---
    let pd = sheet(&mut wb, "Par_Description")?;
    let head = pd.find_row(0, "ParName").ok_or_else(|| anyhow!("Par_Description: header row not found"))?;
    let mut params = Vec::new();
    for r in head + 1..pd.rows() {
        let name = pd.get(r, 0);
        if name.is_empty() {
            continue;
        }
        let mut text = BTreeMap::new();
        // German text in columns 7/8, then one label/hint pair per language starting at 10
        let de = ParamText { label: pd.get(r, 7), hint: pd.get(r, 8) };
        if !(de.label.is_empty() && de.hint.is_empty()) {
            text.insert("de".to_string(), de);
        }
        for (i, lang) in LANGUAGES.iter().enumerate().skip(1) {
            let c = 10 + (i - 1) * 2;
            let t = ParamText { label: pd.get(r, c), hint: pd.get(r, c + 1) };
            if !(t.label.is_empty() && t.hint.is_empty()) {
                text.insert(lang.to_string(), t);
            }
        }
        params.push(ParamDef {
            name,
            kind: pd.get(r, 2),
            min: pd.get(r, 3),
            max: pd.get(r, 4),
            default: pd.get(r, 5),
            options: pd.get(r, 6),
            text,
        });
    }

    // --- Platform_Parameters ---
    let pp = sheet(&mut wb, "Platform_Parameters")?;
    let head = pp.find_row(0, "Settings Name").ok_or_else(|| anyhow!("Platform_Parameters: header row not found"))?;
    let mut platforms: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let cols: Vec<(usize, String)> = (2..40).map(|c| (c, pp.get(head, c))).filter(|(_, n)| !n.is_empty()).collect();
    for r in head + 1..pp.rows() {
        let key = pp.get(r, 0);
        if key.is_empty() {
            continue;
        }
        for (c, platform) in &cols {
            // Keep the raw value: some lists need their leading/trailing spaces
            let raw = match pp.range.get_value((r as u32, *c as u32)) {
                None | Some(Data::Empty) => String::new(),
                Some(Data::Float(f)) if f.fract() == 0.0 => format!("{}", *f as i64),
                Some(v) => v.to_string(),
            };
            platforms.entry(platform.clone()).or_default().insert(key.clone(), raw);
        }
    }

    Ok(Catalog { lib_version: lib_version(properties)?, macros, params, platforms })
}
