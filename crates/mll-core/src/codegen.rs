//! Generation of `LEDs_AutoProg.h`. Port of `M06_Write_Header.bas` (and the parts of
//! `M06_Write_Header_SW.bas` that are always written).
//!
//! The output is meant to be identical to the Excel tool's, apart from the program name
//! and the creation time. Line endings are CRLF like VBA's `Print #`.

use chrono::{NaiveDateTime, Timelike};
use serde::Serialize;

use crate::catalog::{call_args, Catalog};
use crate::gen_text as t;
use crate::leds::{self, LED_CHANNELS, SERIAL_CHANNEL_PREFIX};
use crate::model::{InputType, Layout, Protocol, Row, Sheet};
use crate::vba::{is_numeric, pad, pad_left, val, val_i};

const NL: &str = "\r\n";
/// Minimum number of LEDs, so the color test program can always test 20 LEDs.
const MIN_LEDS: i64 = 20;
const VIRTUAL_CHANNEL: &str = "V";

// Status storage types of the macros (Lib_Macros "Type" column)
const MST_CTR_NONE: i64 = 1;
const MST_CTR_ON: i64 = 2;
const MST_CTR_OFF: i64 = 3;
const MST_PREVENT_STORE: i64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreStatus {
    None,
    CounterOn,
    CounterOff,
    OnOff,
    Trigger,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Diagnostic {
    /// Index of the row in the sheet, if the message belongs to a row
    pub row: Option<usize>,
    pub message: String,
}

/// The LEDs of each channel as the header counts them (at least `MIN_LEDS` in all, the rest on
/// channel 0) and the first LED of each channel in the `leds[]` array (`Start_LED_Channel`).
pub fn channel_layout(per_channel: &[i64; LED_CHANNELS]) -> ([i64; LED_CHANNELS], [i64; LED_CHANNELS]) {
    let mut leds = *per_channel;
    let num: i64 = leds.iter().sum();
    if num < MIN_LEDS {
        leds[0] += MIN_LEDS - num;
    }
    let mut start = [0i64; LED_CHANNELS];
    for nr in 1..LED_CHANNELS {
        start[nr] = start[nr - 1] + leds[nr - 1];
    }
    (leds, start)
}

#[derive(Debug, Clone, Serialize)]
pub struct Generated {
    pub header: String,
    pub num_leds: i64,
    /// Number of used input channels (InCh)
    pub input_channels: i64,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenerateError {
    pub errors: Vec<Diagnostic>,
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.errors {
            match e.row {
                Some(r) => writeln!(f, "row {}: {}", r + 1, e.message)?,
                None => writeln!(f, "{}", e.message)?,
            }
        }
        Ok(())
    }
}

impl std::error::Error for GenerateError {}

#[derive(Debug, Clone)]
pub struct GenOptions {
    /// Program name and version written into the header comment
    pub program: String,
    /// MobaLedLib version (for START_MSG)
    pub lib_version: String,
    pub now: NaiveDateTime,
    /// Language of the messages: Excel's language number (0 = German, 1 = English, ...; see
    /// [`crate::catalog::LANGUAGES`])
    pub language: usize,
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions {
            program: format!("MobaLedLib Studio {}", env!("CARGO_PKG_VERSION")),
            lib_version: String::new(),
            now: chrono::Local::now().naive_local(),
            language: crate::lang::ENGLISH,
        }
    }
}

#[derive(Debug, Clone)]
enum Addr {
    /// No address: the effect is always active (#InCh -> SI_1)
    None,
    Num(i64),
    /// A define or variable name used as input channel
    Name(String),
}

fn typ_const(t: InputType) -> &'static str {
    match t {
        InputType::OnOff => "S_ONOFF,",
        InputType::Red => "B_RED,  ",
        InputType::Green => "B_GREEN,",
        InputType::Push => "B_TAST, ",
    }
}

fn next_typ(t: InputType) -> InputType {
    match t {
        InputType::Red => InputType::Green,
        InputType::Green => InputType::Red,
        t => t,
    }
}

/// VBA `Round()` uses banker's rounding.
fn vba_round(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 && (r as i64) % 2 != 0 {
        (r - x.signum()) as i64
    } else {
        r as i64
    }
}

/// VBA `Split(s, " ")`: keeps empty elements.
fn vba_split_space(s: &str) -> Vec<String> {
    s.split(' ').map(|x| x.to_string()).collect()
}

/// VBA `Split(s, " ")`: an empty string gives no elements (`UBound` = -1).
fn vba_split(s: &str) -> Vec<&str> {
    if s.is_empty() {
        Vec::new()
    } else {
        s.split(' ').collect()
    }
}

/// VBA `InStr(start, s, sub)` (1-based, 0 if not found).
fn vba_instr(start: usize, s: &str, sub: &str) -> usize {
    let hay: Vec<char> = s.chars().collect();
    let needle: Vec<char> = sub.chars().collect();
    let start = start.max(1);
    if needle.is_empty() || start > hay.len() {
        return 0;
    }
    (start - 1..=hay.len().saturating_sub(needle.len()))
        .find(|&i| hay[i..i + needle.len()] == needle[..])
        .map_or(0, |i| i + 1)
}

/// `List_Lenght` (M06_Write_Header_SW.bas): the number of words in a blank separated list.
fn list_length(list: &str) -> i64 {
    vba_split(list.trim_matches(' ')).len() as i64
}

/// `To_Eight` (M06_Write_Header_SW.bas)
fn to_eight(count: i64) -> i64 {
    (count + 7) / 8 * 8
}

/// `Make_sure_that_Channel_is_divisible_by_4` (M06_Write_Header_SW.bas)
fn make_channel_divisible_by_4(channel: &mut i64) {
    if *channel % 4 != 0 {
        *channel += 4 - *channel % 4;
    }
}

/// The switch variables of M06_Write_Header_SW.bas (`SwitchA_InpCnt` .. `But_Inp_List_2`), index 0..3 = A..D.
#[derive(Default)]
struct Switches {
    /// `Switch?_InpCnt`: the highest switch number used
    inp_cnt: [i64; 4],
    /// `Switch?_InpLst`: the input pins
    inp_lst: [String; 4],
    clk_pin: String,
    rst_pin: String,
    ldr_pin: String,
    /// `LED_PINNr_List`
    led_pin_list: String,
    read_ldr: bool,
    /// `Switch_Damping_Fact`: the "#define SWITCH_DAMPING_FACT" line
    damping_fact: String,
    /// `CTR_Channels_1/2`, `Channel1/2InpCnt`, `CTR_Cha_Name_1/2`, `But_Inp_List_1/2`
    ctr_channels: [i64; 2],
    channel_inp_cnt: [i64; 2],
    ctr_cha_name: [String; 2],
    but_inp_list: [String; 2],
}

/// `ExpandName` (M06_Write_Header.bas): "$Name" becomes "I<includeCount>_Name". Without `include()`
/// (not supported yet) the include count is always 0.
fn expand_name(name: &str) -> String {
    match name.strip_prefix('$') {
        Some(rest) => format!("I0_{rest}"),
        None => name.to_string(),
    }
}

/// `Is_in_Nr_String` (M06_Write_Header_SW.bas): does `name` start with `expected` (each trailing "?" skips
/// one character) followed by a number? Returns 1 and the number if it is in `min..=max`, -1 if the number is
/// out of range, 0 if it doesn't match (no number, a leading 0, or one of "-+.,eE" in it).
fn is_in_nr_string(name: &str, expected: &str, min: i64, max: i64) -> (i64, i64) {
    let skip = expected.chars().rev().take_while(|&c| c == '?').count();
    let expected = &expected[..expected.len() - skip];
    if !name.starts_with(expected) {
        return (0, 0);
    }
    let nr_str: String = name.chars().skip(expected.chars().count() + skip).collect();
    if !is_numeric(&nr_str) || nr_str.starts_with('0') || nr_str.contains(['-', '+', '.', ',', 'e', 'E']) {
        return (0, 0);
    }
    let nr = val(&nr_str) as i64;
    (if (min..=max).contains(&nr) { 1 } else { -1 }, nr)
}

/// `Get_Arguments` (M06_Write_Header_SW.bas): the trimmed, comma separated arguments between the first
/// "(" and the last ")" after it. `None` where VBA shows "Opening/Closing bracket not found".
fn get_arguments(line: &str) -> Option<Vec<String>> {
    let arguments = line.split('(').nth(1)?;
    let p = arguments.rfind(')')?;
    Some(arguments[..p].split(',').map(|s| s.trim().to_string()).collect())
}

/// `SplitEx(s, True, "OR", "AND", "NOT")` (M30_Tools.bas): doubled delimiters are reduced first, then the
/// text is split at every delimiter. Like VBA's `Replace` this is case sensitive and also splits inside
/// names ("DOOR" -> "DO", "").
fn split_logic(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let delims = ["OR", "AND", "NOT"];
    let mut s = s.to_string();
    for d in delims {
        let dd = format!("{d}{d}");
        // InStr(..., vbTextCompare) finds the doubled delimiter in any case, Replace only the exact case
        while s.to_lowercase().contains(&dd.to_lowercase()) {
            let next = s.replace(&dd, d);
            if next == s {
                break;
            }
            s = next;
        }
    }
    for d in delims {
        s = s.replace(d, "\u{1}");
    }
    s.split('\u{1}').map(|x| x.to_string()).collect()
}

/// `SplitMultiDelims` (M30_Tools.bas): split at any of `delims`, several delimiters in a row count as one,
/// leading ones are skipped.
fn split_multi_delims(text: &str, delims: &str) -> Vec<String> {
    text.split(|c| delims.contains(c)).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
}

/// `Get_Nr_From_Var` (M06_Write_Header_SW.bas, the active `#If 1` version): the number at the end of a
/// name and the length of the text before it. -1 if the name is only a number or empty.
fn get_nr_from_var(name: &str) -> (i64, usize) {
    let chars: Vec<char> = name.chars().collect();
    let mut p = chars.len();
    while p > 0 && chars[p - 1].is_ascii_digit() {
        p -= 1;
    }
    if p == 0 {
        return (-1, 0);
    }
    let tail: String = chars[p..].iter().collect();
    // IsNumeric("") is False: a name without a number at the end gives -2
    if is_numeric(&tail) {
        (val(&tail) as i64, p)
    } else {
        (-2, p)
    }
}

/// `Get_Bin_Inputs` (M06_Write_Header_SW.bas): inputs needed for `dec_cnt` binary states, -1 if invalid.
fn get_bin_inputs(dec_cnt: i64) -> i64 {
    match dec_cnt {
        i64::MIN..=0 => -1,
        1 => 1,
        2..=3 => 2,
        4..=7 => 3,
        8..=15 => 4,
        16..=31 => 5,
        32..=63 => 6,
        _ => -1,
    }
}

/// Excel's `Evaluate()` as `Get_InCh_Number_w_Err_Msg` uses it on a `Logic()` argument whose "#InCh" was
/// replaced by 0: a constant expression with numbers, + - * / and brackets. `None` where Excel's
/// `Evaluate` gives an error (a name or anything else).
fn evaluate_constant(expr: &str) -> Option<f64> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self.i < self.s.len() && self.s[self.i] == b' ' {
                self.i += 1;
            }
        }
        fn expr(&mut self) -> Option<f64> {
            let mut v = self.term()?;
            loop {
                self.ws();
                match self.s.get(self.i) {
                    Some(b'+') => {
                        self.i += 1;
                        v += self.term()?
                    }
                    Some(b'-') => {
                        self.i += 1;
                        v -= self.term()?
                    }
                    _ => return Some(v),
                }
            }
        }
        fn term(&mut self) -> Option<f64> {
            let mut v = self.factor()?;
            loop {
                self.ws();
                match self.s.get(self.i) {
                    Some(b'*') => {
                        self.i += 1;
                        v *= self.factor()?
                    }
                    Some(b'/') => {
                        self.i += 1;
                        let d = self.factor()?;
                        if d == 0.0 {
                            return None;
                        }
                        v /= d
                    }
                    _ => return Some(v),
                }
            }
        }
        fn factor(&mut self) -> Option<f64> {
            self.ws();
            match self.s.get(self.i)? {
                b'-' => {
                    self.i += 1;
                    Some(-self.factor()?)
                }
                b'+' => {
                    self.i += 1;
                    self.factor()
                }
                b'(' => {
                    self.i += 1;
                    let v = self.expr()?;
                    self.ws();
                    (self.s.get(self.i) == Some(&b')')).then(|| {
                        self.i += 1;
                        v
                    })
                }
                _ => {
                    let start = self.i;
                    while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') {
                        self.i += 1;
                    }
                    std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok()
                }
            }
        }
    }
    let mut p = P { s: expr.as_bytes(), i: 0 };
    let v = p.expr()?;
    p.ws();
    (p.i == p.s.len()).then_some(v)
}

/// `Get_First_Number_of_Range`: "12" -> 12, "10 - 12" -> 10, "" -> None, invalid -> Some(-9).
fn first_number_of_range(s: &str) -> Option<i64> {
    let a: String = s.chars().filter(|c| *c != '\n' && *c != ' ').collect();
    if a.is_empty() {
        return None;
    }
    if a.contains('-') {
        let parts: Vec<&str> = a.split('-').collect();
        if parts.len() > 2 || !is_numeric(parts[0]) || !is_numeric(parts[1]) {
            Some(-9)
        } else {
            Some(val(parts[0]).floor() as i64)
        }
    } else if is_numeric(&a) {
        Some(val(&a).floor() as i64)
    } else {
        None
    }
}

/// Features of the Excel tool that v1 doesn't generate yet. Returns a message for the row.
/// A row the first scan skips: it is refused when it is processed.
fn is_refused(row: &Row) -> bool {
    unsupported_feature(row).is_some() || expand_name(row.address.trim()).starts_with("RM ")
}

fn unsupported_feature(row: &Row) -> Option<String> {
    let text = &row.macro_text;
    let checks: [(&str, &str); 8] = [
        ("include(", "Including other sheets (include)"),
        ("LED_to_Var(", "LED_to_Var"),
        ("SOUND_CHANNEL_DEFINITON(", "Serial sound channels"),
        ("Mainboard_LED(", "Mainboard_LED"),
        ("Multiplexer", "Multiplexer"),
        ("Set_LED_OutpPinLst(", "Set_LED_OutpPinLst"),
        ("#define ENABLE_STORE_STATUS", "ENABLE_STORE_STATUS"),
        ("DMX", "DMX output"),
    ];
    for (needle, what) in checks {
        if text.contains(needle) {
            return Some(format!("{what} is not supported by MobaLedLib Studio yet"));
        }
    }
    if row.leds.trim() == SERIAL_CHANNEL_PREFIX {
        return Some("Serial sound channels are not supported by MobaLedLib Studio yet".into());
    }
    None
}

fn tr(catalog: &Catalog, de: &str, lang: usize) -> String {
    catalog.languages.get(de, lang).replace("\r\n", "\n").replace('\r', "\n")
}

fn trf(catalog: &Catalog, de: &str, lang: usize, args: &[&str]) -> String {
    let mut s = tr(catalog, de, lang);
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("#{}#", i + 1), a);
    }
    s
}

/// VBA `Str()` of a number that isn't negative: a leading space.
fn vba_str(n: i64) -> String {
    if n < 0 {
        n.to_string()
    } else {
        format!(" {n}")
    }
}

struct Gen<'a> {
    sheet: &'a Sheet,
    layout: &'a Layout,
    catalog: &'a Catalog,
    page_id: &'static str,
    sx: bool,
    led_numbers: leds::LedNumbers,

    addr_list: Vec<i64>,
    loc_in_ch_nr: i64,
    counter_id: i64,
    ext_addr_txt: String,
    config_txt: String,
    in_ch_txt: String,
    start_values: String,
    store_values_txt: String,
    store_val_written: String,
    alias_names: String,
    /// `DstVar_List` / `MultiSet_DstVar_List`: the variables effects write to, and those written more than once
    dst_var_list: String,
    multi_set_dst_var_list: String,
    /// `Undefined_Input_Var` / `Undef_Input_Var_Row`: input variables not defined when they were checked
    undefined_input_var: String,
    undef_input_var_rows: Vec<usize>,
    /// `MaxUsed_Loc_InCh` / `MaxUsed_Loc_InCh_Row`
    max_used_loc_in_ch: i64,
    max_used_loc_in_ch_row: usize,
    /// A row was refused as not supported yet: the variables it would define are unknown
    refused_rows: bool,
    sw: Switches,
    day_and_night: String,
    channel: i64,
    led_nr: i64,
    addr_comment: String,
    prev_led_channel: usize,
    start_led_channel: [i64; LED_CHANNELS],
    leds_per_channel: [i64; LED_CHANNELS],
    max_channels: usize,

    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
    /// [`GenOptions::language`]
    lang: usize,
}

