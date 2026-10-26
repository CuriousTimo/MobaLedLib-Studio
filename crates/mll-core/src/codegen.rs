//! Generation of `LEDs_AutoProg.h`. Port of `M06_Write_Header.bas` (and the parts of
//! `M06_Write_Header_SW.bas` that are always written).
//!
//! The output is meant to be identical to the Excel tool's, apart from the program name
//! and the creation time. Line endings are CRLF like VBA's `Print #`.

use chrono::{NaiveDateTime, Timelike};
use serde::Serialize;

use crate::catalog::{call_args, Catalog};
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
        } else if !address.is_empty() {
            self.err(
                r,
                "Variable names and return messages (RM) as address are not supported by MobaLedLib Studio yet",
            );
            return;
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
    for r in 0..sh.rows.len() {
        g.process_row(r);
    }
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

    #[test]
    fn bankers_rounding() {
        assert_eq!(vba_round(2.5), 2);
        assert_eq!(vba_round(3.5), 4);
        assert_eq!(vba_round(2.4), 2);
    }
}
