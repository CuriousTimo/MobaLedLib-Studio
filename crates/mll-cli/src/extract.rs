//! Extract the macro catalog from the Excel workbook's hidden sheets.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook_auto, Data, Range, Reader};
use mll_core::catalog::{Catalog, MacroDef, MacroText, ParamDef, ParamText, LANGUAGES};
use mll_core::lang::LanguageSheet;
use regex::{Captures, Regex};

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

/// Repair the machine translated texts of the workbook. They contain URL escapes (`%28`), zero-width
/// spaces and quote marks that went through UTF-8 → Windows-1252 once or twice, sometimes with spaces
/// inserted and bytes lost (`Ã¢ â‚¬ Å“`, `â € œ`, `â€‹`, a lone `â`).
fn clean_text(text: &str) -> String {
    static RULES: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
        [
            // „ “ ” as double mojibake
            (r#"Ã â (\w)"#, "\"$1"),
            (r#"Ã(?: ?¢(?: ?â)?| ?â)(?: ?‚¬)?(?: ?Å ?[“"¾])?"#, "\""),
            // zero-width spaces as mojibake, with or without the last byte
            (r"â ?€ ?‹|â€â€|“â€|ââ", ""),
            // “ „ ” as single mojibake
            (r"â ?€(?: ?[œž“])?", "\""),
            // the extra space the translation put before () and before a closing quote
            (r"(\w) \(\)", "$1()"),
            (r#"\(\) ""#, "()\""),
        ]
        .into_iter()
        .map(|(re, to)| (Regex::new(re).unwrap(), to))
        .collect()
    });
    static LONE_A: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\p{L})?â(\p{L})?").unwrap());
    static LATIN1: LazyLock<Regex> = LazyLock::new(|| Regex::new("Ã([\u{80}-\u{BF}])").unwrap());

    let mut s = text.replace("%28", "(").replace("%29", ")").replace("%3D", "=").replace('\u{200B}', "");
    for (re, to) in RULES.iter() {
        s = re.replace_all(&s, *to).into_owned();
    }
    // A quote that lost its last two bytes; â inside a word is real (French "câble")
    s = LONE_A
        .replace_all(&s, |c: &Captures| match (c.get(1), c.get(2)) {
            (Some(_), Some(_)) => c[0].to_string(),
            (a, b) => format!("{}\"{}", a.map_or("", |m| m.as_str()), b.map_or("", |m| m.as_str())),
        })
        .into_owned();
    // A single letter as mojibake (lÃ³gicas)
    LATIN1
        .replace_all(&s, |c: &Captures| {
            let byte = c[1].chars().next().unwrap() as u32;
            char::from_u32(0xC0 + byte - 0x80).unwrap().to_string()
        })
        .into_owned()
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
                group: clean_text(&lm.get(r, c)),
                title: clean_text(&lm.get(r, c + 1)),
                short: clean_text(&lm.get(r, c + 2)),
                long: clean_text(&lm.get(r, c + 3)),
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
        let de = ParamText { label: clean_text(&pd.get(r, 7)), hint: clean_text(&pd.get(r, 8)) };
        if !(de.label.is_empty() && de.hint.is_empty()) {
            text.insert("de".to_string(), de);
        }
        for (i, lang) in LANGUAGES.iter().enumerate().skip(1) {
            let c = 10 + (i - 1) * 2;
            let t = ParamText { label: clean_text(&pd.get(r, c)), hint: clean_text(&pd.get(r, c + 1)) };
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

    let languages = languages(&mut wb, xlsm)?;

    Ok(Catalog { lib_version: lib_version(properties)?, macros, params, platforms, languages })
}

/// A cell text as Excel holds it in memory. calamine returns the raw XML text: the XML parser of Excel
/// turns "\r\n" and a lone "\r" into "\n" (line end normalization), and `_xHHHH_` stands for the
/// character HHHH (`_x000D_` = "\r").
fn excel_text(raw: &str) -> String {
    static ESCAPE: LazyLock<Regex> = LazyLock::new(|| Regex::new("_x([0-9A-Fa-f]{4})_").unwrap());
    let s = raw.replace("\r\n", "\n").replace('\r', "\n");
    ESCAPE
        .replace_all(&s, |c: &Captures| {
            char::from_u32(u32::from_str_radix(&c[1], 16).unwrap()).map(|ch| ch.to_string()).unwrap_or_default()
        })
        .into_owned()
}

/// "D13" -> (12, 3)
fn cell_ref(r: &str) -> Option<[u32; 2]> {
    let letters: String = r.chars().take_while(|c| c.is_ascii_uppercase()).collect();
    let row: u32 = r[letters.len()..].parse().ok()?;
    let col = letters.chars().fold(0u32, |acc, c| acc * 26 + (c as u32 - 'A' as u32 + 1));
    Some([row.checked_sub(1)?, col.checked_sub(1)?])
}

/// The cells of a sheet that hold a formula, read from its XML: calamine leaves out some cells of
/// shared formulas (`<f t="shared" .../>`).
fn formula_cells(xlsm: &Path, sheet_name: &str) -> Result<Vec<[u32; 2]>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(xlsm)?)?;
    let mut read = |name: &str| -> Result<String> {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(name)?, &mut s)?;
        Ok(s)
    };
    let workbook = read("xl/workbook.xml")?;
    let rels = read("xl/_rels/workbook.xml.rels")?;
    let sheet_re = Regex::new(&format!(r#"<sheet [^>]*name="{}"[^>]*r:id="([^"]+)""#, regex::escape(sheet_name)))?;
    let rid =
        sheet_re.captures(&workbook).ok_or_else(|| anyhow!("sheet '{sheet_name}' not in workbook.xml"))?[1].to_string();
    let rel_re = Regex::new(&format!(r#"<Relationship [^>]*Id="{}"[^>]*Target="([^"]+)""#, regex::escape(&rid)))?;
    let target = rel_re.captures(&rels).ok_or_else(|| anyhow!("relationship {rid} not found"))?[1].to_string();
    let path = match target.strip_prefix('/') {
        Some(abs) => abs.to_string(),
        None => format!("xl/{target}"),
    };
    let xml = read(&path)?;
    let cell_re = Regex::new(r#"(?s)<c r="([A-Z]+[0-9]+)"[^>]*?(?:/>|>(.*?)</c>)"#)?;
    Ok(cell_re
        .captures_iter(&xml)
        .filter(|c| c.get(2).is_some_and(|b| b.as_str().contains("<f")))
        .filter_map(|c| cell_ref(&c[1]))
        .collect())
}

/// --- Languages ---
fn languages(wb: &mut calamine::Sheets<std::io::BufReader<std::fs::File>>, xlsm: &Path) -> Result<LanguageSheet> {
    let sh = sheet(wb, "Languages")?;
    let (end_r, end_c) = sh.range.end().ok_or_else(|| anyhow!("Languages: empty sheet"))?;
    let mut rows = Vec::new();
    for r in 0..=end_r {
        let mut row: Vec<String> = (0..=end_c)
            .map(|c| match sh.range.get_value((r, c)) {
                None | Some(Data::Empty) => String::new(),
                Some(Data::Float(f)) if f.fract() == 0.0 => format!("{}", *f as i64),
                Some(v) => excel_text(&v.to_string()),
            })
            .collect();
        while row.last().is_some_and(|s| s.is_empty()) {
            row.pop();
        }
        rows.push(row);
    }
    Ok(LanguageSheet { rows, formulas: formula_cells(xlsm, "Languages")? })
}

#[cfg(test)]
mod tests {
    use super::{cell_ref, clean_text, excel_text};

    #[test]
    fn cell_text_as_excel_holds_it() {
        assert_eq!(excel_text("Valeur |\r\nde départ"), "Valeur |\nde départ");
        assert_eq!(excel_text("Load all examples_x000D_Please be patient"), "Load all examples\rPlease be patient");
        assert_eq!(excel_text("a\rb"), "a\nb");
        assert_eq!(cell_ref("D13"), Some([12, 3]));
        assert_eq!(cell_ref("AA1"), Some([0, 26]));
    }

    #[test]
    fn repairs_translation_garbage() {
        let cases = [
            (
                "funktionen Ã¢ â‚¬ Å“New_Local_Var %28%29 Ã¢ â‚¬ og funktionen Ã¢ â‚¬ Å¾Use_GlobalVar %28%29 Ã¢ â‚¬ Å“ kræves",
                "funktionen \"New_Local_Var()\" og funktionen \"Use_GlobalVar()\" kræves",
            ),
            ("función Ã ¢ â‚¬ Å “New_Local_Var %28%29 Ã ¢ â‚¬ y", "función \"New_Local_Var()\" y"),
            ("funzione Ã â Use_GlobalVar %28%29 Ã¢ â‚¬ Å\", qui", "funzione \"Use_GlobalVar()\", qui"),
            ("the â € œNew_Local_Var () â € function", "the \"New_Local_Var()\" function"),
            ("Valeurs LED â€‹â€‹comme variables", "Valeurs LED comme variables"),
            ("LED-waarden â€â€als variabelen", "LED-waarden als variabelen"),
            ("Kopier LED-værdier ââN gange", "Kopier LED-værdier N gange"),
            ("af \u{200B}\u{200B}de tre farver i âSrcLEDâ til", "af de tre farver i \"SrcLED\" til"),
            ("i valori “â€da più variabili lÃ³gicas", "i valori da più variabili lógicas"),
            ("Ks 2 %3D attesa con 5% spazio, x10 %28verde%29", "Ks 2 = attesa con 5% spazio, x10 (verde)"),
            ("les câbles du mât", "les câbles du mât"),
        ];
        for (input, expected) in cases {
            assert_eq!(clean_text(input), expected, "input: {input}");
        }
    }
}