impl<'a> Gen<'a> {
    fn new(layout: &'a Layout, sheet: &'a Sheet, catalog: &'a Catalog) -> Self {
        let led_numbers = leds::compute(sheet);
        let (leds_per_channel, start_led_channel) = channel_layout(&led_numbers.per_channel);
        let max_channels = (0..LED_CHANNELS).rev().find(|&n| leds_per_channel[n] > 0).unwrap_or(0);
        Gen {
            sheet,
            layout,
            catalog,
            page_id: sheet.protocol.page_id(),
            sx: sheet.protocol == Protocol::Selectrix,
            led_numbers,
            addr_list: Vec::new(),
            loc_in_ch_nr: 0,
            counter_id: 0,
            ext_addr_txt: String::new(),
            config_txt: String::new(),
            in_ch_txt: String::new(),
            start_values: String::new(),
            store_values_txt: String::new(),
            store_val_written: String::new(),
            alias_names: String::new(),
            dst_var_list: " ".into(),
            multi_set_dst_var_list: " ".into(),
            undefined_input_var: String::new(),
            undef_input_var_rows: Vec::new(),
            max_used_loc_in_ch: -1,
            max_used_loc_in_ch_row: 0,
            refused_rows: false,
            sw: Switches::default(),
            day_and_night: String::new(),
            channel: 0,
            led_nr: 0,
            addr_comment: String::new(),
            prev_led_channel: 0,
            start_led_channel,
            leds_per_channel,
            max_channels,
            errors: Vec::new(),
            warnings: Vec::new(),
            lang: crate::lang::ENGLISH,
        }
    }

    /// `Get_Language_Str`: a message of [`crate::gen_text`] in the chosen language; `vbCr` becomes a line end.
    fn tr(&self, de: &str) -> String {
        tr(self.catalog, de, self.lang)
    }

    /// `Replace(Get_Language_Str(de), "#1#", args[0])`, then "#2#", …
    fn trf(&self, de: &str, args: &[&str]) -> String {
        trf(self.catalog, de, self.lang, args)
    }

    fn err(&mut self, row: usize, msg: impl Into<String>) {
        self.errors.push(Diagnostic { row: Some(row), message: msg.into() });
    }

    fn description(&self, r: usize) -> String {
        let d = self.sheet.rows[r].description.trim();
        let d = if d.is_empty() { format!("Row {}", r + 1) } else { d.to_string() };
        d.replace("\r\n", "\n").replace('\n', "| ")
    }

    fn address_exists(&mut self, a: i64) -> bool {
        if self.addr_list.contains(&a) {
            return true;
        }
        self.addr_list.push(a);
        false
    }

    /// Red/Green buttons use two virtual addresses per address, on/off switches both.
    fn address_range_exists(&mut self, addr: i64, cnt: i64, typ: InputType) -> bool {
        let mut m = match typ {
            InputType::Red => 1,
            InputType::Green => 2,
            InputType::OnOff | InputType::Push => 3,
        };
        let mut ad = addr;
        for _ in 0..cnt {
            if m & 1 != 0 && self.address_exists(ad * 2) {
                return true;
            }
            if m & 2 != 0 && self.address_exists(ad * 2 + 1) {
                return true;
            }
            match m {
                1 => m = 2,
                2 => {
                    m = 1;
                    ad += 1
                }
                _ => ad += 1,
            }
        }
        false
    }

    fn define_name(&self, addr: i64, typ: InputType) -> String {
        let suffix: String = typ_const(typ)[1..].replace(',', "");
        if self.sx {
            format!("INCH_SX_{}_{}{}", addr / 8, addr % 8 + 1, suffix)
        } else {
            format!("INCH_{}_{}{}", self.page_id, addr, suffix)
        }
    }

    fn define_lines(&mut self, mut addr: i64, r: usize, typ: InputType, comment: &str) -> String {
        let mut out = String::new();
        let mut typ = typ;
        let mut comment = comment.to_string();
        let mut name = String::new();
        for channel in self.channel..self.channel + self.sheet.rows[r].in_cnt as i64 {
            name = self.define_name(addr, typ);
            out += &format!("#define {}  {}   // {}{NL}", pad(&name, 22), pad(&channel.to_string(), 4), comment);
            if typ != InputType::Red {
                addr += 1;
            }
            typ = next_typ(typ);
            comment = "    \"".into();
        }
        // Alias names defined with "// Define Input(Name)"
        if let Some(rest) = self.sheet.rows[r].macro_text.strip_prefix("// Define Input(") {
            let alias = rest.split(')').next().unwrap_or("");
            if !alias.is_empty() {
                if self.alias_names.is_empty() {
                    self.alias_names = format!("{NL}// Alias definitions defined with 'Define Input()'{NL}");
                }
                self.alias_names += &format!("#define {} {}{NL}", pad(alias, 20), name);
            }
        }
        out
    }

