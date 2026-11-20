//! The messages of the header generation, as the German literals the Excel tool passes to
//! `Get_Language_Str` (M06_Write_Header.bas, M06_Write_Header_SW.bas, M09_SelectMacro.bas). They are
//! the keys into the `Languages` sheet: `codegen` translates them with [`crate::lang::LanguageSheet`].
//! "\r" is VBA's `vbCr`, "\n" `vbLf`; "#1#", "#2#", … are replaced after the translation, as Excel does.
//!
//! Messages of our own (no Excel counterpart) are in English and listed in [`OWN`].

// M06_Write_Header_SW.bas
pub const NO_ADDRESS: &str = "Fehler: In Zeile #1# ist keine Adresse, kein Schalter oder keine Variable eingetragen";
pub const BIN_STATES: &str = "Fehler: Anzahl der binären Zustände ungültig. Die Anzahl muss zwischen 1 und 63 liegen";
pub const LOGIC_PARAMS: &str = "Fehler: Falsche Parameter Anzahl in 'Logic()' Ausdruck: '#1#'";
pub const VAR_NR_INVALID: &str = "Fehler: Die Nummer der Variable '#1#' ist ungültig!\r\rGültiger Bereich: #2#";
pub const SPI_PIN: &str = "Fehler: Der Arduino Pin '#1#' kann nicht als Ein- oder Ausgang werden wenn \
DCC, LNet oder Selectrix Daten per SPI Bus gelesen werden. Es muss ein anderer Anschluss verwendet \
werden oder die SPI Kommunikation in der 'Config' Seite deaktiviert werden.\nAchtung: Die beiden Arduinos \
müssen dann per RS232 verbunden sein.";
pub const PIN_INVALID: &str = "Fehler: Der Pin '#1#' ist nicht gültig im\r  '#2#' Befehl";
pub const PIN_TWICE: &str = "Fehler: Der Pin '#1#' wird mehrfach verwendet im\r  '#2#' Befehl";
pub const PIN_READ: &str = "Fehler beim Lesen der Pin Nummern in Zeile:\r  '#1#'";
pub const DST_VAR_DETECT: &str = "Fehler bei der Erkennung der Zielvariable in Makro '#1#'";
pub const DST_VAR_SEQUENCE: &str = "Fehler: Die Zielvariable '#1#' in Zeile #2# muss eine Zahl am Ende haben \
weil sie Teil einer Sequenz ist.\r  Beispiel: #3#";
pub const DST_VAR_DEFINITION: &str = "Fehler in der Definition der Zielvariable(n): '#1#' in Zeile #2#";
pub const VAR_NOT_DEFINED: &str = "Fehler: Die Variable '#1#' in Zeile #2# ist nicht definiert";
pub const ANALOG_NOT_SUPPORTED: &str = "Analoge Taster werden auf dieser Plattform nicht unterstützt.";
pub const LDR_NOT_SUPPORTED: &str = "Ein Lichtsensor wird auf dieser Plattform nicht unterstützt.";
pub const TOO_MANY_ANALOG: &str = "Fehler: Es wurden mehr analoge Taster verwendet als möglich sind. \
Es müssen weitere analoge Eingänge zum einlesen definiert werden.\rDas wird mit dem Befehl \
'Set_SwitchA_InpLst()' in der Makro Spalte gemacht.";
pub const TOO_MANY_SWITCH_D: &str = "Fehler: Es wurden mehr SwitchD Schalter verwendet als Pins definiert sind. \
Es müssen weitere Eingänge zum einlesen definiert werden.\rDas wird mit dem Befehl 'Set_SwitchD_InpLst()' in \
der Makro Spalte gemacht.\rLetzter möglicher Schalter: 'SwitchD#1#'";
pub const LED_PINS: &str = "Fehler: Es sind nicht genügend Ausgangs Pins zur Ansteuerung der LEDs vorhanden. \
Die LED Pins müssen mit dem Befehl \"Set_LED_OutpPinLst()\" definiert werden.\rEs müssen #1# Arduino \
Ausgänge definiert sein.";
pub const PIN_IN_TWO_SWITCHES: &str = "Fehler: Der Eingabe Pin '#1#' des Arduinos wird als Eingabe in den zwei \
Schalter Funktionen '#2#' und '#3#' benutzt.\rDie beiden Schalter Funktionen können nicht gleichzeitig benutzt \
werden. Die Pins können mit den Funktionen 'Set_Switch?_InpLst()' angepasst werden.\rAchtung: Dazu muss auch \
die Hardware angepasst werden!";
pub const PIN_SWITCH_AND_OTHER: &str = "Fehler: Der Eingabe Pin '#1#' des Arduinos wird als Eingabe in der \
Schalter Funktionen '#2#' und gleichzeitig als #3# Pin benutzt.\rDie Pins können mit den Funktionen \
'Set_Switch?_InpLst()' und '#4#' angepasst werden.\rAchtung: Dazu muss auch die Hardware angepasst werden!";
pub const PIN_FOR_SWITCH_B_C: &str = "#1# Pin für SwitchB oder SwitchC";
pub const MULTI_SET_DST_VARS: &str = "Achtung: Die folgenden Zielvariablen werden mehrfach gesetzt:";
pub const INPUT_NEVER_SET: &str = "Fehler: Die Variable '#1#' wird als Eingang benutzt, wird aber nirgendwo gesetzt.";

