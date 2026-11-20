//! Generation of `LEDs_AutoProg.h`. Port of `M06_Write_Header.bas` (and the parts of
//! `M06_Write_Header_SW.bas` that are always written).
//!
//! The output is meant to be identical to the Excel tool's, apart from the program name
//! and the creation time. Line endings are CRLF like VBA's `Print #`.

use chrono::{NaiveDateTime, Timelike};
use serde::Serialize;

use crate::catalog::{call_args, prototype_params, Catalog};
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
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions {
            program: format!("MobaLedLib Studio {}", env!("CARGO_PKG_VERSION")),
            lib_version: String::new(),
            now: chrono::Local::now().naive_local(),
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
/// Effects that write to destination variables (MonoFlop, RS_FlipFlop, Counter, RandMux, PushButton…).
/// Excel declares these variables in the header (`Add_Inp_and_DstVars`, `Print_DstVar_List`); without that
/// the header doesn't compile. `#LocInCh` and `SI_LocalVar` need no declaration.
fn uses_destination_variables(row: &Row, catalog: &Catalog) -> bool {
    row.macro_text.lines().any(|line| {
        let Some(def) = catalog.find_macro(line) else { return false };
        if matches!(def.out_cnt.trim(), "" | "0") {
            return false;
        }
        if line.trim_start().starts_with("Counter(") && line.contains("CF_ONLY_LOCALVAR") {
            return false;
        }
        let params = prototype_params(&def.prototype);
        let args = call_args(line);
        let dst: Vec<&str> = params
            .iter()
            .zip(&args)
            .filter(|(p, _)| p.starts_with("DstVar") || p.as_str() == "OutList")
            .map(|(_, a)| a.trim())
            .collect();
        // No recognisable destination parameter: assume it has one
        dst.is_empty() || dst.iter().any(|a| !a.is_empty() && !matches!(*a, "#LocInCh" | "SI_LocalVar"))
    })
}

fn unsupported_feature(row: &Row) -> Option<String> {
    let text = &row.macro_text;
    let checks: [(&str, &str); 9] = [
        ("include(", "Including other sheets (include)"),
        ("LED_to_Var(", "LED_to_Var"),
        ("SOUND_CHANNEL_DEFINITON(", "Serial sound channels"),
        ("Mainboard_LED(", "Mainboard_LED"),
        ("Multiplexer", "Multiplexer"),
        ("Set_LED_OutpPinLst(", "Set_LED_OutpPinLst"),
        ("_InpLst(", "Changing the switch input pins"),
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
    let uses_switch = |s: &str| {
        ["SwitchA", "SwitchB", "SwitchC", "SwitchD"].iter().any(|sw| {
            s.match_indices(sw).any(|(i, _)| s[i + sw.len()..].chars().next().is_some_and(|c| c.is_ascii_digit()))
        })
    };
    if uses_switch(text) || uses_switch(&row.address) {
        return Some("Switches (SwitchA..D) are not supported by MobaLedLib Studio yet".into());
    }
    None
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
    /// `DstVar_List`: the variables effects write to (step 2: still empty, those rows are refused)
    dst_var_list: String,
    /// `Undefined_Input_Var` / `Undef_Input_Var_Row`: input variables not defined when they were checked
    undefined_input_var: String,
    undef_input_var_rows: Vec<usize>,
    /// `MaxUsed_Loc_InCh` / `MaxUsed_Loc_InCh_Row`
    max_used_loc_in_ch: i64,
    max_used_loc_in_ch_row: usize,
    /// A row was refused as not supported yet: the variables it would define are unknown
    refused_rows: bool,
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
}

impl<'a> Gen<'a> {
    fn new(layout: &'a Layout, sheet: &'a Sheet, catalog: &'a Catalog) -> Self {
        let led_numbers = leds::compute(sheet);
        let mut leds_per_channel = led_numbers.per_channel;
        let num: i64 = leds_per_channel.iter().sum();
        if num < MIN_LEDS {
            leds_per_channel[0] += MIN_LEDS - num;
        }
        let mut start_led_channel = [0i64; LED_CHANNELS];
        for nr in 1..LED_CHANNELS {
            start_led_channel[nr] = start_led_channel[nr - 1] + leds_per_channel[nr - 1];
        }
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
            undefined_input_var: String::new(),
            undef_input_var_rows: Vec::new(),
            max_used_loc_in_ch: -1,
            max_used_loc_in_ch_row: 0,
            refused_rows: false,
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
        }
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
    /// name with an invalid number, 0: no switch. The switch counters come with the switches (step 3);
    /// rows using SwitchA..D are refused before this is reached.
    fn is_switch_var(&mut self, name: &str, row: usize) -> i64 {
        match is_in_nr_string(name, "Switch?", 1, 250) {
            (1, _) => 1,
            (-1, _) => {
                self.err(row, format!("The number of the variable '{name}' is invalid (valid range: 1..250)"));
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
            (-1, _) => self.err(row, format!("The number of the variable '{name}' is invalid (valid range: 0..250)")),
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
                self.err(row, "Error in the logic expression: only a constant positive number may be added to '#InCh'");
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
            self.err(r, "There is no address, switch or variable in this row");
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
                self.err(r, "Invalid number of binary states. It must be between 1 and 63");
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
                self.err(r, format!("Wrong number of parameters in 'Logic()' expression: '{line}'"));
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

    /// `Add_Inp_and_DstVars` (M06_Write_Header_SW.bas), the input part. The destination variables come with
    /// the effects that set variables (step 2).
    fn add_inp_and_dst_vars(&mut self, line: &str, r: usize) -> bool {
        if !line.contains('(') {
            return true;
        }
        let parts: Vec<&str> = line.split('(').collect();
        if !parts[1].contains(')') {
            return true;
        }
        let search = if parts[0] == "HouseT" { "House" } else { parts[0] };
        let Some(def) = self.find_macro_in_lib(&format!("{search}(")) else { return true };
        self.add_inp_vars(parts[0], def, line, r)
    }

    /// `First_Scan_of_Data_Rows` (M06_Write_Header_SW.bas), the variable part: before the rows are
    /// processed, the input variables of every effect line are checked. Rows that are refused as not
    /// supported yet are skipped: they don't reach the header.
    fn first_scan(&mut self) {
        let sheet = self.sheet;
        for (r, row) in sheet.rows.iter().enumerate() {
            if !row.enabled || unsupported_feature(row).is_some() || uses_destination_variables(row, self.catalog) {
                continue;
            }
            for line in row.macro_text.split('\n') {
                if !self.add_inp_and_dst_vars(line.trim(), r) {
                    break;
                }
            }
        }
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
                self.err(row, format!("The variable '{var}' is used as an input, but it is never set"));
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
                self.err(r, format!("Start value '{sv}': this effect can't store its state"));
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
            self.err(r, "Negative start value");
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
            self.err(r, format!("Start value is too big. Maximum: {}", mask - 1));
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
                self.err(r, "The LED number must not be negative (check a previous Next_LED)");
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
                        self.err(r, format!("Address {a} is already used with another type"));
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
            self.err(r, format!("Invalid LED channel. Channels 0-{} are allowed", LED_CHANNELS - 1));
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
                    self.err(r, "Too many inputs to store (> 63)");
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
                self.err(r, format!("InCnt '{in_cnt}' is invalid"));
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
        if uses_destination_variables(row, self.catalog) {
            self.refused_rows = true;
            self.err(r, "Effects that set variables (DstVar) are not supported by MobaLedLib Studio yet");
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
                            self.err(r, format!("Wrong bit position '{bit_p}'"));
                        }
                    } else {
                        self.err(r, "Wrong Selectrix channel");
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
                        self.err(r, "This effect is always active and can't be switched by an address");
                    }
                    None => {}
                    Some(a) if a >= 1 && a <= max_addr => addr = Addr::Num(a),
                    Some(_) => self.err(r, format!("The address '{}' is invalid", address.replace('\n', " "))),
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

    fn write_switches_part_a(&mut self, out: &mut String) {
        // No switches are used in v1: only the totals and the FastLED setup are written
        let p = |out: &mut String, s: &str| {
            out.push_str(s);
            out.push_str(NL)
        };
        p(out, "");
        p(out, &format!("#define TOTAL_SWITCHES_A  {}// Number of used inputs for analog keyboard", pad("0", 41)));
        p(out, &format!("#define TOTAL_SWITCHES_B  {}// Number of used inputs for keyboard 1", pad("0", 41)));
        p(out, &format!("#define TOTAL_SWITCHES_C  {}// Number of used inputs for keyboard 2", pad("0", 41)));
        p(out, &format!("#define TOTAL_SWITCHES_D  {}// Number of used inputs for main board switches", pad("0", 41)));
        p(out, &format!("#define TOTAL_VARIABLES   {}// Number of used variables", pad("0", 41)));

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
            self.errors.push(Diagnostic {
                row: None,
                message: format!(
                    "More LED channels are used than LED output pins are defined ({} needed)",
                    exp_out_pins + 1
                ),
            });
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
    }

    fn write(&mut self, opts: &GenOptions) -> String {
        let num_leds: i64 = self.leds_per_channel.iter().sum();
        let platform = self.layout.board.platform;
        let max_led = self.catalog.platform(platform.id(), "MaxLed").map(val_i).unwrap_or(256);
        if num_leds > max_led {
            self.errors.push(Diagnostic {
                row: None,
                message: format!("Too many LEDs: {num_leds}. At most {max_led} RGB LEDs are possible on this board"),
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
                message: format!("Error: the variable 'LOC_INCH{}' is not defined", self.max_used_loc_in_ch),
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
            self.errors.push(Diagnostic {
                row: None,
                message: "House(): the minimal time must be smaller or equal to the maximal time".into(),
            });
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
                p!("#define LED_HEARTBEAT_PIN A3           // Don't use the internal heartbeat LED because the D13 pins between LED and DCC arduin are connected together");
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
        self.write_switches_part_a(&mut o);
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
                message: format!(
                    "Too many input channels: at most 250 are available, the configuration uses {}",
                    self.channel - 1
                ),
            });
        }
        if self.config_txt.is_empty() {
            self.warnings.push(Diagnostic {
                row: None,
                message: "No effect is active: the program will not control any LED".into(),
            });
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
                message: format!("{} is not supported on {}", sh.protocol.page_id(), layout.board.platform.id()),
            }],
        });
    }
    let mut g = Gen::new(layout, sh, catalog);
    // Create_HeaderFile: Init_HeaderFile_Generation_SW (first scan), the rows, Check_Detected_Variables
    g.first_scan();
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
        }
    }

    fn catalog() -> Catalog {
        let mut c = Catalog::default();
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
            address: "SwitchD1".into(),
            macro_text: "Const(#LED, C1, #InCh, 0, 255)".into(),
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
        assert!(e.errors[0].message.contains("'Foo' is used as an input, but it is never set"));
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
        assert!(e.errors.iter().any(|d| d.message.contains("binary states")), "{:?}", e.errors);
    }

    #[test]
    fn invalid_variable_numbers() {
        let e = gen(vec![row("SwitchE251", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert!(e.errors[0].message.contains("'SwitchE251' is invalid (valid range: 1..250)"), "{:?}", e.errors);
        let e = gen(vec![row("LOC_INCH251", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert!(e.errors[0].message.contains("'LOC_INCH251' is invalid (valid range: 0..250)"), "{:?}", e.errors);
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
        assert!(g.warnings.iter().any(|w| w.message.contains("'LOC_INCH3' is not defined") && w.row == Some(1)));
    }

    #[test]
    fn return_messages_are_still_refused() {
        let e = gen(vec![row("RM 300.1", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert!(e.errors[0].message.contains("Return messages (RM)"), "{:?}", e.errors);
    }

    #[test]
    fn refused_rows_skip_the_undefined_check() {
        // The variable may be set by the refused DstVar row: no misleading "never set"
        let e =
            gen(vec![row("", "MonoFlop(Door, SI_1, 1 Sec)"), row("Door", "Blink(#LED, #InCh, 1 Sec)")]).unwrap_err();
        assert_eq!(e.errors.len(), 1, "{:?}", e.errors);
        assert!(e.errors[0].message.contains("DstVar"));
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