    /// `Find_Macro_in_Lib_Macros_Sheet` (M09_SelectMacro.bas): the first macro whose "Find" text is the line
    /// up to and including its first "(" (`HouseT(` is looked up as `House(`).
    fn find_macro_in_lib(&self, s: &str) -> Option<&'a crate::catalog::MacroDef> {
        let s = s.replace("HouseT(", "House(");
        let key = match s.find('(') {
            Some(p) => format!("{}(", &s[..p]),
            None => s,
        };
        self.catalog.macros.iter().find(|m| !m.find.is_empty() && m.find == key)
    }

    /// `Is_Switch_Var_then_Add_to_Ctr` (M06_Write_Header_SW.bas). 1: a "Switch?<1..250>" name, -1: such a
    /// name with an invalid number, 0: no switch. A switch raises the counter of its keyboard
    /// (`Switch?_InpCnt`) to its number.
    fn is_switch_var(&mut self, name: &str, row: usize) -> i64 {
        match is_in_nr_string(name, "Switch?", 1, 250) {
            (1, nr) => {
                let idx = match name.get(..7) {
                    Some("SwitchA") => Some(0),
                    Some("SwitchB") => Some(1),
                    Some("SwitchC") => Some(2),
                    Some("SwitchD") => Some(3),
                    // Excel only prints "Unsupported variable" to the debug window
                    _ => None,
                };
                if let Some(i) = idx {
                    self.sw.inp_cnt[i] = self.sw.inp_cnt[i].max(nr);
                }
                1
            }
            (-1, _) => {
                self.err(row, self.trf(t::VAR_NR_INVALID, &[name, "1..250"]));
                -1
            }
            _ => 0,
        }
    }

    /// `Valid_Var_Name` (M06_Write_Header_SW.bas): is `name` a known input variable? Names that are not
    /// known yet are noted and checked at the end (`Check_Detected_Variables`). Only an invalid switch
    /// number makes it fail.
    fn valid_var_name(&mut self, name: &str, row: usize) -> bool {
        const STD_NAMES: [&str; 5] = ["SI_1", "SI_0", "SI_Enable_Sound", "SI_LocalVar", "#LocInCh"];
        if STD_NAMES.contains(&name) {
            return true;
        }
        match self.is_switch_var(name, row) {
            1 => return true,
            -1 => return false,
            _ => {}
        }
        if self.dst_var_list.contains(&format!(" {name} ")) {
            return true;
        }
        if self.in_ch_txt.contains(&format!("#define {name} ")) {
            return true;
        }
        match is_in_nr_string(name, "LOC_INCH", 0, 250) {
            (1, nr) => {
                // Checked when the LOC_INCH defines are written
                if nr > self.max_used_loc_in_ch {
                    self.max_used_loc_in_ch = nr;
                    self.max_used_loc_in_ch_row = row;
                }
                return true;
            }
            // Excel shows the message and goes on; the name then stays undefined
            (-1, _) => self.err(row, self.trf(t::VAR_NR_INVALID, &[name, "0..250"])),
            _ => {}
        }
        // "RM ..." names are valid with "#define USE_ATTiny_CAN_GBM" (return messages: not supported yet)
        if !self.undefined_input_var.contains(&format!(" {name} ")) {
            self.undefined_input_var += &format!("{name} ");
            self.undef_input_var_rows.push(row);
        }
        true
    }

    /// `Get_InCh_Number_w_Err_Msg` (M09_SelectMacro.bas): the number added to "#InCh", -1 with a message if
    /// it isn't a constant positive number.
    fn in_ch_number(&mut self, arg: &str, row: usize) -> i64 {
        match evaluate_constant(&arg.replace("#InCh", "0")) {
            Some(v) if vba_round(v) >= 0 => vba_round(v),
            _ => {
                self.err(row, self.tr(t::IN_CH_CONSTANT));
                -1
            }
        }
    }

    /// `Valid_Var_Name_and_Skip_InCh_and_Numbers` (M06_Write_Header_SW.bas)
    fn valid_var_name_skip_in_ch_and_numbers(&mut self, arg: &str, row: usize) -> bool {
        for sub in split_multi_delims(arg, " +-") {
            if sub == "#InCh" {
                if self.in_ch_number(arg, row) < 0 {
                    return false;
                }
            } else if !is_numeric(&sub) && !self.valid_var_name(&sub, row) {
                return false;
            }
        }
        true
    }

    /// `Check_if_all_Variables_in_sequece_of_N_exists` (M06_Write_Header_SW.bas): an effect with `n` inputs
    /// switched by a variable "Name5" also reads "Name6" … "Name<5+n-1>".
    fn check_sequence_of_n(&mut self, r: usize, n: i64) -> bool {
        let adr_or_name = self.sheet.rows[r].address.trim().to_string();
        if adr_or_name.is_empty() {
            self.err(r, self.trf(t::NO_ADDRESS, &[&(r + 1).to_string()]));
            return false;
        }
        if !is_numeric(adr_or_name.split(' ').next().unwrap_or("")) {
            let adr_or_name = expand_name(&adr_or_name);
            let (nr, txt_len) = get_nr_from_var(&adr_or_name);
            if nr >= 0 {
                let name: String = adr_or_name.chars().take(txt_len).collect();
                for i in nr..nr + n {
                    if !self.valid_var_name(&format!("{name}{i}"), r) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// `Check_if_all_Variables_in_sequece_exist` (M06_Write_Header_SW.bas)
    fn check_sequence(&mut self, r: usize, ctr_name: &str, n_str: &str) -> bool {
        let n = if ctr_name == "BinStates" {
            let n = get_bin_inputs(val_i(n_str));
            if n <= 0 {
                self.err(r, self.tr(t::BIN_STATES));
                return false;
            }
            n
        } else {
            val_i(n_str)
        };
        self.check_sequence_of_n(r, n)
    }

    /// `Add_Logic_InpVars` (M06_Write_Header_SW.bas)
    fn add_logic_inp_vars(&mut self, logic_exp: &str, r: usize) -> bool {
        for arg in split_logic(logic_exp) {
            let arg = arg.trim();
            if !arg.is_empty() && !self.valid_var_name_skip_in_ch_and_numbers(arg, r) {
                return false;
            }
        }
        true
    }

    /// `Add_InpVars` (M06_Write_Header_SW.bas): check the input variables an effect line reads.
    fn add_inp_vars(&mut self, macro_name: &str, def: &crate::catalog::MacroDef, line: &str, r: usize) -> bool {
        const SECOND_INPUT_NAMES: [&str; 4] = ["InCh", "R_InCh", "InReset", "InCh2"];
        let (Some(arg_list), Some(fil_list)) = (get_arguments(&def.prototype), get_arguments(line)) else {
            return true;
        };
        if macro_name == "Logic" {
            if fil_list.len() != 2 {
                self.err(r, self.trf(t::LOGIC_PARAMS, &[line]));
                return false;
            }
            return self.add_logic_inp_vars(&fil_list[1], r);
        }
        let mut det_cnt = 0;
        let mut pos_counter_var = None;
        for (arg_nr, arg) in arg_list.iter().enumerate() {
            if matches!(arg.as_str(), "InCh_Cnt" | "States" | "BinStates") {
                pos_counter_var = Some(arg_nr);
            }
            if SECOND_INPUT_NAMES.contains(&arg.as_str()) {
                det_cnt += 1;
                // VBA stops with a run-time error when the line has fewer arguments than the macro
                let Some(fil) = fil_list.get(arg_nr) else {
                    self.err(r, format!("Too few parameters in '{line}'"));
                    return false;
                };
                if !self.valid_var_name_skip_in_ch_and_numbers(fil, r) {
                    return false;
                }
            }
        }
        if let Some(pos) = pos_counter_var {
            let n_str = fil_list.get(pos).cloned().unwrap_or_default();
            return self.check_sequence(r, &arg_list[pos], &n_str);
        }
        let in_cnt_str = def.in_cnt.as_str();
        if !in_cnt_str.is_empty() {
            if is_numeric(in_cnt_str) && det_cnt != val_i(in_cnt_str) && !self.check_sequence_of_n(r, val_i(in_cnt_str))
            {
                return false;
            }
            if let Some(head) = in_cnt_str.strip_suffix('?') {
                if is_numeric(head)
                    && fil_list.get(2).is_some_and(|a| a == "#InCh+1")
                    && !self.check_sequence_of_n(r, val_i(in_cnt_str))
                {
                    return false;
                }
            }
        }
        true
    }

    /// `Get_Matching_Arg` (M06_Write_Header_SW.bas): the argument of `line` at the place of `dest_var_name`
    /// in the macro's prototype; for "..." and "OutList" all arguments from there on, joined by ",".
    /// `None` (with Excel's message) if it isn't found.
    fn get_matching_arg(&mut self, org_macro: &str, line: &str, dest_var_name: &str, r: usize) -> Option<String> {
        if let (Some(org_args), Some(act_args)) = (get_arguments(org_macro), get_arguments(line)) {
            // Org_Args may end with "...": the line may have more arguments
            if act_args.len() >= org_args.len() {
                if let Some(i) = org_args.iter().position(|a| a == dest_var_name) {
                    let arg = match dest_var_name {
                        "..." | "OutList" => act_args[i..].join(","),
                        _ => act_args[i].clone(),
                    };
                    return Some(expand_name(&arg));
                }
            }
        }
        self.err(r, self.trf(t::DST_VAR_DETECT, &[line]));
        None
    }

    /// `Add_Variable_to_DstVar_List` (M06_Write_Header_SW.bas): a variable set twice goes to
    /// `MultiSet_DstVar_List`.
    fn add_variable_to_dst_var_list(&mut self, var_name: &str) {
        let check = format!(" {var_name} ");
        if !self.dst_var_list.contains(&check) {
            self.dst_var_list += &format!("{var_name} ");
        } else if !self.multi_set_dst_var_list.contains(&check) {
            self.multi_set_dst_var_list += &format!("{var_name} ");
        }
    }

    /// `Add_Matching_Arg_to_DstVars` (M06_Write_Header_SW.bas). Ex.: MonoFlop(DstVar, InCh, Duration)
    fn add_matching_arg_to_dst_vars(&mut self, org_macro: &str, line: &str, dest_var_name: &str, r: usize) -> bool {
        match self.get_matching_arg(org_macro, line, dest_var_name, r).as_deref() {
            None | Some("") => false,
            // "#LocInCh": the random goto activation; "SI_LocalVar" may be used again
            Some("#LocInCh") | Some("SI_LocalVar") => true,
            Some(arg) => {
                self.add_variable_to_dst_var_list(arg);
                true
            }
        }
    }

    /// `Add_N2_Arg_to_DstVars` (M06_Write_Header_SW.bas). Ex.: RandMux(DstVar1, DstVarN, InCh, RandMode, ...)
    fn add_n2_arg_to_dst_vars(&mut self, org_macro: &str, line: &str, r: usize) -> bool {
        let arg1 = match self.get_matching_arg(org_macro, line, "DstVar1", r) {
            Some(a) if !a.is_empty() => a,
            _ => return false,
        };
        let arg_n = match self.get_matching_arg(org_macro, line, "DstVarN", r) {
            Some(a) if !a.is_empty() => a,
            _ => return false,
        };
        let (start_nr, txt_len1) = get_nr_from_var(&arg1);
        if start_nr < 0 {
            return false;
        }
        let (end_nr, txt_len_n) = get_nr_from_var(&arg_n);
        if end_nr < 0 {
            return false;
        }
        let name: String = arg1.chars().take(txt_len1).collect();
        if txt_len1 != txt_len_n || name != arg_n.chars().take(txt_len_n).collect::<String>() {
            return false;
        }
        for i in start_nr..=end_nr {
            self.add_variable_to_dst_var_list(&format!("{name}{i}"));
        }
        true
    }

    /// `Add_VarArgCnt_to_DstVars` (M06_Write_Header_SW.bas). Ex.: Counter(CtrMode, InCh, Enable, TimeOut, ...)
    fn add_var_arg_cnt_to_dst_vars(&mut self, org_macro: &str, line: &str, r: usize) -> bool {
        // A Counter with CF_ONLY_LOCALVAR has no destination variables
        if org_macro.starts_with("Counter(") && line.contains("CF_ONLY_LOCALVAR") {
            return true;
        }
        match self.get_matching_arg(org_macro, line, "OutList", r) {
            Some(arg) if !arg.is_empty() => {
                for name in arg.split(',') {
                    self.add_variable_to_dst_var_list(name);
                }
                true
            }
            _ => false,
        }
    }

    /// `Add_Cx_to_DstVars` (M06_Write_Header_SW.bas). Ex.: PushButton_w_LED_0_2(B_LED, B_LED_Cx, InCh, DstVar1, ...)
    fn add_cx_to_dst_vars(&mut self, org_macro: &str, line: &str, cnt: i64, r: usize) -> bool {
        let arg1 = match self.get_matching_arg(org_macro, line, "DstVar1", r) {
            Some(a) if !a.is_empty() => a,
            _ => return false,
        };
        let (start_nr, txt_len) = get_nr_from_var(&arg1);
        if start_nr < 0 {
            self.err(r, self.trf(t::DST_VAR_SEQUENCE, &[&arg1, &(r + 1).to_string(), &format!("{arg1}0")]));
            return false;
        }
        let name: String = arg1.chars().take(txt_len).collect();
        for i in start_nr..start_nr + cnt {
            self.add_variable_to_dst_var_list(&format!("{name}{i}"));
        }
        true
    }

    /// `Add_Inp_and_DstVars` (M06_Write_Header_SW.bas): check the input variables of an effect line and
    /// note the variables it sets.
    fn add_inp_and_dst_vars(&mut self, line: &str, r: usize) -> bool {
        if !line.contains('(') {
            return true;
        }
        let parts: Vec<&str> = line.split('(').collect();
        if !parts[1].contains(')') {
            return true;
        }
        let search = if parts[0] == "HouseT" { "House" } else { parts[0] };
        // A macro that isn't in Lib_Macros (an own one): its destination variables can't be detected
        let Some(def) = self.find_macro_in_lib(&format!("{search}(")) else { return true };
        if !self.add_inp_vars(parts[0], def, line, r) {
            return false;
        }
        let org_macro = def.prototype.as_str();
        let out_cnt = def.out_cnt.as_str();
        let res = match out_cnt {
            "" | "0" => return true,
            "1" => self.add_matching_arg_to_dst_vars(org_macro, line, "DstVar", r),
            "2" => {
                self.add_matching_arg_to_dst_vars(org_macro, line, "DstVar1", r)
                    && self.add_matching_arg_to_dst_vars(org_macro, line, "DstVar2", r)
            }
            "n.." => self.add_var_arg_cnt_to_dst_vars(org_macro, line, r),
            "n2" => self.add_n2_arg_to_dst_vars(org_macro, line, r),
            _ => match out_cnt.strip_prefix('C') {
                // "C" followed by a non number: Res stays False
                Some(n) => is_numeric(n) && self.add_cx_to_dst_vars(org_macro, line, val_i(n), r),
                None => {
                    self.err(r, format!("Internal error: undefined OutCnt entry '{out_cnt}' of macro '{}'", def.name));
                    return false;
                }
            },
        };
        if !res {
            self.err(r, self.trf(t::DST_VAR_DEFINITION, &[line, &(r + 1).to_string()]));
        }
        res
    }

    /// `Get_Current_Platform_String` (M30_Tools.bas): a value starting with "=" refers to another parameter.
    fn platform_str(&self, key: &str) -> String {
        let v = self.catalog.platform(self.layout.board.platform.id(), key).unwrap_or("").to_string();
        match v.strip_prefix('=') {
            Some(other) => self.platform_str(other),
            None => v,
        }
    }

    /// `Get_Current_Platform_Bool` (M30_Tools.bas)
    fn platform_bool(&self, key: &str) -> bool {
        let v = self.platform_str(key);
        v.to_uppercase() == "TRUE" || v == "1"
    }

    /// `AliasToPin` (M30_Tools.bas): a pin alias of the platform ("PIN_ALIAS_<name>") or the pin itself.
    fn alias_to_pin(&self, pin: &str) -> String {
        let alias = self.platform_str(&format!("PIN_ALIAS_{}", pin.to_uppercase()));
        if alias.is_empty() {
            pin.to_string()
        } else {
            alias
        }
    }

    /// `Set_PinNrLst_if_Matching` (M06_Write_Header_SW.bas): if `line` is the command `name`, its pins
    /// (aliases mapped to the physical pins). `Err` with Excel's message if they are invalid.
    fn pin_nr_lst_if_matching(
        &self,
        line: &str,
        name: &str,
        pin_typ: &str,
        max_cnt: usize,
    ) -> Result<Option<String>, String> {
        // On the AM328 the SPI pins are only free if no CAN module is connected
        let mut valid_pins = String::new();
        if self.page_id != "CAN" && ["I", "O", "OV", "Pu"].contains(&pin_typ) {
            valid_pins = self.platform_str("SPI_Pins");
        }
        valid_pins += &self.platform_str(&format!("{pin_typ}_Pins"));
        if !line.starts_with(name) {
            return Ok(None);
        }
        let print_error = || self.trf(t::PIN_READ, &[line]);
        let Some(p) = line.find(')').filter(|&p| p >= name.len()) else { return Err(print_error()) };
        let nr_str = line[name.len()..p].replace(',', " ").trim_matches(' ').replace("  ", " ");
        if nr_str.is_empty() {
            return Err(print_error());
        }
        let nr_arr: Vec<&str> = nr_str.split(' ').collect();
        if nr_arr.len() > max_cnt {
            return Err(print_error());
        }
        let command = line.replace("// ", "");
        let mut list = String::new();
        for one_pin in nr_arr {
            let pin = self.alias_to_pin(one_pin);
            if !valid_pins.contains(&format!(" {pin} ")) {
                return Err(self.trf(t::PIN_INVALID, &[one_pin, &command]));
            }
            // A virtual pin may be used more than once
            if one_pin != VIRTUAL_CHANNEL {
                let needle = format!(" {one_pin} ");
                let p = vba_instr(1, &format!(" {} ", line.replace(['(', ')'], " ")), &needle);
                if vba_instr(p + 1, &format!(" {line} "), &needle) > 0 {
                    return Err(self.trf(t::PIN_TWICE, &[one_pin, &command]));
                }
            }
            if !list.is_empty() {
                list.push(' ');
            }
            list += &pin;
        }
        Ok(Some(list))
    }

    /// `Init_HeaderFile_Generation_SW` with `First_Scan_of_Data_Rows` (M06_Write_Header_SW.bas): before
    /// the rows are processed, the switches, the pin commands and the variables of every row are noted.
    /// Like Excel, the first error stops the generation (`false`). Rows that are refused as not supported
    /// yet are skipped: they don't reach the header.
    fn first_scan(&mut self) -> bool {
        self.sw.led_pin_list = self.platform_str("LED_Pins");
        self.sw.ldr_pin = self.platform_str("LDR_Pin");
        for (i, key) in ["SwitchA_Pins", "SwitchB_Pins", "SwitchC_Pins", "SwitchD_Pins"].iter().enumerate() {
            self.sw.inp_lst[i] = self.platform_str(key);
        }
        self.sw.clk_pin = self.platform_str("CLK_Pin");
        self.sw.rst_pin = self.platform_str("RST_Pin");

        let sheet = self.sheet;
        for (r, row) in sheet.rows.iter().enumerate() {
            if !row.enabled || is_refused(row) {
                continue;
            }
            if self.is_switch_var(&row.address, r) == -1 {
                return false;
            }
            for line in row.macro_text.split('\n') {
                let line = line.trim();
                // (command, list index or pin, pin type, max count)
                const PIN_CMDS: [(&str, usize, &str, usize); 7] = [
                    ("// Set_SwitchA_InpLst(", 0, "A", 5),
                    ("// Set_SwitchB_InpLst(", 1, "I", 12),
                    ("// Set_SwitchC_InpLst(", 2, "I", 12),
                    ("// Set_SwitchD_InpLst(", 3, "Pu", 12),
                    ("// Set_CLK_Pin_Number(", 4, "O", 1),
                    ("// Set_RST_Pin_Number(", 5, "O", 1),
                    ("// Set_LDR_Pin_Number(", 6, "A", 1),
                ];
                for (cmd, dest, typ, max) in PIN_CMDS {
                    match self.pin_nr_lst_if_matching(line, cmd, typ, max) {
                        Ok(None) => {}
                        Ok(Some(list)) => match dest {
                            0..=3 => self.sw.inp_lst[dest] = list,
                            4 => self.sw.clk_pin = list,
                            5 => self.sw.rst_pin = list,
                            _ => self.sw.ldr_pin = list,
                        },
                        Err(msg) => {
                            self.err(r, msg);
                            return false;
                        }
                    }
                }
                if line == "#define READ_LDR" {
                    self.sw.read_ldr = true;
                }
                if line.starts_with("#define SWITCH_DAMPING_FACT") {
                    self.sw.damping_fact = line.to_string();
                }
                if !self.add_inp_and_dst_vars(line, r) {
                    return false;
                }
            }
        }
        // Init_HeaderFile_Generation_SW asks whether to cancel; the header can still be written
        if self.multi_set_dst_var_list != " " {
            self.warnings.push(Diagnostic {
                row: None,
                message: format!("{}\n{}", self.tr(t::MULTI_SET_DST_VARS), self.multi_set_dst_var_list),
            });
        }
        if !self.no_duplicates_in_inp_lists() {
            return false;
        }
        self.sw.but_inp_list = ["Unused".into(), "Unused".into()];
        let [_, b, c, _] = self.sw.inp_cnt;
        let lst_b = self.sw.inp_lst[1].clone();
        let lst_c = self.sw.inp_lst[2].clone();
        let ctr = |cnt: i64, lst: &str| (cnt as f64 / vba_split(lst).len() as f64).ceil() as i64;
        if b > 0 && c > 0 && !lst_b.is_empty() && !lst_c.is_empty() {
            self.sw.ctr_cha_name = ["SwitchB".into(), "SwitchC".into()];
            self.sw.ctr_channels = [ctr(b, &lst_b), ctr(c, &lst_c)];
            self.sw.but_inp_list = [lst_b, lst_c];
            self.sw.channel_inp_cnt = [b, c];
        } else if b > 0 && !lst_b.is_empty() {
            self.sw.ctr_cha_name = ["SwitchB".into(), "Unused".into()];
            self.sw.ctr_channels[0] = ctr(b, &lst_b);
            self.sw.but_inp_list[0] = lst_b;
            self.sw.channel_inp_cnt[0] = b;
        } else if c > 0 && !lst_c.is_empty() {
            self.sw.ctr_cha_name = ["SwitchC".into(), "Unused".into()];
            self.sw.ctr_channels[0] = ctr(c, &lst_c);
            self.sw.but_inp_list[0] = lst_c;
            self.sw.channel_inp_cnt[0] = c;
        }
        true
    }

    /// `No_Duplicates_in_two_InpLists` (M06_Write_Header_SW.bas)
    fn no_duplicates_in_two_inp_lists(
        &mut self,
        letter1: &str,
        letter2_or_name: &str,
        inp_lst1: &str,
        inp_lst2: &str,
        set_funct2: &str,
    ) -> bool {
        let inp_lst2 = format!(" {inp_lst2} ");
        for pin in vba_split(inp_lst1) {
            if inp_lst2.contains(&format!(" {pin} ")) {
                let message = if letter2_or_name.chars().count() == 1 {
                    self.trf(t::PIN_IN_TWO_SWITCHES, &[pin, letter1, letter2_or_name])
                } else {
                    self.trf(t::PIN_SWITCH_AND_OTHER, &[pin, letter1, letter2_or_name, set_funct2])
                };
                self.errors.push(Diagnostic { row: None, message });
                return false;
            }
        }
        true
    }

    /// `Check_CLK_a_RST_Pin_Usage` (M06_Write_Header_SW.bas)
    fn check_clk_a_rst_pin_usage(&mut self, letter1: &str, inp_lst1: &str) -> bool {
        if self.sw.inp_cnt[1] > 0 || self.sw.inp_cnt[2] > 0 {
            let rst = self.sw.rst_pin.clone();
            let clk = self.sw.clk_pin.clone();
            let name = self.trf(t::PIN_FOR_SWITCH_B_C, &["Reset"]);
            if !self.no_duplicates_in_two_inp_lists(letter1, &name, inp_lst1, &rst, "Set_RST_Pin_Number()") {
                return false;
            }
            let name = self.trf(t::PIN_FOR_SWITCH_B_C, &["Clock"]);
            if !self.no_duplicates_in_two_inp_lists(letter1, &name, inp_lst1, &clk, "Set_CLK_Pin_Number()") {
                return false;
            }
        }
        true
    }

    /// `No_Duplicates_in_InpLists` (M06_Write_Header_SW.bas). The checks against `Serial_PinLst` are left
    /// out: serial sound channels are refused, so the list is always empty.
    fn no_duplicates_in_inp_lists(&mut self) -> bool {
        const LED: &str = "Set_LED_OutpPinLst()";
        const LDR: &str = "Set_LDR_Pin_Number()";
        let letters = ["A", "B", "C", "D"];
        let cnt = self.sw.inp_cnt;
        let lst = self.sw.inp_lst.clone();
        let led = self.sw.led_pin_list.clone();
        let ldr = self.sw.ldr_pin.clone();
        for i in 0..4 {
            if cnt[i] == 0 {
                continue;
            }
            for j in i + 1..4 {
                if cnt[j] > 0 && !self.no_duplicates_in_two_inp_lists(letters[i], letters[j], &lst[i], &lst[j], LED) {
                    return false;
                }
            }
            if !self.no_duplicates_in_two_inp_lists(letters[i], "LED", &lst[i], &led, LED) {
                return false;
            }
            if self.sw.read_ldr && !self.no_duplicates_in_two_inp_lists(letters[i], "LDR", &lst[i], &ldr, LDR) {
                return false;
            }
            if !self.check_clk_a_rst_pin_usage(letters[i], &lst[i]) {
                return false;
            }
        }
        true
    }

    /// `PIN_A3_Is_Used` (M06_Write_Header_SW.bas)
    fn pin_a3_is_used(&self) -> bool {
        self.sw.channel_inp_cnt[0] > 0 && self.sw.rst_pin == "A3"
    }

    /// `Check_Switch_Lists_for_SPI_Pins` (M06_Write_Header_SW.bas)
    fn check_switch_lists_for_spi_pins(&mut self) -> bool {
        for i in 0..4 {
            if self.sw.inp_cnt[i] > 0 {
                let list = format!(" {} ", self.sw.inp_lst[i]);
                for pin in ["10", "11", "12"] {
                    if list.contains(&format!(" {pin} ")) {
                        self.errors.push(Diagnostic { row: None, message: self.trf(t::SPI_PIN, &[pin]) });
                        return false;
                    }
                }
            }
        }
        true
    }

    /// `Find_Alias_Name_In_Sheet` (M06_Write_Header_SW.bas): an enabled row whose effect cell is
    /// "// Define Input(<name>)". Excel's `Match` compares the whole cell, ignoring case.
    fn find_alias_name_in_sheet(&self, name: &str) -> bool {
        let wanted = format!("// Define Input({name})").to_lowercase();
        self.sheet.rows.iter().any(|row| row.enabled && row.macro_text.to_lowercase() == wanted)
    }

    /// `Check_Detected_Variables` (M06_Write_Header_SW.bas): every input variable must be set somewhere.
    /// Like Excel, only the first undefined one is reported.
    fn check_detected_variables(&mut self) {
        if self.refused_rows {
            return;
        }
        let list = self.undefined_input_var.trim().to_string();
        if list.is_empty() {
            return;
        }
        for (nr, var) in list.split(' ').enumerate() {
            let found = self.in_ch_txt.contains(&format!(" {var} "))
                || self.dst_var_list.contains(&format!("{var} "))
                || var == "[Multiplexer]"
                || self.find_alias_name_in_sheet(var);
            if !found {
                let row = self.undef_input_var_rows[nr];
                self.err(row, self.trf(t::INPUT_NEVER_SET, &[var]));
                return;
            }
        }
    }

    fn macro_store_type(&self, macro_text: &str) -> i64 {
        for line in macro_text.split('\n') {
            if line.trim().is_empty() || line.split('(').next().unwrap_or("").trim().is_empty() {
                continue;
            }
            if let Some(m) = self.catalog.find_macro(line) {
                let t = val_i(&m.store_type);
                if t != 0 {
                    return t;
                }
            }
            if !macro_text.contains('\n') {
                break;
            }
        }
        0
    }

    fn store_status(&mut self, r: usize, addr: &Addr, typ: Option<InputType>, cod: &str) -> Option<StoreStatus> {
        let sheet = self.sheet;
        let row = &sheet.rows[r];
        let sv = row.start_value.trim();
        let sv_numeric = !sv.is_empty() && is_numeric(sv);
        let status = match self.macro_store_type(&row.macro_text) {
            MST_CTR_OFF => StoreStatus::CounterOff,
            MST_CTR_ON => StoreStatus::CounterOn,
            MST_PREVENT_STORE => StoreStatus::None,
            _ => {
                // GetOnOffStoreType
                let mut cod = cod.to_string();
                let mut tc = "";
                if let Some(t) = typ {
                    tc = typ_const(t);
                    if let Addr::Num(a) = addr {
                        cod = self.define_name(*a, t);
                    }
                }
                if cod.is_empty() {
                    StoreStatus::None
                } else if matches!(addr, Addr::Num(_)) && tc == "S_ONOFF," {
                    StoreStatus::OnOff
                } else if row.in_cnt > 1 && self.led_numbers.rows[r].is_some() {
                    StoreStatus::Trigger
                } else {
                    StoreStatus::None
                }
            }
        };
        Some(match status {
            StoreStatus::CounterOff if sv == "*" => StoreStatus::CounterOn,
            StoreStatus::CounterOn if sv == "0" || sv_numeric => StoreStatus::CounterOff,
            StoreStatus::OnOff | StoreStatus::Trigger if sv == "0" || sv_numeric => StoreStatus::None,
            StoreStatus::None if sv == "*" => {
                self.err(r, format!("{}{sv}{}", self.tr(t::ENTRY), self.tr(t::IN_START_COLUMN)));
                return None;
            }
            s => s,
        })
    }

    fn start_value_entry(&mut self, r: usize) {
        let sheet = self.sheet;
        let row = &sheet.rows[r];
        let sv = val_i(&row.start_value);
        if sv == 0 {
            return;
        }
        if sv < 0 {
            self.err(r, format!("{}{}", self.tr(t::NEGATIVE_START), r + 1));
        }
        let mut desc = self.description(r);
        let mut mask: i64 = 1;
        for i in 0..row.in_cnt as i64 {
            if sv & mask != 0 {
                self.start_values += &format!(
                    "{} // {}{NL}",
                    pad(&format!("  MobaLedLib.Set_Input({}, 1);", self.channel + i), 109),
                    desc
                );
                desc = "   \"".into();
            }
            mask *= 2;
        }
        if sv > mask - 1 {
            self.err(r, format!("{}{}{}{}", self.tr(t::START_IN_ROW), r + 1, self.tr(t::START_TOO_BIG), mask - 1));
        }
    }

    fn activate_day_and_night_timer(&mut self, cmd: &str) {
        let inner = cmd.replace("DayAndNightTimer(", "").replace(')', "");
        let args: Vec<&str> = inner.trim().split(',').collect();
        let period = val(args.get(1).unwrap_or(&"0").trim());
        self.day_and_night =
            format!("{NL}#define DayAndNightTimer_Period    {}{NL}", vba_round(period * 60.0 * 1000.0 / 512.0));
        let inch = args.first().unwrap_or(&"").trim();
        if inch != "SI_1" {
            self.day_and_night += &format!("#define DayAndNightTimer_InCh      {inch}{NL}");
        }
    }

    fn config_line(&mut self, r: usize, led_nr: i64, cod: &str, addr: &Addr) -> Option<String> {
        let sheet = self.sheet;
        let row = &sheet.rows[r];
        let txt = row.macro_text.replace("\r\n", "\n");
        if txt.trim().is_empty() {
            return Some(String::new());
        }
        let description = self.description(r);
        let mut add_description = !description.is_empty();
        let mut res = String::new();
        let mut inc_loc_in_ch = false;
        for line in txt.split('\n') {
            let (mut cmd, comment) = match line.find("//") {
                None => (line.to_string(), String::new()),
                Some(0) => (String::new(), line.to_string()),
                // Like VBA Left(line, CommentStart): the first '/' stays with the command
                Some(p) => (line[..=p].to_string(), line[p + 1..].to_string()),
            };
            if cmd.contains('(') && cmd.contains('$') {
                for a in call_args(&cmd) {
                    if let Some(rest) = a.strip_prefix('$') {
                        let expanded = format!("I0_{rest}");
                        cmd = cmd.replace(&format!("{a})"), &format!("{expanded})"));
                        cmd = cmd.replace(&format!("{a},"), &format!("{expanded},"));
                    }
                }
            }
            if led_nr < 0 {
                self.err(r, self.tr(t::NEGATIVE_LED));
                return None;
            }
            if cmd.contains("#LED") && row.leds.trim().is_empty() && !self.warnings.iter().any(|w| w.row == Some(r)) {
                // It gets the LED number of the next effect on the bus
                self.warnings.push(Diagnostic {
                    row: Some(r),
                    message: format!(
                        "Row {}: the effect uses an LED but its LEDs field is empty; choose the effect again with Change…",
                        r + 1
                    ),
                });
            }
            cmd = cmd.replace("#LED", &led_nr.to_string());
            cmd = match addr {
                Addr::None => cmd.replace("#InCh", "SI_1"),
                _ => cmd.replace("#InCh", cod),
            };
            if cmd.contains("#LocInCh") {
                if row.loc_in_ch == 0 {
                    self.err(r, "'#LocInCh' is used but 'Loc InCh' is 0");
                    return None;
                }
                cmd = cmd.replace("#LocInCh", &format!("LOC_INCH{}", self.loc_in_ch_nr));
                inc_loc_in_ch = true;
            }
            if cmd.starts_with("DayAndNightTimer(") {
                self.activate_day_and_night_timer(&cmd);
                cmd = format!("// {cmd}");
            }
            let add_backslash = cmd.trim_end().ends_with('\\');
            if add_backslash {
                let t = cmd.trim_end();
                cmd = t[..t.len() - 1].to_string();
            }
            let mut c = format!("  {cmd}{comment}");
            if add_description {
                c = format!("{} /* {}", pad(&c, 109), description);
            } else if !description.is_empty() {
                c = format!("{} /*     \"", pad(&c, 109));
            }
            c = format!("{} */", pad(&c, 300));
            if add_backslash {
                c += " \\";
            }
            add_description = false;
            res += &c;
            res += NL;
        }
        if inc_loc_in_ch {
            self.loc_in_ch_nr += row.loc_in_ch as i64;
        }
        Some(res)
    }

    fn header_entry(&mut self, r: usize, mut addr: Addr) {
        let sheet = self.sheet;
        let row = &sheet.rows[r];
        let comment = self.description(r);
        let in_cnt = row.in_cnt as i64;
        let mut cod = String::new();
        let mut typ: Option<InputType> = None;

        match addr.clone() {
            Addr::Num(a) => {
                let Some(t) = row.input_type else {
                    self.err(r, "The input type (Red/Green/OnOff/Push) is missing");
                    return;
                };
                typ = Some(t);
                if self.address_range_exists(a, in_cnt, t) {
                    cod = self.define_name(a, t);
                    if !self.in_ch_txt.contains(&cod) {
                        self.err(
                            r,
                            format!(
                                "{}{a}{}{}{}",
                                self.tr(t::THE_ADDRESS),
                                self.tr(t::IN_ROW),
                                r + 1,
                                self.tr(t::OTHER_TYPE)
                            ),
                        );
                    }
                    addr = Addr::Name(cod.clone());
                } else {
                    cod = self.channel.to_string();
                }
            }
            Addr::Name(n) => cod = n,
            Addr::None => {}
        }

        let led_channel = match row.led_channel {
            None => self.prev_led_channel,
            Some(c) => {
                self.prev_led_channel = c as usize;
                c as usize
            }
        };
        if led_channel >= LED_CHANNELS {
            self.err(r, self.trf(t::LED_CHANNEL, &[&(r + 1).to_string(), &vba_str(LED_CHANNELS as i64 - 1)]));
            return;
        }
        if let Some(n) = self.led_numbers.rows[r] {
            self.led_nr = n + self.start_led_channel[led_channel];
        }

        let led_nr = self.led_nr;
        let Some(res) = self.config_line(r, led_nr, &cod, &addr) else { return };
        self.config_txt += &res;

        let store_type = self.macro_store_type(&row.macro_text);
        if matches!(store_type, MST_CTR_NONE | MST_CTR_ON | MST_CTR_OFF) {
            self.counter_id += 1;
        }
        let Some(status) = self.store_status(r, &addr, typ, &cod) else { return };

        if status != StoreStatus::None {
            if let (Some(t), Addr::Num(a)) = (typ, &addr) {
                cod = self.define_name(*a, t);
            }
            if matches!(status, StoreStatus::OnOff | StoreStatus::Trigger)
                && !self.store_val_written.contains(&format!(" {cod} "))
            {
                let kind = if status == StoreStatus::OnOff { "IS_TOGGLE + " } else { "IS_PULSE  + " };
                if in_cnt > 63 {
                    self.err(r, self.trf(t::STORE_TOO_MANY, &[&(r + 1).to_string()]));
                }
                self.store_values_txt += &format!(
                    "           {{ {kind}{}, {}}},      // {comment}{NL}",
                    pad(&in_cnt.to_string(), 2),
                    pad(&cod, 20)
                );
                self.store_val_written += &format!(" {cod} ");
            }
        }
        if status == StoreStatus::CounterOn {
            self.store_values_txt += &format!(
                "           {{ IS_COUNTER    , {}}},      // {comment}{NL}",
                pad(&format!("COUNTER_ID {}", self.counter_id), 20)
            );
        }

        if let (Addr::Num(a), Some(t)) = (&addr, typ) {
            let a = *a;
            let defines = self.define_lines(a, r, t, &comment);
            self.in_ch_txt += &defines;
            let mut line = format!(
                "           {{ {}+ {}{}}},      // ",
                pad(&a.to_string(), 5),
                pad(typ_const(t), 11),
                pad(&in_cnt.to_string(), 2)
            );
            if !self.addr_comment.is_empty() {
                line += &pad(&self.addr_comment, 10);
            }
            line += &comment;
            line += NL;
            self.ext_addr_txt += &line;
            self.start_value_entry(r);
            if in_cnt > 100 {
                self.err(r, format!("{}{in_cnt}{}", self.tr(t::ENTRY), self.tr(t::IN_IN_CNT_COLUMN)));
            } else {
                self.channel += in_cnt;
            }
        }
    }

    fn process_row(&mut self, r: usize) {
        let sheet = self.sheet;
        let row = &sheet.rows[r];
        if !row.enabled {
            return;
        }
        if let Some(msg) = unsupported_feature(row) {
            self.refused_rows = true;
            self.err(r, msg);
            return;
        }
        let address = row.address.trim().to_string();
        let mut addr = Addr::None;
        if address.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            if self.sx {
                let bit_p = row.bit_pos.trim();
                if !bit_p.is_empty() && row.in_cnt > 0 {
                    let sx_ch = first_number_of_range(&address).unwrap_or(0);
                    if (0..=99).contains(&sx_ch) {
                        let bp = val_i(bit_p);
                        if (1..=8).contains(&bp) {
                            addr = Addr::Num(sx_ch * 8 + bp - 1);
                            self.addr_comment = format!("SX {},{}: ", pad_left(&sx_ch.to_string(), 2), bit_p);
                        } else {
                            self.err(r, format!("{}{}", t::WRONG_BIT_POS, r + 1));
                        }
                    } else {
                        self.err(r, format!("{}{}", t::WRONG_SX_CHANNEL, r + 1));
                    }
                }
            } else {
                let max_addr = match self.sheet.protocol {
                    Protocol::Dcc => 10240,
                    Protocol::LNet => 2048,
                    _ => 65535,
                };
                match first_number_of_range(&address) {
                    Some(a) if row.in_cnt == 0 => {
                        let _ = a;
                        self.err(r, format!("{}{}{}", self.tr(t::ALWAYS_ACTIVE_1), r + 1, self.tr(t::ALWAYS_ACTIVE_2)));
                    }
                    None => {}
                    Some(a) if a >= 1 && a <= max_addr => addr = Addr::Num(a),
                    Some(_) => {
                        let a = address.replace('\n', " ");
                        let m = format!(
                            "{}{a}{}{}{}",
                            self.tr(t::THE_ADDRESS),
                            self.tr(t::IN_ROW),
                            r + 1,
                            self.tr(t::IS_INVALID)
                        );
                        self.err(r, m)
                    }
                }
            }
        } else {
            // A variable name as address (ProcessLine, M06_Write_Header.bas)
            let var_name = expand_name(&address);
            if var_name.starts_with("RM ") {
                self.refused_rows = true;
                self.err(r, "Return messages (RM) as address are not supported by MobaLedLib Studio yet");
                return;
            }
            if !var_name.is_empty() {
                if !self.valid_var_name(&var_name, r) {
                    return;
                }
                addr = Addr::Name(var_name);
            }
        }
        self.header_entry(r, addr);
    }

    /// `Print_Keyboard_Defines_for_Type` (M06_Write_Header_SW.bas)
    fn print_keyboard_defines_for_type(
        out: &mut String,
        name: &str,
        inp_cnt: i64,
        nr: &mut i64,
        skip_11_16: bool,
        suppress_fill: bool,
    ) {
        for i in 1..=inp_cnt {
            out.push_str(&format!("{}{}{NL}", pad(&format!("#define {name}{i}"), 32), nr));
            // VBA: "(i - 1 Mod 16) = 9" is "i - (1 Mod 16) = 9": only after the 10th switch 6 channels are skipped
            if skip_11_16 && i - 1 == 9 {
                *nr += 6;
            }
            *nr += 1;
        }
        // Always multiples of 8 input channels are used
        if !suppress_fill {
            let res_cnt = (8 - inp_cnt % 8) % 8;
            if res_cnt > 0 {
                *nr += res_cnt;
                out.push_str(&format!(
                    "// Reserve channels: {res_cnt} because MobaLedLib_Copy_to_InpStruct always writes multiple of 8 channels{NL}"
                ));
            }
        }
    }

    /// `Write_Switches_Header_File_Part_A` (M06_Write_Header_SW.bas). `false` (with Excel's message)
    /// stops the generation.
    fn write_switches_part_a(&mut self, out: &mut String) -> bool {
        let p = |out: &mut String, s: &str| {
            out.push_str(s);
            out.push_str(NL)
        };
        let board = self.layout.board.platform.id();
        let [sw_a, _, _, sw_d] = self.sw.inp_cnt;
        let [ch1_cnt, ch2_cnt] = self.sw.channel_inp_cnt;
        let read_ldr = self.sw.read_ldr;
        let mut min_channel: i64 = 99999999;
        let mut used_abutton_channels = 0;
        let mut start_abuttons = 0;
        if sw_a > 0 || read_ldr {
            if sw_a > 0 && !self.platform_bool("SupportsAnalogButtons") {
                self.errors.push(Diagnostic { row: None, message: self.tr(t::ANALOG_NOT_SUPPORTED) });
                return false;
            }
            if read_ldr && !self.platform_bool("SupportsLDR") {
                self.errors.push(Diagnostic { row: None, message: self.tr(t::LDR_NOT_SUPPORTED) });
                return false;
            }
            let ana_but_pins: Vec<String> = vba_split(&self.sw.inp_lst[0]).iter().map(|s| s.to_string()).collect();
            if sw_a > ana_but_pins.len() as i64 * 10 {
                self.errors.push(Diagnostic { row: None, message: self.tr(t::TOO_MANY_ANALOG) });
                return false;
            }
            p(out, "#ifndef CONFIG_ONLY");
            p(out, "//*** Analog switches ***");
            p(out, "");
            match board {
                // Excel also makes sure that the AnalogScanner library is installed here
                "AM328" => {
                    p(out, "#include <AnalogScanner.h>   // Interrupt driven analog reading library. The library has to be installed manually from https://github.com/merose/AnalogScanner");
                    p(out, "AnalogScanner scanner;       // Creates an instance of the analog pin scanner.");
                }
                "ESP32" => {
                    p(out, "#include \"AnalogScannerESP32.h\"   ");
                    p(out, "AnalogScannerESP32 scanner;       // Creates an instance of the analog pin scanner.");
                }
                "PICO" => {
                    p(out, "#include \"AnalogScannerPico.h\"   ");
                    p(out, "AnalogScannerPico scanner;        // Creates an instance of the analog pin scanner.");
                }
                _ => {}
            }
            p(out, "");
            if sw_a > 0 {
                p(out, "#include <Analog_Buttons10.h>");
                used_abutton_channels = (sw_a + 9) / 10;
                for ach in 1..=used_abutton_channels {
                    p(out, &format!("Analog_Buttons10_C AButtons{ach}({});", ana_but_pins[ach as usize - 1]));
                }
                p(out, "");
            }
            p(out, "");
            if !self.sw.damping_fact.is_empty() {
                p(out, &self.sw.damping_fact);
                p(out, "");
            }
            if read_ldr {
                p(out, "#include \"Read_LDR.h\"     // Darkness sensor");
                p(out, "");
            }
            p(out, "#endif //CONFIG_ONLY");

            make_channel_divisible_by_4(&mut self.channel);
            start_abuttons = self.channel;
            let mut tmp_channel = self.channel;
            min_channel = min_channel.min(self.channel);
            Self::print_keyboard_defines_for_type(out, "SwitchA", sw_a, &mut tmp_channel, true, false);
            p(
                out,
                &format!(
                    "#define START_SWITCHES_A  {}// Define the start number for the first analog switch",
                    pad(&self.channel.to_string(), 41)
                ),
            );
            // 16 because Copy_Bits_to_InpStructArray() always fills bytes
            self.channel += used_abutton_channels * 16;
            p(out, "");
        }

        let (mut start_switches1, mut start_switches2) = (0, 0);
        if ch1_cnt > 0 {
            p(out, "//*** Digital switches ***");
            p(out, "");
            make_channel_divisible_by_4(&mut self.channel);
            start_switches1 = self.channel;
            min_channel = min_channel.min(self.channel);
            let name1 = self.sw.ctr_cha_name[0].clone();
            Self::print_keyboard_defines_for_type(out, &name1, ch1_cnt, &mut self.channel, false, false);

            make_channel_divisible_by_4(&mut self.channel);
            start_switches2 = self.channel;
            min_channel = min_channel.min(self.channel);
            if ch2_cnt > 0 {
                let name2 = self.sw.ctr_cha_name[1].clone();
                Self::print_keyboard_defines_for_type(out, &name2, ch2_cnt, &mut self.channel, false, false);
            }
            p(out, "");
        }

        let d_pins = vba_split(&self.sw.inp_lst[3]).len() as i64;
        if sw_d > d_pins {
            self.errors.push(Diagnostic { row: None, message: self.trf(t::TOO_MANY_SWITCH_D, &[&d_pins.to_string()]) });
            return false;
        }
        if sw_d > 0 {
            p(out, "//*** Direct connected switches ***");
            p(out, "");
            min_channel = min_channel.min(self.channel);
            p(
                out,
                &format!(
                    "#define START_SWITCHES_D  {}// Define the start number for the first mainboard switch",
                    pad(&self.channel.to_string(), 41)
                ),
            );
            Self::print_keyboard_defines_for_type(out, "SwitchD", sw_d, &mut self.channel, false, true);
            p(out, "");
        }

        let dst_vars: Vec<String> = self.dst_var_list.split_whitespace().map(|s| s.to_string()).collect();
        if self.dst_var_list != " " {
            // #defines for the output variables (Print_DstVar_List)
            min_channel = min_channel.min(self.channel);
            p(out, "//*** Output Channels ***");
            p(
                out,
                &format!(
                    "#define START_VARIABLES   {}// Define the start number for the variables.",
                    pad(&self.channel.to_string(), 41)
                ),
            );
            for var in &dst_vars {
                p(out, &format!("#define {}  {}", pad(var, 22), self.channel));
                self.channel += 1;
            }
            p(out, "");
        }

        if sw_d > 0 {
            p(
                out,
                &format!(
                    "const PROGMEM uint8_t SwitchD_Pins[] = {}// Array of pins which read switches 'D'",
                    pad(&format!("{{ {} }};", self.sw.inp_lst[3].replace(' ', ",")), 28)
                ),
            );
            p(out, "#define SWITCH_D_INP_CNT sizeof(SwitchD_Pins)");
            p(out, "");
        }

        let last_char = |s: &str| s.chars().last().map(|c| c.to_string()).unwrap_or_default();
        if ch1_cnt > 0 {
            let sw = &self.sw;
            p(out, "");
            p(out, &format!("#define CTR_CHANNELS_1    {}// Number of used counter channels for keyboard 1. Up to 10 if one CD4017 is used, up to 18 if two CD4017 are used, ...", pad(&sw.ctr_channels[0].to_string(), 41)));
            p(out, &format!("#define CTR_CHANNELS_2    {}// Number of used counter channels for keyboard 2. Up to 10 if one CD4017 is used, up to 18 if two CD4017 are used, ...", pad(&sw.ctr_channels[1].to_string(), 41)));
            p(
                out,
                &format!(
                    "#define BUTTON_INP_LIST_1 {}// Comma separated list of the button input pins",
                    pad(&sw.but_inp_list[0].replace(' ', ","), 41)
                ),
            );
            p(
                out,
                &format!(
                    "#define BUTTON_INP_LIST_2 {}// Comma separated list of the button input pins",
                    pad(&sw.but_inp_list[1].replace(' ', ","), 41)
                ),
            );
            p(
                out,
                &format!("#define CLK_PIN           {}// Pin number used for the CD4017 clock", pad(&sw.clk_pin, 41)),
            );
            p(
                out,
                &format!("#define RESET_PIN         {}// Pin number used for the CD4017 reset", pad(&sw.rst_pin, 41)),
            );
            p(out, "");
            p(out, "#ifndef CONFIG_ONLY");
            p(out, "#include <Keys_4017.h>                                             // Keyboard library which uses the CD4017 counter to save Arduino pins. Attention: The pins (CLK_PIN, ...) must be defined prior.");
            p(out, "#endif");
            p(out, "");
            p(
                out,
                &format!(
                    "#define START_SWITCHES_1  {}// Define the start number for the first keyboard.",
                    pad(&start_switches1.to_string(), 41)
                ),
            );
            p(
                out,
                &format!(
                    "#define START_SWITCHES_2  {}// Define the start number for the second keyboard.",
                    pad(&start_switches2.to_string(), 41)
                ),
            );
            p(
                out,
                &format!(
                    "#define START_SWITCHES_{}  {}// Define the start number for the first keyboard.",
                    last_char(&sw.ctr_cha_name[0]),
                    pad(&start_switches1.to_string(), 41)
                ),
            );
        }
        if ch2_cnt > 0 {
            p(
                out,
                &format!(
                    "#define START_SWITCHES_{}  {}// Define the start number for the second keyboard.",
                    last_char(&self.sw.ctr_cha_name[1]),
                    pad(&start_switches2.to_string(), 41)
                ),
            );
        }
        p(out, "");

        let total_a = to_eight(used_abutton_channels * 16);
        let total_1 = to_eight(self.sw.ctr_channels[0] * list_length(&self.sw.but_inp_list[0]));
        let total_2 = to_eight(self.sw.ctr_channels[1] * list_length(&self.sw.but_inp_list[1]));
        let total_vars = list_length(&self.dst_var_list);
        if min_channel != 99999999 {
            p(
                out,
                &format!(
                    "#define START_SEND_INPUTS {}// Start address of all switches/variables",
                    pad(&min_channel.to_string(), 41)
                ),
            );
            p(
                out,
                &format!(
                    "#define TOTAL_SEND_INPUTS {}// Number of used switches/variables",
                    pad(&(total_a + total_1 + total_2 + sw_d + total_vars).to_string(), 41)
                ),
            );
        }
        p(
            out,
            &format!(
                "#define TOTAL_SWITCHES_A  {}// Number of used inputs for analog keyboard",
                pad(&total_a.to_string(), 41)
            ),
        );
        let (total_b, total_c) =
            if last_char(&self.sw.ctr_cha_name[0]) == "B" { (total_1, total_2) } else { (0, total_1) };
        p(
            out,
            &format!(
                "#define TOTAL_SWITCHES_B  {}// Number of used inputs for keyboard 1",
                pad(&total_b.to_string(), 41)
            ),
        );
        p(
            out,
            &format!(
                "#define TOTAL_SWITCHES_C  {}// Number of used inputs for keyboard 2",
                pad(&total_c.to_string(), 41)
            ),
        );
        p(
            out,
            &format!(
                "#define TOTAL_SWITCHES_D  {}// Number of used inputs for main board switches",
                pad(&sw_d.to_string(), 41)
            ),
        );
        p(out, &format!("#define TOTAL_VARIABLES   {}// Number of used variables", pad(&total_vars.to_string(), 41)));

        let platform = self.layout.board.platform.id();
        let pins = vba_split_space(self.catalog.platform(platform, "LED_Pins").unwrap_or(" 6 "));
        p(out, "/*********************/");
        p(out, "#define SETUP_FASTLED()                                                      \\");
        p(out, "/*********************/                                                      \\");
        let mut cnt = 0;
        let mut exp_out_pins = 0;
        for ch in 0..LED_CHANNELS {
            let n = self.leds_per_channel[ch];
            if n > 0 {
                exp_out_pins = ch;
                if ch < pins.len() && pins[ch] != VIRTUAL_CHANNEL {
                    p(
                        out,
                        &format!(
                            "  CLEDController& controller{ch} = FastLED.addLeds<NEOPIXEL, {}>(leds+{},{}); \\",
                            pad_left(&pins[ch], 2),
                            pad_left(&cnt.to_string(), 3),
                            pad_left(&n.to_string(), 3)
                        ),
                    );
                }
            }
            cnt += n;
        }
        if exp_out_pins >= pins.len() {
            self.errors
                .push(Diagnostic { row: None, message: self.trf(t::LED_PINS, &[&(exp_out_pins + 1).to_string()]) });
        }
        p(out, "                                                                             \\");
        for ch in 0..LED_CHANNELS {
            if self.leds_per_channel[ch] > 0 && ch < pins.len() && pins[ch] != VIRTUAL_CHANNEL {
                p(out, &format!("  controller{ch}.clearLeds(256);                                                \\"));
            }
        }
        p(out, "  FastLED.setDither(DISABLE_DITHER);       // avoid sending slightly modified brightness values");
        p(out, "/*End*/");
        p(out, "");

        // Additional setup proc
        if sw_a > 0 || read_ldr || ch1_cnt > 0 || sw_d > 0 {
            p(out, "#define USE_ADDITIONAL_SETUP_PROC                                  // Activate the usage of the Additional_Setup_Proc()");
            p(out, "");
            p(out, "//--------------------------");
            p(out, "void Additional_Setup_Proc()");
            p(out, "//--------------------------");
            p(out, "{");
            if sw_a > 0 || read_ldr {
                let ldr = self.sw.ldr_pin.clone();
                p(out, "#ifndef CONFIG_ONLY");
                let mut pin_list = String::new();
                if sw_a > 0 {
                    pin_list = format!("{},", self.sw.inp_lst[0].trim_matches(' ').replace(' ', ","));
                }
                if read_ldr {
                    pin_list += &format!("{ldr},");
                }
                pin_list.pop();
                p(out, &format!("  int scanOrder[] = {{{pin_list}}};"));
                p(out, "  const int SCAN_COUNT = sizeof(scanOrder) / sizeof(scanOrder[0]);");
                if board == "AM328" {
                    p(out, "");
                    if read_ldr {
                        p(out, &format!("  Init_DarknessSensor({ldr}, 50, SCAN_COUNT); // Attention: The analogRead() function can't be used together with the darkness sensor !"));
                        p(out, &format!("  scanner.setCallback({ldr}, Darkness_Detection_Callback);"));
                    }
                    p(out, "  scanner.setScanOrder(SCAN_COUNT, scanOrder);");
                    p(out, "  scanner.beginScanning();");
                } else if board == "ESP32" || board == "PICO" {
                    p(out, "  scanner.setScanPins(SCAN_COUNT, scanOrder);");
                    if read_ldr {
                        p(out, &format!("  Init_DarknessSensor({ldr}, 50, 50); // Attention: The analogRead() function can't be used together with the darkness sensor !"));
                        p(out, &format!("  scanner.setCallback({ldr}, Darkness_Detection_Callback);"));
                    }
                }
                p(out, "#endif");
                p(out, "");
            }
            if ch1_cnt > 0 {
                p(out, "#ifndef CONFIG_ONLY");
                p(out, "  Keys_4017_Setup(); // Initialize the keyboard scanning process");
                p(out, "#endif");
            }
            if sw_d > 0 {
                p(out, "");
                p(out, "  for (uint8_t i = 0; i < SWITCH_D_INP_CNT; i++)");
                p(out, "    pinMode(pgm_read_byte_near(&SwitchD_Pins[i]), INPUT_PULLUP);");
            }
            p(out, "}");
            p(out, "");
        }

        // Additional_Loop_Proc()
        if sw_a > 0 || ch1_cnt > 0 || sw_d > 0 {
            p(out, "/****************************/");
            p(out, "#define Additional_Loop_Proc() \\");
            p(out, "/****************************/ \\");
            p(out, "{                              \\");
            if sw_a > 0 {
                let mut act_start_abuttons = start_abuttons;
                p(out, "  uint16_t Button;             \\");
                for ach in 1..=used_abutton_channels {
                    let line = format!(
                        "  Button = AButtons{ach}.Get(); MobaLedLib_Copy_to_InpStruct((uint8_t*)&Button, 2, {act_start_abuttons});"
                    );
                    p(out, &format!("{}\\", pad(&line, 89)));
                    act_start_abuttons += 16;
                }
            }
            if !self.store_values_txt.is_empty() {
                // Add_Set_Input_Loop_for_Keys (Store_ValuesTxt_Used)
                for nr in [1, 2] {
                    if self.sw.channel_inp_cnt[nr - 1] > 0 {
                        for line in [
                            format!("  uint8_t channel{nr} = START_SWITCHES_{nr};"),
                            format!("  for (int j=0;j<KEYS_ARRAY_BYTE_SIZE_{nr};j++)"),
                            "    {".into(),
                            format!("    uint8_t tmp = Keys_Array_{nr}[j];"),
                            "    for (int i=0;i<8;i++)".into(),
                            "      {".into(),
                            format!("      MobaLedLib.Set_Input(channel{nr}++, tmp & 0x01);"),
                            "      tmp = tmp >> 1;".into(),
                            "      }".into(),
                            "    }".into(),
                        ] {
                            p(out, &format!("{} \\", pad(&line, 70)));
                        }
                    }
                }
            } else {
                if ch1_cnt > 0 {
                    p(
                        out,
                        "  MobaLedLib_Copy_to_InpStruct(Keys_Array_1, KEYS_ARRAY_BYTE_SIZE_1, START_SWITCHES_1);  \\",
                    );
                }
                if ch2_cnt > 0 {
                    p(
                        out,
                        "  MobaLedLib_Copy_to_InpStruct(Keys_Array_2, KEYS_ARRAY_BYTE_SIZE_2, START_SWITCHES_2);  \\",
                    );
                }
            }
            if sw_d > 0 {
                p(out, &format!("  for (uint8_t i = 0; i < {sw_d}; i++) \\"));
                p(
                    out,
                    "      MobaLedLib.Set_Input(SwitchD1 + i, !digitalRead(pgm_read_byte_near(&SwitchD_Pins[i])));\\",
                );
            }
            p(out, "}");
        }
        true
    }

    fn write(&mut self, opts: &GenOptions) -> String {
        let num_leds: i64 = self.leds_per_channel.iter().sum();
        let platform = self.layout.board.platform;
        let max_led = self.catalog.platform(platform.id(), "MaxLed").map(val_i).unwrap_or(256);
        if num_leds > max_led {
            self.errors.push(Diagnostic {
                row: None,
                message: format!("{}{num_leds}\n{}", self.tr(t::MAX_LEDS_1), self.tr(t::MAX_LEDS_2))
                    .replace("#1#", &max_led.to_string()),
            });
        }

        // DelTailingEmptyLines + local InCh variables
        for t in [&mut self.ext_addr_txt, &mut self.store_values_txt, &mut self.in_ch_txt] {
            while t.ends_with("\r\n\r\n") {
                t.truncate(t.len() - 3);
            }
        }
        if self.loc_in_ch_nr > 0 {
            self.in_ch_txt += &format!("{NL}// Local InCh variables{NL}");
            for i in 0..self.loc_in_ch_nr {
                self.in_ch_txt += &format!("{}{}{NL}", pad(&format!("#define LOC_INCH{i}"), 32), self.channel);
                self.channel += 1;
            }
        }
        // Create_Loc_InCh_Defines: Excel shows this error, but still writes the header
        if self.max_used_loc_in_ch >= self.loc_in_ch_nr {
            self.warnings.push(Diagnostic {
                row: Some(self.max_used_loc_in_ch_row),
                message: self.trf(
                    t::VAR_NOT_DEFINED,
                    &[&format!("LOC_INCH{}", self.max_used_loc_in_ch), &(self.max_used_loc_in_ch_row + 1).to_string()],
                ),
            });
        }

        let s = &self.layout.settings;
        let page_id = self.page_id;
        let leds_list: Vec<String> = (0..=self.max_channels).map(|n| self.leds_per_channel[n].to_string()).collect();
        let now = opts.now;
        let mut o = String::new();
        macro_rules! p {
            ($($arg:tt)*) => {{ o.push_str(&format!($($arg)*)); o.push_str(NL); }};
        }

        p!("// This file contains the {page_id} and LED definitions.");
        p!("//");
        p!("// It was automatically generated by the program {}", opts.program);
        p!("// File creation: {} {}", now.format("%d.%m.%Y"), now.format("%H:%M:%S"));
        p!("// (Attention: The display in the Arduino IDE is not updated if Options/External Editor is disabled)");
        p!("");
        p!("#ifndef __LEDS_AUTOPROG_H__");
        p!("#define __LEDS_AUTOPROG_H__");
        p!("");
        p!("#ifndef CONFIG_ONLY");
        p!("#ifndef ARDUINO_RASPBERRY_PI_PICO");
        p!("#ifndef FASTLED_ESP32_I2S");
        p!("#define FASTLED_INTERNAL       // Disable version number message in FastLED library (looks like an error)");
        p!("#endif");
        p!("#include <FastLED.h>           // The FastLED library must be installed in addition if you got the error message \"..fatal error: FastLED.h: No such file or directory\"");
        p!("                               // Arduino IDE: Sketch / Include library / Manage libraries                    Deutsche IDE: Sketch / Bibliothek einbinden / Bibliothek verwalten");
        p!("                               //              Type \"FastLED\" in the \"Filter your search...\" field                          \"FastLED\" in das \"Grenzen Sie ihre Suche ein\" Feld eingeben");
        p!("                               //              Select the entry and click \"Install\"                                         Gefundenen Eintrag auswaehlen und \"Install\" anklicken");
        p!("#else");
        p!("#include <PicoFastLED.h>       // Juergens minimum version or FastLED for Raspberry Pico");
        p!("#endif");
        p!("#endif // CONFIG_ONLY");
        p!("");
        p!("#include <MobaLedLib.h>");
        p!("");
        p!(
            "#define START_MSG \"LEDs_AutoProg Ver 3: {} {} {} {:02}:{:02}\"",
            platform.id(),
            opts.lib_version,
            now.format("%d.%m.%y"),
            now.hour(),
            now.minute()
        );
        p!("");
        if self.sx {
            p!("#define TWO_BUTTONS_PER_ADDRESS 0      // One button is used (Selectrix)");
            p!("#define USE_SX_INTERFACE               // enable Selectrix protocol on single CPU mainboards");
        } else {
            p!("#define TWO_BUTTONS_PER_ADDRESS 1      // Two buttons (Red/Green) are used (DCC/LNet/CAN)");
        }
        p!("#ifdef NUM_LEDS");
        p!("  #warning \"'NUM_LEDS' definition in the main program is replaced by the included 'LEDs_AutoProg.h' with {num_leds}\"");
        p!("  #undef NUM_LEDS");
        p!("#endif");
        p!("");
        p!("#define NUM_LEDS {}// Number of LEDs (Maximal 256 RGB LEDs could be used)", pad(&num_leds.to_string(), 22));
        p!("");
        p!("#define LEDS_PER_CHANNEL \",{}\"", leds_list.join(","));
        p!("");
        p!("#define USE_PROTOCOL_{}", page_id.to_uppercase());
        if !self.alias_names.is_empty() {
            p!("{}", self.alias_names);
        }
        if let Some(t) = s.house_min_t {
            p!("#undef  HOUSE_MIN_T");
            p!("#define HOUSE_MIN_T  {t}");
        }
        if let Some(t) = s.house_max_t {
            p!("#undef  HOUSE_MAX_T");
            p!("#define HOUSE_MAX_T {t}");
        }
        let (min_t, max_t) = (s.house_min_t.unwrap_or(50), s.house_max_t.unwrap_or(150));
        if min_t > max_t || max_t == 0 {
            self.errors.push(Diagnostic { row: None, message: self.tr(t::HOUSE_TIMES) });
        }
        p!("");
        if s.color_test_mode {
            p!("#define RECEIVE_LED_COLOR_PER_RS232{NL}");
        }
        if s.use_spi || self.sheet.protocol == Protocol::Can {
            if s.use_spi {
                p!("#define USE_SPI_COM                    // Use the SPI bus for the communication in addition to the RS232 if J13 is closed. If no DCC commands are configured the A1 pin of the DCC Arduino is disabled");
            }
            if platform.id() == "AM328" {
                if self.pin_a3_is_used() {
                    p!("#define LED_HEARTBEAT_PIN -1           // Disable the heartbeat pin because it's used for the SwitchB or SwitchC");
                } else {
                    p!("#define LED_HEARTBEAT_PIN A3           // Don't use the internal heartbeat LED because the D13 pins between LED and DCC arduin are connected together");
                }
            }
        }
        if s.gen_button_release_com == 0 || self.sx {
            p!("#define GEN_BUTTON_RELEASE");
        }
        if !(2..=3).contains(&s.gen_button_release_com) {
            p!("#define GEN_BUTTON_RELEASE_COM GEN_OFF");
        } else {
            p!("#define GEN_BUTTON_RELEASE_COM {}", s.gen_button_release_com - 1);
        }

        if !self.ext_addr_txt.is_empty() {
            if s.use_spi && !self.check_switch_lists_for_spi_pins() {
                return o;
            }
            p!("#define USE_EXT_ADDR");
            if matches!(self.sheet.protocol, Protocol::Dcc | Protocol::Selectrix | Protocol::LNet) {
                p!("#define USE_RS232_OR_SPI_AS_INPUT 1     // Use the RS232 or SPI Input to read DCC/LNet/SX commands from the second Arduino and from the PC (The SPI is only used if enabled with USE_SPI_COM)");
            }
            if self.sheet.protocol == Protocol::Dcc {
                p!("#define ADDR_OFFSET {}", s.dcc_offset);
            } else {
                p!("#define ADDR_OFFSET 0");
            }
            if self.sheet.protocol == Protocol::Can {
                p!("#define USE_CAN_AS_INPUT");
            }
            p!("");
            p!("#define ADDR_MSK  0x3FFF  // 14 Bits are used for the Address");
            p!("");
            p!("#define S_ONOFF   (uint16_t)0");
            p!("#define B_RED     (uint16_t)(1<<14)");
            p!("#define B_GREEN   (uint16_t)(2<<14)");
            p!("#define O_RET_MSG (uint16_t)(3<<14)    // Return messages (Rueckmelder)");
            p!("#define B_TAST    B_RED");
            p!("");
            p!("");
            p!("typedef struct");
            p!("    {{");
            p!("    uint16_t AddrAndTyp; // Addr range: 0..16383. The upper two bytes are used for the type");
            p!("    uint8_t  InCnt;");
            p!("    }} __attribute__ ((packed)) Ext_Addr_T;");
            p!("");
            p!("// Definition of external adresses");
            p!("#ifdef CONFIG_ONLY");
            p!("const Ext_Addr_T Ext_Addr[] __attribute__ ((section (\".MLLAddressConfig\"))) =");
            p!("#else");
            p!("const PROGMEM Ext_Addr_T Ext_Addr[] =");
            p!("#endif");
            p!("         {{ // Addr & Typ    InCnt");
            o.push_str(&self.ext_addr_txt);
            p!("         }};");
            p!("");
            p!("");
        }

        p!("// Input channel defines for local inputs and expert users");
        p!("{}", self.in_ch_txt);
        p!("");
        if !self.write_switches_part_a(&mut o) {
            return o;
        }
        p!("{}", self.day_and_night);
        p!("");
        p!("//*******************************************************************");
        p!("// *** Configuration array which defines the behavior of the LEDs ***");
        p!("MobaLedLib_Configuration()");
        p!("  {{");
        p!("{}", self.config_txt);
        p!("  EndCfg // End of the configuration");
        p!("  }};");
        p!("//*******************************************************************");
        p!("");
        p!("#ifndef COPYLED_OFF");
        p!("#define COPYLED_OFF 0");
        p!("#endif");
        p!("");
        p!("#ifndef COPYLED_OFF_ONCE");
        p!("#define COPYLED_OFF_ONCE 1");
        p!("#endif");
        p!("");
        p!("//---------------------------------------------");
        p!("void Set_Start_Values(MobaLedLib_C &MobaLedLib)");
        p!("//---------------------------------------------");
        p!("{{");
        o.push_str(&self.start_values);
        p!("}}");
        p!("");

        if !self.store_values_txt.is_empty() {
            p!("");
            p!("// if function returns TRUE the calling loop stops");
            p!("typedef bool(*HandleValue_t) (uint8_t CallbackType, uint8_t ValueId, uint8_t* Value, uint16_t EEPromAddr, uint8_t TargetValueId, uint8_t Options);");
            p!("");
            p!("");
            p!("#define InCnt_MSK  0x003F  // 6 bits are used for the InCnt, 2 bits for the type ttcc cccc => Max InCnt=63");
            p!("#define IS_COUNTER (uint8_t)0x80");
            p!("#define IS_PULSE   (uint8_t)0x40");
            p!("#define IS_TOGGLE  (uint8_t)0x00");
            p!("#define COUNTER_ID");
            p!("");
            p!("typedef struct");
            p!("    {{");
            p!("    uint8_t TypAndInCnt; // Type bits 7 & 6, InCnt 0..5");
            p!("    uint8_t Channel;");
            p!("    }} __attribute__ ((packed)) Store_Channel_T;");
            p!("");
            p!("// Definition of channels and counters that need to store state in EEProm{NL}const PROGMEM Store_Channel_T Store_Values[] ={NL}         {{ // Mode + InCnt , Channel");
            o.push_str(&self.store_values_txt);
            p!("         }};");
            p!("");
        } else {
            p!("");
            p!("// No macros used which are stored to the EEPROM => Disable the ENABLE_STORE_STATUS flag in case it was set in the excel sheet");
            p!("#ifdef ENABLE_STORE_STATUS");
            p!("  #undef ENABLE_STORE_STATUS");
            p!("#endif");
            p!("");
        }
        p!("#ifndef CONFIG_ONLY");
        p!("#endif // CONFIG_ONLY");
        for _ in 0..5 {
            p!("");
        }
        p!("#endif // __LEDS_AUTOPROG_H__");

        if self.channel - 1 > 250 {
            self.errors.push(Diagnostic {
                row: None,
                message: format!("{}{}.\n\n{}", self.tr(t::IN_CH_TOO_MANY), self.channel - 1, self.tr(t::IN_CH_USE)),
            });
        }
        if self.config_txt.is_empty() {
            self.warnings.push(Diagnostic { row: None, message: self.tr(t::EMPTY_CONFIG) });
        }
        o
    }
}

/// Generate `LEDs_AutoProg.h` for one sheet of the layout.
pub fn generate(
    layout: &Layout,
    sheet: usize,
    catalog: &Catalog,
    opts: &GenOptions,
) -> Result<Generated, GenerateError> {
    let Some(sh) = layout.sheets.get(sheet) else {
        return Err(GenerateError { errors: vec![Diagnostic { row: None, message: "sheet not found".into() }] });
    };
    let supported = catalog.platform(layout.board.platform.id(), &format!("Supports{}", sh.protocol.page_id()));
    if supported.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "false" | "")) {
        return Err(GenerateError {
            errors: vec![Diagnostic {
                row: None,
                message: trf(catalog, t::PROTOCOL_NOT_SUPPORTED, opts.language, &[sh.protocol.page_id()]),
            }],
        });
    }
    let mut g = Gen::new(layout, sh, catalog);
    g.lang = opts.language;
    // Create_HeaderFile: Init_HeaderFile_Generation_SW (first scan), the rows, Check_Detected_Variables
    if !g.first_scan() {
        return Err(GenerateError { errors: g.errors });
    }
    for r in 0..sh.rows.len() {
        g.process_row(r);
    }
    g.check_detected_variables();
    let header = g.write(opts);
    if !g.errors.is_empty() {
        return Err(GenerateError { errors: g.errors });
    }
    Ok(Generated { header, num_leds: g.leds_per_channel.iter().sum(), input_channels: g.channel, warnings: g.warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Platform, Row};

    fn opts() -> GenOptions {
        GenOptions {
            program: "Test".into(),
            lib_version: "3.5.0".into(),
            now: NaiveDateTime::parse_from_str("2026-10-26 21:05:09", "%Y-%m-%d %H:%M:%S").unwrap(),
            language: crate::lang::ENGLISH,
        }
    }

    fn catalog() -> Catalog {
        static LANGUAGES: std::sync::OnceLock<crate::lang::LanguageSheet> = std::sync::OnceLock::new();
        let mut c =
            Catalog { languages: LANGUAGES.get_or_init(|| Catalog::bundled().languages).clone(), ..Catalog::default() };
        let mut am = std::collections::BTreeMap::new();
        am.insert("LED_Pins".to_string(), "6 A4 A5 A6 A7".to_string());
        am.insert("MaxLed".to_string(), "256".to_string());
        c.platforms.insert("AM328".into(), am);
        c
    }

    fn layout(rows: Vec<Row>) -> Layout {
        let mut l = Layout::new("t", Protocol::Dcc, Platform::Am328);
        l.sheets[0].rows.extend(rows);
        l
    }

    #[test]
    fn heartbeat_only() {
        let l = layout(vec![]);
        let g = generate(&l, 0, &catalog(), &opts()).unwrap();
        assert_eq!(g.num_leds, 20);
        assert!(g.header.contains("#define NUM_LEDS 20                    // Number of LEDs"));
        assert!(g.header.contains("  RGB_Heartbeat(0)"));
        assert!(g.header.contains("FastLED.addLeds<NEOPIXEL,  6>(leds+  0, 20); \\"));
        assert!(!g.header.contains("USE_EXT_ADDR"));
        assert!(g.header.contains("#define START_MSG \"LEDs_AutoProg Ver 3: AM328 3.5.0 26.10.26 21:05\""));
        assert!(g.header.contains("\r\n"));
    }

    #[test]
    fn dcc_address_rows() {
        let l = layout(vec![
            Row {
                enabled: true,
                address: "1".into(),
                input_type: Some(InputType::OnOff),
                description: "Haus".into(),
                macro_text: "House(#LED, #InCh, 2, 5, ROOM_DARK, ROOM_BRIGHT)".into(),
                leds: "2".into(),
                in_cnt: 1,
                ..Row::default()
            },
            Row {
                enabled: true,
                address: "10".into(),
                input_type: Some(InputType::Red),
                description: "Signal".into(),
                macro_text: "EntrySignal3_RGB(#LED, #InCh)".into(),
                leds: "2".into(),
                in_cnt: 3,
                ..Row::default()
            },
        ]);
        let g = generate(&l, 0, &catalog(), &opts()).unwrap();
        let h = &g.header;
        assert!(h.contains("  House(1, 0, 2, 5, ROOM_DARK, ROOM_BRIGHT)"));
        assert!(h.contains("  EntrySignal3_RGB(3, 1)"));
        assert!(h.contains("           { 1    + S_ONOFF,   1 },      // Haus\r\n"));
        assert!(h.contains("           { 10   + B_RED,     3 },      // Signal\r\n"));
        assert!(h.contains("#define INCH_DCC_1_ONOFF        0      // Haus\r\n"));
        // Red/Green alternate, the address increments after green
        assert!(h.contains("#define INCH_DCC_10_RED         1      // Signal\r\n"));
        assert!(h.contains("#define INCH_DCC_10_GREEN       2      //     \"\r\n"));
        assert!(h.contains("#define INCH_DCC_11_RED         3      //     \"\r\n"));
        // An on/off address stores its state in the EEPROM
        assert!(h.contains("           { IS_TOGGLE + 1 , INCH_DCC_1_ONOFF    },      // Haus"));
        assert!(h.contains("#define ADDR_OFFSET 0"));
        assert_eq!(g.input_channels, 4);
    }

    #[test]
    fn errors_point_at_rows() {
        let l = layout(vec![Row {
            enabled: true,
            address: "5".into(),
            macro_text: "Const(#LED, C1, #InCh, 0, 255)".into(),
            leds: "C1-1".into(),
            in_cnt: 1,
            ..Row::default()
        }]);
        let e = generate(&l, 0, &catalog(), &opts()).unwrap_err();
        assert_eq!(e.errors[0].row, Some(1));
        assert!(e.errors[0].message.contains("input type"));
    }

    #[test]
    fn unsupported_features_are_reported() {
        let l = layout(vec![Row {
            enabled: true,
            address: "5".into(),
            macro_text: "include(Other)".into(),
            leds: "C1-1".into(),
            in_cnt: 1,
            ..Row::default()
        }]);
        let e = generate(&l, 0, &catalog(), &opts()).unwrap_err();
        assert!(e.errors[0].message.contains("not supported"));
    }

    // ---- Variables as address (M06_Write_Header_SW.bas: Valid_Var_Name, Add_InpVars, Check_Detected_Variables)

    fn row(address: &str, macro_text: &str) -> Row {
        Row {
            enabled: true,
            address: address.into(),
            input_type: Some(InputType::OnOff),
            description: format!("{address} {macro_text}"),
            macro_text: macro_text.into(),
            leds: "1".into(),
            in_cnt: 1,
            ..Row::default()
        }
    }

    fn gen(rows: Vec<Row>) -> Result<Generated, GenerateError> {
        generate(&layout(rows), 0, &Catalog::bundled(), &opts())
    }

    #[test]
    fn is_in_nr_string_like_vba() {
        // "?" skips one character, which is not checked (Is_Switch_Var only looks at A..D afterwards)
        assert_eq!(is_in_nr_string("SwitchA12", "Switch?", 1, 250), (1, 12));
        assert_eq!(is_in_nr_string("SwitchE5", "Switch?", 1, 250), (1, 5));
        assert_eq!(is_in_nr_string("SwitchB251", "Switch?", 1, 250), (-1, 251));
        // Leading 0 and the characters "-+.,eE" are not a match at all
        assert_eq!(is_in_nr_string("SwitchA0", "Switch?", 1, 250), (0, 0));
        assert_eq!(is_in_nr_string("SwitchA05", "Switch?", 1, 250), (0, 0));
        assert_eq!(is_in_nr_string("SwitchA1.5", "Switch?", 1, 250), (0, 0));
        assert_eq!(is_in_nr_string("LOC_INCH7", "LOC_INCH", 0, 250), (1, 7));
        assert_eq!(is_in_nr_string("LOC_INCH0", "LOC_INCH", 0, 250), (0, 0));
        assert_eq!(is_in_nr_string("Switch", "Switch?", 1, 250), (0, 0));
        assert_eq!(is_in_nr_string("Door1", "LOC_INCH", 0, 250), (0, 0));
    }

    #[test]
    fn split_helpers_like_vba() {
        // Test_SplitEx: the parts keep their spaces
        assert_eq!(split_logic("A AND #InCh OR NOT B"), vec!["A ", " #InCh ", " ", " B"]);
        assert_eq!(split_logic("A ANDAND B"), vec!["A ", " B"]);
        assert_eq!(split_logic("DOOR"), vec!["DO", ""]);
        assert!(split_logic("").is_empty());
        // Test_SplitMultiDelims
        assert_eq!(
            split_multi_delims("  Text  with+several|delimmiters +|", " +|"),
            vec!["Text", "with", "several", "delimmiters"]
        );
        assert_eq!(
            get_arguments("Blink(#LED, #InCh , 1 Sec)"),
            Some(vec!["#LED".into(), "#InCh".into(), "1 Sec".into()])
        );
        assert_eq!(get_arguments("Blink"), None);
        assert_eq!(get_nr_from_var("Sig5"), (5, 3));
        assert_eq!(get_nr_from_var("Sig"), (-2, 3));
        assert_eq!(get_nr_from_var("12"), (-1, 0));
        assert_eq!(get_bin_inputs(0), -1);
        assert_eq!(get_bin_inputs(7), 3);
        assert_eq!(get_bin_inputs(8), 4);
        assert_eq!(get_bin_inputs(64), -1);
        assert_eq!(expand_name("$Door"), "I0_Door");
        assert_eq!(evaluate_constant("0+1"), Some(1.0));
        assert_eq!(evaluate_constant("(0+2)*3"), Some(6.0));
        assert_eq!(evaluate_constant("0+Foo"), None);
    }

    #[test]
    fn standard_names_as_address() {
        let g = gen(vec![row("SI_1", "Blink(#LED, #InCh, 1 Sec)")]).unwrap();
        assert!(g.header.contains("  Blink(1, SI_1, 1 Sec)"), "{}", g.header);
        // A variable address gets no Ext_Addr entry and no input channel
        assert!(!g.header.contains("USE_EXT_ADDR"));
        assert_eq!(g.input_channels, 0);
    }

    #[test]
    fn a_dcc_define_as_address() {
        // Valid_Var_Name: "#define <Name> " already in InChTxt (a row above)
        let g = gen(vec![row("1", "Blink(#LED, #InCh, 1 Sec)"), row("INCH_DCC_1_ONOFF", "Blink(#LED, #InCh, 2 Sec)")])
            .unwrap();
        assert!(g.header.contains("  Blink(2, INCH_DCC_1_ONOFF, 2 Sec)"), "{}", g.header);
    }

    #[test]
    fn a_dcc_define_of_a_later_row() {
        // Not in InChTxt yet: noted as undefined, found by Check_Detected_Variables (" <Name> " in InChTxt)
        let g = gen(vec![row("INCH_DCC_1_ONOFF", "Blink(#LED, #InCh, 2 Sec)"), row("1", "Blink(#LED, #InCh, 1 Sec)")])
            .unwrap();
        assert!(g.header.contains("  Blink(1, INCH_DCC_1_ONOFF, 2 Sec)"), "{}", g.header);
    }

    #[test]
    fn an_alias_from_define_input() {
        let g = gen(vec![row("5", "// Define Input(Door)"), row("Door", "Blink(#LED, #InCh, 1 Sec)")]).unwrap();
        assert!(g.header.contains("#define Door                 INCH_DCC_5_ONOFF"), "{}", g.header);
        assert!(g.header.contains("  Blink(2, Door, 1 Sec)"), "{}", g.header);
    }

    #[test]
    fn an_alias_is_found_ignoring_case() {
        // FastFind uses Excel's Match, which ignores case
        assert!(gen(vec![row("5", "// define input(Door)"), row("Door", "Blink(#LED, #InCh, 1 Sec)")]).is_ok());
    }

    #[test]
    fn an_undefined_variable_is_an_error() {
        let e = gen(vec![row("Foo", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert_eq!(e.errors.len(), 1);
        assert_eq!(e.errors[0].row, Some(1));
        assert!(e.errors[0]
            .message
            .contains("Error: The variable 'Foo' is used as an input, but is not set anywhere."));
    }

    #[test]
    fn messages_in_the_chosen_language() {
        let l = layout(vec![row("Foo", "Blink(#LED, #InCh, 1 Sec)")]);
        let msg = |language| generate(&l, 0, &Catalog::bundled(), &GenOptions { language, ..opts() }).unwrap_err();
        assert_eq!(
            msg(0).errors[0].message,
            "Fehler: Die Variable 'Foo' wird als Eingang benutzt, wird aber nirgendwo gesetzt."
        );
        assert_eq!(
            msg(2).errors[0].message,
            "Fout: De variabele 'Foo' wordt gebruikt als invoer, maar wordt nergens ingesteld."
        );
        // vbCr in the German text: a line end
        let l = layout(vec![row("SwitchE251", "Blink(#LED, #InCh, 1 Sec)")]);
        let e = generate(&l, 0, &Catalog::bundled(), &GenOptions { language: 0, ..opts() }).unwrap_err();
        assert_eq!(
            e.errors[0].message,
            "Fehler: Die Nummer der Variable 'SwitchE251' ist ungültig!\n\nGültiger Bereich: 1..250"
        );
    }

    #[test]
    fn only_the_first_undefined_variable_is_reported() {
        let e =
            gen(vec![row("Foo", "Blink(#LED, #InCh, 1 Sec)"), row("Bar", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert_eq!(e.errors.len(), 1);
        assert!(e.errors[0].message.contains("'Foo'"));
    }

    #[test]
    fn a_dollar_name_is_expanded() {
        let e = gen(vec![row("$Door", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert!(e.errors[0].message.contains("'I0_Door'"), "{:?}", e.errors);
    }

    #[test]
    fn an_input_variable_in_the_effect_line() {
        // Add_InpVars: arguments named InCh, R_InCh, InReset, InCh2 are checked
        let e = gen(vec![row("", "Blinker(#LED, C1, Foo, 1 Sec)")]).unwrap_err();
        assert!(e.errors[0].message.contains("'Foo' is used as an input"), "{:?}", e.errors);
        assert!(gen(vec![row("", "Blinker(#LED, C1, SI_0, 1 Sec)")]).is_ok());
    }

    #[test]
    fn a_sequence_of_variables() {
        // EntrySignal3_RGB has InCnt 3 but only one InCh argument: Sig5, Sig6 and Sig7 are read
        let mut r = row("Sig5", "EntrySignal3_RGB(#LED, #InCh)");
        r.in_cnt = 3;
        r.leds = "2".into();
        let e = gen(vec![r]).unwrap_err();
        assert!(e.errors[0].message.contains("'Sig5'"), "{:?}", e.errors);
    }

    #[test]
    fn a_sequence_needs_an_address() {
        let mut r = row("", "InCh_to_TmpVar(#InCh, 3)");
        r.in_cnt = 3;
        let e = gen(vec![r]).unwrap_err();
        assert!(e.errors[0].message.contains("no address, switch or variable"), "{:?}", e.errors);
    }

    #[test]
    fn binary_states_out_of_range() {
        let e = gen(vec![row("5", "Charlie_Binary(#LED, #InCh, 64)")]).unwrap_err();
        assert!(e.errors.iter().any(|d| d.message.contains("Anzahl der binären Zustände ungültig")), "{:?}", e.errors);
    }

    #[test]
    fn invalid_variable_numbers() {
        let e = gen(vec![row("SwitchE251", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert!(
            e.errors[0]
                .message
                .contains("Error: The number of the variable 'SwitchE251' is invalid! || Valid range: 1..250"),
            "{:?}",
            e.errors
        );
        let e = gen(vec![row("LOC_INCH251", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert!(
            e.errors[0]
                .message
                .contains("Error: The number of the variable 'LOC_INCH251' is invalid! || Valid range: 0..250"),
            "{:?}",
            e.errors
        );
    }

    #[test]
    fn loc_inch_zero_is_not_a_loc_inch_name() {
        // Is_in_Nr_String refuses the leading "0", so LOC_INCH0 is checked like any other name (Excel too)
        let e = gen(vec![row("LOC_INCH0", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert!(e.errors[0].message.contains("'LOC_INCH0' is used as an input"), "{:?}", e.errors);
    }

    #[test]
    fn an_unused_loc_inch_number_is_reported_but_written() {
        // Create_Loc_InCh_Defines shows the error after the header was built
        let g = gen(vec![row("LOC_INCH3", "Blink(#LED, #InCh, 1 Sec)")]).unwrap();
        assert!(g
            .warnings
            .iter()
            .any(
                |w| w.message.contains("Error: The variable 'LOC_INCH3' in line 2 is not defined") && w.row == Some(1)
            ));
    }

    #[test]
    fn return_messages_are_still_refused() {
        let e = gen(vec![row("RM 300.1", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert!(e.errors[0].message.contains("Return messages (RM)"), "{:?}", e.errors);
    }

    #[test]
    fn refused_rows_skip_the_undefined_check() {
        // The variable may be set by the refused LED_to_Var row: no misleading "never set"
        let e =
            gen(vec![row("", "LED_to_Var(Door, 0, 0, >, 10)"), row("Door", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert_eq!(e.errors.len(), 1, "{:?}", e.errors);
        assert!(e.errors[0].message.contains("LED_to_Var"));
    }

    // ---- Destination variables (M06_Write_Header_SW.bas: Get_Matching_Arg, Add_*_to_DstVars, Print_DstVar_List)

    fn dst_vars(lines: &[&str]) -> (String, String, Vec<Diagnostic>) {
        let l = layout(vec![]);
        let cat = Catalog::bundled();
        let mut g = Gen::new(&l, &l.sheets[0], &cat);
        for line in lines {
            g.add_inp_and_dst_vars(line, 0);
        }
        (g.dst_var_list.clone(), g.multi_set_dst_var_list.clone(), g.errors.clone())
    }

    // ---- Switches (M06_Write_Header_SW.bas: First_Scan_of_Data_Rows, Set_PinNrLst_if_Matching,
    // No_Duplicates_in_InpLists, Init_HeaderFile_Generation_SW, Write_Switches_Header_File_Part_A)

    /// A row with only a configuration line (no LED, no input)
    fn cfg(macro_text: &str) -> Row {
        Row { enabled: true, macro_text: macro_text.into(), ..Row::default() }
    }

    fn header(rows: Vec<Row>) -> String {
        match gen(rows) {
            Ok(g) => g.header.replace("\r\n", "\n"),
            Err(e) => panic!("{:?}", e.errors),
        }
    }

    fn first_error(rows: Vec<Row>) -> String {
        gen(rows).unwrap_err().errors[0].message.clone()
    }

    #[test]
    fn switches_a_b_d_and_a_variable() {
        // AM328: SwitchA_Pins A6, SwitchB_Pins A2, SwitchD_Pins 7 8 9, CLK A0, RST A3
        let h = header(vec![
            row("SwitchB3", "Blink(#LED, #InCh, 1 Sec)"),
            row("SwitchD2", "Blink(#LED, #InCh, 1 Sec)"),
            // A switch used as an argument counts too (Valid_Var_Name)
            row("", "MonoFlop(Door, SwitchA7, 1 Sec)"),
        ]);
        let expected = [
            "#ifndef CONFIG_ONLY\n//*** Analog switches ***\n\n#include <AnalogScanner.h>",
            "#include <Analog_Buttons10.h>\nAnalog_Buttons10_C AButtons1(A6);\n\n\n#endif //CONFIG_ONLY\n",
            "#define SwitchA7                6\n// Reserve channels: 1 because MobaLedLib_Copy_to_InpStruct always writes multiple of 8 channels\n#define START_SWITCHES_A  0 ",
            // 16 channels per analog button bank, then divisible by 4
            "//*** Digital switches ***\n\n#define SwitchB1                16\n",
            "#define SwitchB3                18\n// Reserve channels: 5 because",
            // SwitchD is not filled to 8
            "#define START_SWITCHES_D  24 ",
            "#define SwitchD2                25\n\n//*** Output Channels ***\n#define START_VARIABLES   26 ",
            "#define Door                    26\n",
            "const PROGMEM uint8_t SwitchD_Pins[] = { 7,8,9 };                  // Array of pins which read switches 'D'\n#define SWITCH_D_INP_CNT sizeof(SwitchD_Pins)\n",
            "#define CTR_CHANNELS_1    3 ",
            "#define CTR_CHANNELS_2    0 ",
            "#define BUTTON_INP_LIST_1 A2 ",
            "#define BUTTON_INP_LIST_2 Unused ",
            "#define CLK_PIN           A0 ",
            "#define RESET_PIN         A3 ",
            "#define START_SWITCHES_1  16 ",
            "#define START_SWITCHES_2  24 ",
            "#define START_SWITCHES_B  16 ",
            "#define START_SEND_INPUTS 0 ",
            // 16 + To_Eight(3 * 1) + 2 + 1
            "#define TOTAL_SEND_INPUTS 27 ",
            "#define TOTAL_SWITCHES_A  16 ",
            "#define TOTAL_SWITCHES_B  8 ",
            "#define TOTAL_SWITCHES_C  0 ",
            "#define TOTAL_SWITCHES_D  2 ",
            "#define TOTAL_VARIABLES   1 ",
            "{\n#ifndef CONFIG_ONLY\n  int scanOrder[] = {A6};\n  const int SCAN_COUNT = sizeof(scanOrder) / sizeof(scanOrder[0]);\n\n  scanner.setScanOrder(SCAN_COUNT, scanOrder);\n  scanner.beginScanning();\n#endif\n\n#ifndef CONFIG_ONLY\n  Keys_4017_Setup();",
            "  for (uint8_t i = 0; i < SWITCH_D_INP_CNT; i++)\n    pinMode(pgm_read_byte_near(&SwitchD_Pins[i]), INPUT_PULLUP);\n}\n",
            "  Button = AButtons1.Get(); MobaLedLib_Copy_to_InpStruct((uint8_t*)&Button, 2, 0);       \\\n",
            "  MobaLedLib_Copy_to_InpStruct(Keys_Array_1, KEYS_ARRAY_BYTE_SIZE_1, START_SWITCHES_1);  \\\n",
            "  for (uint8_t i = 0; i < 2; i++) \\\n      MobaLedLib.Set_Input(SwitchD1 + i, !digitalRead(pgm_read_byte_near(&SwitchD_Pins[i])));\\\n}\n",
            "  Blink(1, SwitchB3, 1 Sec) ",
        ];
        for e in expected {
            assert!(h.contains(e), "missing {e:?}");
        }
        assert!(!h.contains("LED_HEARTBEAT_PIN"));
    }

    #[test]
    fn analog_switches_skip_after_the_tenth() {
        // "(i - 1 Mod 16) = 9" is "i - 1 = 9" in VBA: the 6 channels are only skipped after SwitchA10
        let h = header(vec![cfg("// Set_SwitchA_InpLst(A6 A7)"), row("SwitchA12", "Blink(#LED, #InCh, 1 Sec)")]);
        assert!(h.contains("#define SwitchA10               9\n#define SwitchA11               16\n#define SwitchA12               17\n// Reserve channels: 4"));
        assert!(h.contains("Analog_Buttons10_C AButtons1(A6);\nAnalog_Buttons10_C AButtons2(A7);"));
        assert!(h.contains("  Button = AButtons2.Get(); MobaLedLib_Copy_to_InpStruct((uint8_t*)&Button, 2, 16);"));
        assert!(h.contains("#define TOTAL_SWITCHES_A  32 "));
        assert!(h.contains("  int scanOrder[] = {A6,A7};"));
        // More than 10 buttons per analog pin
        let e = first_error(vec![row("SwitchA11", "Blink(#LED, #InCh, 1 Sec)")]);
        assert!(e.contains("Error: More analog buttons were used than are possible."), "{e}");
    }

    #[test]
    fn keyboards_b_and_c() {
        // SwitchC_Pins "2 10 11 12 A5": 7 switches need 2 counter channels
        let h =
            header(vec![row("SwitchB2", "Blink(#LED, #InCh, 1 Sec)"), row("SwitchC7", "Blink(#LED, #InCh, 1 Sec)")]);
        for e in [
            "#define SwitchB2                1\n// Reserve channels: 6",
            "#define SwitchC1                8\n",
            "#define CTR_CHANNELS_1    2 ",
            "#define CTR_CHANNELS_2    2 ",
            "#define BUTTON_INP_LIST_2 2,10,11,12,A5 ",
            "#define START_SWITCHES_B  0 ",
            "#define START_SWITCHES_C  8 ",
            "#define TOTAL_SWITCHES_B  8 ",
            "#define TOTAL_SWITCHES_C  16 ",
            "  MobaLedLib_Copy_to_InpStruct(Keys_Array_2, KEYS_ARRAY_BYTE_SIZE_2, START_SWITCHES_2);  \\",
        ] {
            assert!(h.contains(e), "missing {e:?}");
        }
        // SwitchC alone is keyboard 1; its total is written as TOTAL_SWITCHES_C
        let h = header(vec![row("SwitchC3", "Blink(#LED, #InCh, 1 Sec)")]);
        assert!(h.contains("#define START_SWITCHES_C  0 "));
        assert!(h.contains("#define TOTAL_SWITCHES_B  0 "));
        assert!(h.contains("#define TOTAL_SWITCHES_C  8 "));
    }

    #[test]
    fn read_ldr_and_damping() {
        let h = header(vec![cfg("#define READ_LDR"), cfg("#define SWITCH_DAMPING_FACT 0.5")]);
        assert!(h.contains("#ifndef CONFIG_ONLY\n//*** Analog switches ***\n\n#include <AnalogScanner.h>"));
        assert!(!h.contains("Analog_Buttons10"));
        assert!(h.contains("\n#define SWITCH_DAMPING_FACT 0.5\n\n#include \"Read_LDR.h\"     // Darkness sensor\n\n#endif //CONFIG_ONLY\n"));
        assert!(h.contains("  int scanOrder[] = {A7};"));
        assert!(h.contains("  Init_DarknessSensor(A7, 50, SCAN_COUNT);"));
        assert!(h.contains("  scanner.setCallback(A7, Darkness_Detection_Callback);"));
        // No switches: no loop proc
        assert!(!h.contains("Additional_Loop_Proc"));
    }

    #[test]
    fn set_pin_lists() {
        // A pin alias with an indirection: PIN_ALIAS_J1_P2 = "=LDR_Pin" = A7
        let h = header(vec![cfg("#define READ_LDR"), cfg("// Set_LDR_Pin_Number(J1_P2)")]);
        assert!(h.contains("  int scanOrder[] = {A7};"));
        let h = header(vec![cfg("// Set_SwitchD_InpLst(2, 3)"), row("SwitchD2", "Blink(#LED, #InCh, 1 Sec)")]);
        assert!(h.contains("SwitchD_Pins[] = { 2,3 };"));
        let e = first_error(vec![cfg("// Set_SwitchD_InpLst(7 99)")]);
        // Like Excel: the English text has "|" without a line end
        assert_eq!(e, "Error: The pin '99' is not valid in | 'Set_SwitchD_InpLst(7 99)' command");
        let e = first_error(vec![cfg("// Set_SwitchD_InpLst(7 8 8 9)")]);
        assert_eq!(e, "Error: The pin '8' is used several times in | 'Set_SwitchD_InpLst(7 8 8 9)' command");
        // Like Excel: the second search is in the line with the brackets, so a pin that is first and last
        // (next to "(" and ")") is not found twice
        assert!(gen(vec![cfg("// Set_SwitchD_InpLst(7 8 7)")]).is_ok());
        for bad in ["// Set_CLK_Pin_Number(A0 A1)", "// Set_CLK_Pin_Number(A0", "// Set_CLK_Pin_Number()"] {
            assert!(first_error(vec![cfg(bad)]).starts_with("Error reading the pin numbers in line"), "{bad}");
        }
        // The SPI pins are only valid if the page isn't CAN
        let mut l = layout(vec![cfg("// Set_SwitchD_InpLst(10)")]);
        assert!(generate(&l, 0, &Catalog::bundled(), &opts()).is_ok());
        l.sheets[0].protocol = Protocol::Can;
        let e = generate(&l, 0, &Catalog::bundled(), &opts()).unwrap_err();
        assert!(e.errors[0].message.contains("The pin '10' is not valid"), "{:?}", e.errors);
    }

    #[test]
    fn duplicate_pins() {
        let e = first_error(vec![
            cfg("// Set_SwitchD_InpLst(A2)"),
            row("SwitchB1", "Blink(#LED, #InCh, 1 Sec)"),
            row("SwitchD1", "Blink(#LED, #InCh, 1 Sec)"),
        ]);
        assert!(
            e.starts_with(
                "Error: The input pin 'A2' of the Arduino is used as input in the two switch functions 'B' and 'D'."
            ),
            "{e}"
        );
        let e = first_error(vec![
            cfg("// Set_SwitchD_InpLst(A0)"),
            row("SwitchB1", "Blink(#LED, #InCh, 1 Sec)"),
            row("SwitchD1", "Blink(#LED, #InCh, 1 Sec)"),
        ]);
        assert!(
            e.contains("switch functions 'D' and at the same time as the Clock pin for SwitchB or SwitchC pin"),
            "{e}"
        );
        assert!(e.contains("'Set_CLK_Pin_Number()'"), "{e}");
        let e = first_error(vec![cfg("// Set_SwitchD_InpLst(6)"), row("SwitchD1", "Blink(#LED, #InCh, 1 Sec)")]);
        assert!(e.contains("at the same time as the LED pin"), "{e}");
    }

    #[test]
    fn more_switch_d_than_pins() {
        let e = first_error(vec![row("SwitchD4", "Blink(#LED, #InCh, 1 Sec)")]);
        assert!(e.ends_with("Last possible switch: 'SwitchD3'"), "{e}");
    }

    #[test]
    fn an_invalid_switch_number_stops_the_generation() {
        let e = gen(vec![row("SwitchB251", "Blink(#LED, #InCh, 1 Sec)"), row("Foo", "Blink(#LED, #InCh, 1 Sec)")])
            .unwrap_err();
        assert_eq!(e.errors.len(), 1, "{:?}", e.errors);
        assert!(e.errors[0]
            .message
            .contains("Error: The number of the variable 'SwitchB251' is invalid! || Valid range: 1..250"));
    }

    #[test]
    fn platform_without_analog_buttons() {
        let mut c = Catalog::bundled();
        c.platforms.get_mut("AM328").unwrap().insert("SupportsAnalogButtons".into(), "false".into());
        let e = generate(&layout(vec![row("SwitchA1", "Blink(#LED, #InCh, 1 Sec)")]), 0, &c, &opts()).unwrap_err();
        assert_eq!(e.errors[0].message, "Analog buttons are not supported on this platform.");
    }

    #[test]
    fn heartbeat_pin_and_spi_pins() {
        // PIN_A3_Is_Used: the CD4017 reset is on A3
        let mut l = layout(vec![row("SwitchB1", "Blink(#LED, #InCh, 1 Sec)")]);
        l.sheets[0].protocol = Protocol::Can;
        let h = generate(&l, 0, &Catalog::bundled(), &opts()).unwrap().header;
        assert!(h.contains("#define LED_HEARTBEAT_PIN -1           // Disable the heartbeat pin because it's used for the SwitchB or SwitchC"));
        // Check_Switch_Lists_for_SPI_Pins: SwitchC_Pins of the AM328 contain 10 11 12
        let mut l = layout(vec![row("1", "Blink(#LED, #InCh, 1 Sec)"), row("SwitchC1", "Blink(#LED, #InCh, 1 Sec)")]);
        l.settings.use_spi = true;
        let e = generate(&l, 0, &Catalog::bundled(), &opts()).unwrap_err();
        assert!(e.errors[0].message.starts_with("Error: The Arduino pin '10' cannot be used"), "{:?}", e.errors);
    }

    #[test]
    fn stored_keys_use_set_input() {
        // Add_Set_Input_Loop_for_Keys when values are stored in the EEPROM
        let h = header(vec![row("1", "Blink(#LED, #InCh, 1 Sec)"), row("SwitchB1", "Blink(#LED, #InCh, 1 Sec)")]);
        assert!(h.contains("{ IS_TOGGLE"));
        assert!(h.contains(&format!("{} \\\n", pad("  uint8_t channel1 = START_SWITCHES_1;", 70))));
        assert!(h.contains("      MobaLedLib.Set_Input(channel1++, tmp & 0x01);"));
        assert!(!h.contains("MobaLedLib_Copy_to_InpStruct(Keys_Array_1"));
    }

    #[test]
    fn get_matching_arg_like_vba() {
        // Test_Get_Matching_Arg
        let l = layout(vec![]);
        let cat = Catalog::bundled();
        let mut g = Gen::new(&l, &l.sheets[0], &cat);
        let random = "Random(        DstVar, InCh, RandMode, MinTime, MaxTime, MinOn, MaxOn)";
        assert_eq!(g.get_matching_arg(random, "Random( OutA, 1, 2, 3, 4, 5, 6)", "DstVar", 0).as_deref(), Some("OutA"));
        let counter = "Counter(       CtrMode, InCh, Enable, TimeOut, ...)";
        let line = "Counter(12, #InCh, Enable, TimeOut, OutA, OutB, OutB)";
        assert_eq!(g.get_matching_arg(counter, line, "...", 0).as_deref(), Some("OutA,OutB,OutB"));
        let randmux = "RandMux(       DstVar1, DstVarN, InCh, RandMode, MinTime, MaxTime)";
        let line = "RandMux( Out1, Out10, InCh, RandMode, MinTime, MaxTime)";
        assert_eq!(g.get_matching_arg(randmux, line, "DstVarN", 0).as_deref(), Some("Out10"));
        // Fewer arguments than the prototype: Excel's message
        assert_eq!(g.get_matching_arg(random, "Random( OutA, 1)", "DstVar", 0), None);
        assert!(g.errors[0].message.contains("Error recognizing target variable in macro"));
    }

    #[test]
    fn dst_var_out_cnt_1_and_2() {
        let (list, multi, errors) = dst_vars(&["MonoFlop(Door, SI_1, 1 Sec)", "RS_FlipFlop2(Up, Down, SI_1, SI_1)"]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(list, " Door Up Down ");
        assert_eq!(multi, " ");
    }

    #[test]
    fn dst_var_loc_in_ch_and_si_local_var_are_not_added() {
        let (list, _, errors) = dst_vars(&["MonoFlop(#LocInCh, SI_1, 1 Sec)", "MonoFlop(SI_LocalVar, SI_1, 1 Sec)"]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(list, " ");
    }

    #[test]
    fn dst_var_counter_out_list() {
        let (list, multi, errors) = dst_vars(&["Counter(CF_ROTATE, SI_1, SI_1, 0, A1, A2, A3)"]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(list, " A1 A2 A3 ");
        assert_eq!(multi, " ");
        // With CF_ONLY_LOCALVAR there are no destination variables
        let (list, _, errors) = dst_vars(&["Counter(CF_ONLY_LOCALVAR, SI_1, SI_1, 0, 5)"]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(list, " ");
    }

    #[test]
    fn dst_var_randmux_n2() {
        let (list, _, errors) = dst_vars(&["RandMux(Out1, Out3, SI_1, RM_NORMAL, 1 Sec, 2 Sec)"]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(list, " Out1 Out2 Out3 ");
        // Different names: Res stays False
        let (_, _, errors) = dst_vars(&["RandMux(Out1, Led3, SI_1, RM_NORMAL, 1 Sec, 2 Sec)"]);
        assert!(errors[0].message.contains("Error in the definition of the target variable (s)"), "{errors:?}");
    }

    #[test]
    fn dst_var_push_button_cx() {
        // PushButton_w_LED_BL_0_2: OutCnt C3
        let line = "PushButton_w_LED_BL_0_2(#LED, C1, SI_1, Btn0, 1, 0, 0, 0, 1 Min, 255, 20)";
        let (list, _, errors) = dst_vars(&[line]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(list, " Btn0 Btn1 Btn2 ");
        // A name without a number at the end
        let line = "PushButton_w_LED_BL_0_2(#LED, C1, SI_1, Btn, 1, 0, 0, 0, 1 Min, 255, 20)";
        let (_, _, errors) = dst_vars(&[line]);
        assert!(
            errors[0].message.contains("Error: The target variable 'Btn' in line 1 must have a number at the end"),
            "{errors:?}"
        );
    }

    #[test]
    fn dst_var_set_twice_is_a_warning() {
        let g = gen(vec![row("", "MonoFlop(Door, SI_1, 1 Sec)"), row("", "MonoFlop(Door, SI_1, 2 Sec)")]).unwrap();
        assert!(
            g.warnings
                .iter()
                .any(|w| w.message.contains("The following target variables are set several times:\n Door ")),
            "{:?}",
            g.warnings
        );
    }

    #[test]
    fn dst_vars_in_the_header() {
        let g = gen(vec![
            row("", "RS_FlipFlop2(Up, Down, SI_1, SI_1)"),
            row("Up", "Blink(#LED, #InCh, 1 Sec)"),
            row("Down", "Blink(#LED, #InCh, 1 Sec)"),
        ])
        .unwrap();
        let h = &g.header;
        assert!(h.contains("//*** Output Channels ***\r\n"));
        let start = h.find("#define START_VARIABLES").unwrap();
        let vars = &h[start..];
        let up = vars.find("#define Up                      ").unwrap();
        let down = vars.find("#define Down                    ").unwrap();
        assert!(up < down);
        assert!(h.contains("#define TOTAL_SEND_INPUTS 2 "));
        assert!(h.contains("#define TOTAL_VARIABLES   2 "));
        // The rows with the variable as address use its #define name as input channel
        assert!(h.contains("  Blink(2, Up, 1 Sec)"));
        assert!(h.contains("  Blink(3, Down, 1 Sec)"));
    }

    #[test]
    fn logic_arguments() {
        let l = layout(vec![]);
        let cat = Catalog::bundled();
        let mut g = Gen::new(&l, &l.sheets[0], &cat);
        assert!(g.add_inp_and_dst_vars("Logic(Out, #InCh OR Foo AND NOT SI_1)", 0));
        assert_eq!(g.undefined_input_var, "Foo ");
        assert!(g.errors.is_empty());
        assert!(!g.add_inp_and_dst_vars("Logic(Out, #InCh+Foo)", 0));
        assert!(g.errors[0].message.contains("constant positive number"));
        assert!(!g.add_inp_and_dst_vars("Logic(Out)", 0));
        assert!(g.errors[1].message.contains("Wrong number of parameters"));
    }

    #[test]
    fn bankers_rounding() {
        assert_eq!(vba_round(2.5), 2);
        assert_eq!(vba_round(3.5), 4);
        assert_eq!(vba_round(2.4), 2);
    }
}