// M09_SelectMacro.bas
pub const IN_CH_CONSTANT: &str =
    "Fehler im Logischen Ausdruck. Es darf nur eine konstante positive Zahl zu '#InCh' addiert werden";

// M06_Write_Header.bas
pub const NEGATIVE_LED: &str = "Fehler: Die LED Nummer darf nicht negativ werden. Das kann durch eine falsche \
Angabe bei einem vorangegangenen \"Next_LED\" Befehl passieren.";
pub const NEGATIVE_START: &str = "Negativer Startwert in Zeile ";
pub const START_IN_ROW: &str = "Startwert in Zeile ";
pub const START_TOO_BIG: &str = " ist zu groß. Maximal möglicher Wert: ";
pub const THE_ADDRESS: &str = "Die Adresse '";
pub const IN_ROW: &str = "' in Zeile ";
pub const OTHER_TYPE: &str = " wird bereits mit einem anderen Typ benutzt.";
pub const IS_INVALID: &str = " ist ungültig.";
pub const LED_CHANNEL: &str =
    "Fehler: Der 'LED Kanal' in Zeile #1# ist ungültig.\rEs sind die Led Kanäle 0-#2# erlaubt.";
pub const STORE_TOO_MANY: &str = "Fehler: Die Anzahl der zu speichernden Eingänge in Zeile #1# ist zu groß (> 63)";
pub const ENTRY: &str = "Fehler: Eintrag '";
pub const IN_IN_CNT_COLUMN: &str = "' in InCnt Spalte ist ungültig";
pub const IN_START_COLUMN: &str = "' in Startwert Spalte ist ungültig";
pub const PROTOCOL_NOT_SUPPORTED: &str = "Das Protokoll #1# wird auf dieser Plattform nicht unterstützt.";
pub const ALWAYS_ACTIVE_1: &str = "Die Ausgewählte Funktion in Zeile ";
pub const ALWAYS_ACTIVE_2: &str = " ist immer aktiv und kann nicht über DCC, LNet oder CAN geschaltet werden.";
pub const MAX_LEDS_1: &str = "Maximale LED Anzahl überschritten: ";
pub const MAX_LEDS_2: &str = "Es sind maximal #1# RGB LEDs möglich";
pub const HOUSE_TIMES: &str = "Fehler auf der 'Config' Seite:\rDie 'Minimale Zeit bis zur nächsten Änderung' muss \
kleiner oder gleich groß wie die Maximale Zeit sein.\rAchtung: Wenn nichts eingegeben ist werden die Standard \
Werte vom 50/150 verwendet. Dadurch kann es ebenfalls zu einem Konflikt kommen.";
pub const IN_CH_TOO_MANY: &str = "Fehler: Die Anzahl der verwendeten Eingangskanäle ist zu groß!\rEs sind \
maximal 250 verfügbar. Die Konfiguration enthält aber ";
pub const IN_CH_USE: &str = "Die Eingangskanäle werden zum einlesen von DCC, LNet, Selectrix und CAN Daten \
benutzt. \rAußerdem werden sie als interne Zwischenspeicher benötigt.";
pub const EMPTY_CONFIG: &str = "Achtung: Es ist keine einzige Zeile in der Spalte \"Beleuchtung, Sound, oder \
andere Effekte\" aktiv!\r=> Das Programm wird keine LEDs ansteuern";

