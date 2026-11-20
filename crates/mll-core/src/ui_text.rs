//! Texts of the user interface that the Excel tool shows too: its German text (the key into the
//! `Languages` sheet: column headers of the sheets, the `Config` page, buttons and dialogs) and the id the
//! app uses. They are translated with [`crate::lang::LanguageSheet::get`], as `Get_Language_Str` does.
//!
//! All other texts of the app are our own and stay English.

use std::collections::BTreeMap;

use crate::Catalog;

/// `(id, German text)`. The comment names where the Excel tool shows it.
pub const KEYS: &[(&str, &str)] = &[
    // Column headers of the DCC, Selectrix, CAN and LNet sheets (Languages: Cell, Cell_DCC, Cell_SX)
    ("active", "Aktiv"),
    ("filter", "Filter"),
    ("address", "Adresse oder Name"),
    ("sx_channel", "Channel oder Name [0..99]"),
    ("bit_pos", "Bitposition [1..8]"),
    ("type", "Typ"),
    ("start_value", "Start- wert"),
    ("description", "Beschreibung"),
    ("distributor", "Verteiler- Nummer"),
    ("plug", "Stecker- Nummer"),
    ("effect", "Beleuchtung, Sound, oder andere Effekte"),
    ("leds", "LEDs"),
    ("in_cnt", "InCnt"),
    ("loc_in_ch", "Loc InCh"),
    // Input types: the selection dialog of the "Typ" column (DialogBox)
    ("red", "Rot"),
    ("green", "Grün"),
    ("on_off", "An / Aus"),
    ("push", "Tast"),
    // Distributor and plug dialog (DialogBox)
    ("distributor_plug", "Verteiler und Stecker Nummer"),
    // Protocol selection dialog (DialogBox)
    ("can_bus", "Märklin CAN Bus"),
    // Config page (Config Sh)
    ("house_times", "Standardzeiten der House() Funktion:"),
    ("house_min_t", "Minimale Zeit bis zur nächsten Änderung :"),
    ("house_max_t", "Maximale Zeit bis zur nächsten Änderung :"),
    ("dcc_offset", "Offset für DCC Adresse :"),
    ("color_test", "Aktiviere LED Farbtest Mode :"),
    ("language", "Umstellung der Sprache:"),
    // Buttons of the dialogs (DialogBox)
    ("cancel", "Abbrechen"),
    ("close", "Schließen"),
    ("led_arduino", "LED Arduino"),
];

/// The texts of [`KEYS`] in language `lang` (Excel's language number), by id. "|" and a line end in the
/// sheet (`vbCr`) become a line end.
pub fn texts(catalog: &Catalog, lang: usize) -> BTreeMap<&'static str, String> {
    KEYS.iter()
        .map(|&(id, de)| (id, catalog.languages.get(de, lang).replace("\r\n", "\n").replace('\r', "\n")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_in_the_languages_sheet() {
        let l = Catalog::bundled().languages;
        let missing: Vec<&str> = KEYS.iter().map(|&(_, de)| de).filter(|de| !l.contains(de)).collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = KEYS.iter().map(|&(id, _)| id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), KEYS.len());
    }

    #[test]
    fn english_and_dutch() {
        let c = Catalog::bundled();
        let en = texts(&c, crate::lang::ENGLISH);
        assert_eq!(en["address"], "Address or name");
        assert_eq!(en["start_value"], "Start- val");
        assert_eq!(en["house_min_t"], "Minimum time until the next change:");
        let de = texts(&c, 0);
        assert_eq!(de["distributor"], "Verteiler- Nummer");
    }
}
