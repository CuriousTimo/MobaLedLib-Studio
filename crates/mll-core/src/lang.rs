//! The hidden `Languages` sheet of the workbook and `Get_Language_Str` (M09_Language.bas).
//!
//! The sheet holds one text per row in German (column C), English, Dutch, French, Italian, Spanish and
//! Danish (columns D..I). The Excel tool looks a text up in any cell of the sheet and returns the text of
//! the same row in the active language. `vbCr` can't be stored in a cell: the sheet has "|" instead.

use serde::{Deserialize, Serialize};

use crate::vba::is_numeric;

/// `FirstLangCol` (M09_Language.bas) as a 0-based column: German. The other languages follow in the
/// order of [`crate::catalog::LANGUAGES`].
pub const FIRST_LANG_COL: usize = 2;

/// Excel's language number of English (`Get_ExcelLanguage`: 1 = "English and all other languages").
pub const ENGLISH: usize = 1;

/// The `Languages` sheet, extracted from the workbook (`mll extract-catalog`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LanguageSheet {
    /// Cell values from A1, row by row, as Excel holds them in memory: line ends are "\n", a "\r"
    /// stored as `_x000D_` is "\r". Leading and trailing spaces are kept.
    pub rows: Vec<Vec<String>>,
    /// Cells that hold a formula, `[row, col]` from 0. Excel searches with `LookIn:=xlFormulas`, so
    /// such a cell is found by its formula ("=C13"), never by its value.
    #[serde(default)]
    pub formulas: Vec<[u32; 2]>,
}

/// VBA `RTrim`: only spaces.
fn rtrim(s: &str) -> &str {
    s.trim_end_matches(' ')
}

impl LanguageSheet {
    fn cell(&self, r: usize, c: usize) -> &str {
        self.rows.get(r).and_then(|row| row.get(c)).map_or("", |s| s.as_str())
    }

    fn is_formula(&self, r: usize, c: usize) -> bool {
        self.formulas.iter().any(|&[fr, fc]| fr as usize == r && fc as usize == c)
    }

    /// `Find_Language_Str_Row` (M09_Language.bas) with `xlPart`: `Cells.Find` starts after A1 and goes
    /// row by row (A1 comes last); a found cell only counts if it is the whole text (`RTrim` on both
    /// sides), else the search goes on until it is back at the first cell found. Returns the row (from 0).
    pub fn find_row(&self, desc: &str) -> Option<usize> {
        let desc = rtrim(desc);
        if desc.is_empty() {
            return None;
        }
        let cols = self.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let cells = (0..self.rows.len()).flat_map(|r| (0..cols).map(move |c| (r, c)));
        cells
            .skip(1)
            .chain(std::iter::once((0, 0)))
            .find(|&(r, c)| !self.is_formula(r, c) && rtrim(self.cell(r, c)) == desc)
            .map(|(r, _)| r)
    }

    /// `Get_Language_Str_Sub` (M09_Language.bas): the text of `desc`'s row in language `lang` (0 = German,
    /// 1 = English, ...); if that is empty the English text, else `desc` itself.
    fn get_sub(&self, desc: &str, lang: usize) -> String {
        if is_numeric(desc) {
            return desc.to_string();
        }
        let row = self.find_row(desc);
        if let Some(r) = row {
            let t = self.cell(r, FIRST_LANG_COL + lang);
            if !t.is_empty() {
                return t.to_string();
            }
        }
        let english = row.map_or("", |r| self.cell(r, FIRST_LANG_COL + ENGLISH));
        if english.is_empty() {
            desc.to_string()
        } else {
            english.to_string()
        }
    }

    /// `Get_Language_Str` (M09_Language.bas): `desc` (the German text of the program) in language `lang`.
    /// A "\r" in `desc` is "|" (and a line end) in the sheet and comes back as "\r".
    pub fn get(&self, desc: &str, lang: usize) -> String {
        if desc.is_empty() {
            return String::new();
        }
        let mut desc = desc.replace("| ", "|");
        let use_crlf = desc.contains("\r\n");
        desc = if use_crlf { desc.replace('\r', "|") } else { desc.replace('\r', "|\n") };
        let res = self.get_sub(&desc, lang);
        if use_crlf {
            res.replace('|', "\r")
        } else {
            res.replace("|\n", "\r").replace("| \n", "\r")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> LanguageSheet {
        let row = |cells: &[&str]| cells.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        LanguageSheet {
            rows: vec![
                row(&["", "", "Note"]),
                row(&["Type", "Param", "German", "English", "Dutch"]),
                row(&["VBA", "", "Fehler", "Error", "Fout"]),
                row(&["VBA", "", "Zeile 1|\nZeile 2", "Line 1|\nLine 2", ""]),
                row(&["VBA", "", "Nur Deutsch ", "", ""]),
                row(&["VBA", "", "Kopie", "Copy", "Error"]),
            ],
            formulas: vec![[5, 4]],
        }
    }

    #[test]
    fn translates_by_the_german_text() {
        let s = sheet();
        assert_eq!(s.get("Fehler", 0), "Fehler");
        assert_eq!(s.get("Fehler", 1), "Error");
        assert_eq!(s.get("Fehler", 2), "Fout");
    }

    #[test]
    fn any_cell_of_the_row_is_found() {
        // Cells.Find searches every column: the English text finds its row too
        assert_eq!(sheet().get("Error", 2), "Fout");
    }

    #[test]
    fn formula_cells_are_not_found_by_their_value() {
        // E6 holds a formula with the value "Error": the first "Error" found is still D3
        assert_eq!(sheet().find_row("Error"), Some(2));
        assert_eq!(sheet().find_row("Copy"), Some(5));
    }

    #[test]
    fn vb_cr_is_a_bar_in_the_sheet() {
        assert_eq!(sheet().get("Zeile 1\rZeile 2", 1), "Line 1\rLine 2");
        // Missing Dutch: the English text
        assert_eq!(sheet().get("Zeile 1\rZeile 2", 2), "Line 1\rLine 2");
    }

    #[test]
    fn missing_texts() {
        let s = sheet();
        // Trailing spaces are ignored when comparing; no English text: the text itself
        assert_eq!(s.get("Nur Deutsch", 1), "Nur Deutsch");
        assert_eq!(s.get("Unbekannt", 1), "Unbekannt");
        assert_eq!(s.get("42", 1), "42");
        assert_eq!(s.get("", 1), "");
    }

    #[test]
    fn the_workbook_sheet() {
        let l = crate::Catalog::bundled().languages;
        let de = "Fehler: Die Variable '#1#' wird als Eingang benutzt, wird aber nirgendwo gesetzt.";
        assert_eq!(l.get(de, 1), "Error: The variable '#1#' is used as an input, but is not set anywhere.");
        assert_eq!(l.get(de, 2), "Fout: De variabele '#1#' wordt gebruikt als invoer, maar wordt nergens ingesteld.");
        // "Valeur |" + line end in the French cell of "Start- wert"
        assert_eq!(l.get("Start- wert", 3), "Valeur \rde départ");
    }
}