/// Every key above: each must be in the `Languages` sheet.
pub const ALL: &[&str] = &[
    NO_ADDRESS,
    BIN_STATES,
    LOGIC_PARAMS,
    VAR_NR_INVALID,
    SPI_PIN,
    PIN_INVALID,
    PIN_TWICE,
    PIN_READ,
    DST_VAR_DETECT,
    DST_VAR_SEQUENCE,
    DST_VAR_DEFINITION,
    VAR_NOT_DEFINED,
    ANALOG_NOT_SUPPORTED,
    LDR_NOT_SUPPORTED,
    TOO_MANY_ANALOG,
    TOO_MANY_SWITCH_D,
    LED_PINS,
    PIN_IN_TWO_SWITCHES,
    PIN_SWITCH_AND_OTHER,
    PIN_FOR_SWITCH_B_C,
    MULTI_SET_DST_VARS,
    INPUT_NEVER_SET,
    IN_CH_CONSTANT,
    NEGATIVE_LED,
    NEGATIVE_START,
    START_IN_ROW,
    START_TOO_BIG,
    THE_ADDRESS,
    IN_ROW,
    OTHER_TYPE,
    IS_INVALID,
    LED_CHANNEL,
    STORE_TOO_MANY,
    ENTRY,
    IN_IN_CNT_COLUMN,
    IN_START_COLUMN,
    PROTOCOL_NOT_SUPPORTED,
    ALWAYS_ACTIVE_1,
    ALWAYS_ACTIVE_2,
    MAX_LEDS_1,
    MAX_LEDS_2,
    HOUSE_TIMES,
    IN_CH_TOO_MANY,
    IN_CH_USE,
    EMPTY_CONFIG,
];

/// Keys the Excel tool doesn't find in the `Languages` sheet (`Cells.Find` with `MatchCase:=True`: the
/// sheet has "Binären Zustände", "zum Einlesen", …), so it shows them in German in every language.
pub const NOT_IN_SHEET: &[&str] = &[BIN_STATES, STORE_TOO_MANY, IN_CH_USE];

/// Messages the Excel tool shows as fixed text, not through `Get_Language_Str`: they stay as Excel has them.
pub const WRONG_SX_CHANNEL: &str = "Wrong SX channel in row ";
/// The VBA literal is `"Wrong bitpos "" & bp & "" in row "`: Excel shows the quotes and `& bp &` as text.
pub const WRONG_BIT_POS: &str = "Wrong bitpos \" & bp & \" in row ";

/// Messages of our own, in English only (the Excel tool has none, a German one only, or stops with a
/// run-time error there).
pub const OWN: &[&str] = &[
    "Too few parameters in '#1#'",
    "Internal error: undefined OutCnt entry '#1#' of macro '#2#'",
    "Row #1#: the effect uses an LED but its LEDs field is empty; choose the effect again with Change…",
    "'#LocInCh' is used but 'Loc InCh' is 0",
    "The input type (Red/Green/OnOff/Push) is missing",
    "#1# is not supported by MobaLedLib Studio yet",
    "Serial sound channels are not supported by MobaLedLib Studio yet",
    "Return messages (RM) as address are not supported by MobaLedLib Studio yet",
    "sheet not found",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_in_the_languages_sheet() {
        let l = crate::Catalog::bundled().languages;
        let missing: Vec<&str> = ALL.iter().copied().filter(|k| !l.contains(k)).collect();
        assert_eq!(missing, NOT_IN_SHEET);
    }
}
