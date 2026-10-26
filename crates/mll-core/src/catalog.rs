//! The macro catalog: every effect the library offers, its parameters, and the
//! platform parameters. Extracted from the Excel workbook's hidden sheets
//! (`Lib_Macros`, `Par_Description`, `Platform_Parameters`) by `mll extract-catalog`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Error;

/// Language codes in the order of the Excel language columns.
pub const LANGUAGES: [&str; 7] = ["de", "en", "nl", "fr", "it", "es", "da"];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MacroText {
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub short: String,
    #[serde(default)]
    pub long: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroDef {
    /// Macro name, e.g. "House"
    pub name: String,
    /// Prototype with parameter names, e.g. "House(LED,InCh, On_Min,On_Limit, ...)"
    pub prototype: String,
    /// Search key used by Excel to recognise a macro in the macro column, e.g. "House("
    pub find: String,
    /// Special dialog: "" (generic), "House", "ColTab", "EX.Macro", ...
    #[serde(default)]
    pub dialog: String,
    #[serde(default)]
    pub mode: String,
    /// LEDs column template: a number, "n", "Cx", "C1-2", "S", "LedCnt..."
    #[serde(default)]
    pub leds: String,
    /// InCnt template: a number, "n", "States", "BinStates", "Logic", "2?"
    #[serde(default)]
    pub in_cnt: String,
    #[serde(default)]
    pub out_cnt: String,
    #[serde(default)]
    pub loc_in_ch: String,
    /// Number of single channels used by Cx macros
    #[serde(default)]
    pub single_led_cnt: String,
    #[serde(default)]
    pub default_channel: String,
    /// Status storage type (MST_*), "Type" column
    #[serde(default)]
    pub store_type: String,
    #[serde(default)]
    pub list_sort: i64,
    #[serde(default)]
    pub tree_sort: i64,
    /// Icon names separated by "|"
    #[serde(default)]
    pub icons: String,
    /// Texts per language code
    #[serde(default)]
    pub text: BTreeMap<String, MacroText>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParamText {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub name: String,
    /// "", "Time", "List", "Mode", "RGB", "RGB2", "CX", "Var", "InpVar", "Logic", ...
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub min: String,
    #[serde(default)]
    pub max: String,
    #[serde(default)]
    pub default: String,
    /// Raw options, e.g. "w=140;Values: MP3_NOP,MP3_NEXT"
    #[serde(default)]
    pub options: String,
    #[serde(default)]
    pub text: BTreeMap<String, ParamText>,
}

impl ParamDef {
    /// Choices of a "List" parameter.
    pub fn values(&self) -> Vec<String> {
        self.options
            .split(';')
            .find_map(|p| p.trim().strip_prefix("Values:"))
            .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    /// Version of the MobaLedLib the catalog was extracted from
    #[serde(default)]
    pub lib_version: String,
    pub macros: Vec<MacroDef>,
    pub params: Vec<ParamDef>,
    /// Platform parameters: platform id ("AM328", ...) -> setting -> value
    #[serde(default)]
    pub platforms: BTreeMap<String, BTreeMap<String, String>>,
}

impl Catalog {
    pub fn from_json(s: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(s)?)
    }

    /// The catalog that ships with the application.
    pub fn bundled() -> Self {
        Self::from_json(include_str!("../../../data/catalog.json")).expect("bundled catalog is valid")
    }

    /// Find the macro used in a macro column entry (`Find_Macro_in_Lib_Macros_Sheet`).
    pub fn find_macro(&self, macro_text: &str) -> Option<&MacroDef> {
        let first = macro_text.trim_start();
        let name = first.split('(').next()?.trim();
        if name.is_empty() {
            return None;
        }
        let key = format!("{name}(");
        self.macros.iter().find(|m| m.find == key).or_else(|| self.macros.iter().find(|m| m.name == name))
    }

    pub fn param(&self, name: &str) -> Option<&ParamDef> {
        self.params.iter().find(|p| p.name == name)
    }

    pub fn platform(&self, platform: &str, key: &str) -> Option<&str> {
        self.platforms.get(platform)?.get(key).map(|s| s.as_str())
    }

    /// `fill_macro` with the values checked like the effect dialog does (`Check_Par_with_ErrMsg`):
    /// color lists ("10 0 0", "0 127 0  0 0 0") become separate arguments ("10, 0, 0").
    pub fn fill(
        &self,
        def: &MacroDef,
        values: &BTreeMap<String, String>,
        extra: &[String],
    ) -> Result<FilledMacro, Error> {
        let values = values
            .iter()
            .map(|(k, v)| {
                let color = self.param(k).is_some_and(|p| p.kind == "RGB" || p.kind == "RGB2");
                let v = if color {
                    v.replace(',', " ").split_whitespace().collect::<Vec<_>>().join(", ")
                } else {
                    v.clone()
                };
                (k.clone(), v)
            })
            .collect();
        fill_macro(def, &values, extra)
    }
}

/// Parameter names of a prototype like "Blink3(LED,Cx,InCh,Pause,Act,Val0,Val1,Off)".
pub fn prototype_params(prototype: &str) -> Vec<String> {
    let Some(open) = prototype.find('(') else { return Vec::new() };
    let inner = prototype[open + 1..].trim_end();
    let inner = inner.strip_suffix(')').unwrap_or(inner);
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(',').map(|s| s.trim().to_string()).collect()
}

/// Arguments of a filled macro call like "Blink3(#LED, C1, #InCh, 500, 500, 0, 255, 0)".
pub fn call_args(call: &str) -> Vec<String> {
    let Some(open) = call.find('(') else { return Vec::new() };
    let close = call.rfind(')').unwrap_or(call.len());
    if close <= open {
        return Vec::new();
    }
    let inner = &call[open + 1..close];
    if inner.trim().is_empty() {
        return Vec::new();
    }
    // Split on top level commas only
    let mut args = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in inner.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c)
            }
            ')' => {
                depth -= 1;
                cur.push(c)
            }
            ',' if depth == 0 => {
                args.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    args.push(cur.trim().to_string());
    args
}

/// `Cx_to_LED_Channel`: value of the Cx parameter -> content of the LEDs column.
pub fn cx_to_leds(cx: &str, single_led_cnt: i64) -> String {
    let n = single_led_cnt.abs().max(1);
    match cx.trim() {
        "C_ALL" => "1".into(),
        "C1" => format!("C1-{}", n),
        "C2" => format!("C2-{}", 1 + n),
        "C3" => format!("C3-{}", 2 + n),
        "C12" => "C1-2".into(),
        "C23" => "C2-3".into(),
        _ => String::new(),
    }
}

/// A macro filled in by the user: everything a row needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilledMacro {
    pub macro_text: String,
    pub leds: String,
    pub in_cnt: u32,
    pub loc_in_ch: u32,
}

/// Build the row content from a macro and its argument values (port of `Proc_General`).
///
/// `values` maps parameter names to their values. `LED`/`B_LED` and `InCh` are filled
/// automatically. A trailing `...` parameter takes `extra` (e.g. the rooms of `House`).
pub fn fill_macro(def: &MacroDef, values: &BTreeMap<String, String>, extra: &[String]) -> Result<FilledMacro, Error> {
    let params = prototype_params(&def.prototype);
    let mut args: Vec<String> = Vec::new();
    for p in &params {
        match p.as_str() {
            "LED" | "B_LED" => args.push("#LED".into()),
            "InCh" => args.push("#InCh".into()),
            "..." => args.extend(extra.iter().cloned()),
            // Placeholders like #LED, #InCh and #LocInCh are filled in when the header is generated
            name if name.starts_with('#') => args.push(name.to_string()),
            "" => {}
            name => {
                let v = values.get(name).map(|s| s.trim()).unwrap_or("");
                if v.is_empty() {
                    return Err(Error::Invalid(format!("parameter '{name}' of {} is empty", def.name)));
                }
                args.push(v.to_string());
            }
        }
    }
    let macro_text = if let Some(open) = def.prototype.find('(') {
        // The prototype's own head, not the name: "// Set_LEDNr(" must stay a comment
        format!("{}({})", def.prototype[..open].trim(), args.join(", "))
    } else {
        def.prototype.trim().to_string()
    };

    // Special_ConstrWarnLight: the LED count is part of the name, and only the Flash variant has WaitT
    let (macro_text, special_leds) = match (def.prototype.split('(').next().map(str::trim), args.as_slice()) {
        (Some("ConstrWarnLight"), [led, inch, cnt, min, max, on_t, wait_t, wait_e]) => {
            let flash = crate::vba::val(wait_t) > 0.0;
            let mut a = vec![led, inch, min, max, on_t];
            if flash {
                a.push(wait_t);
            }
            a.push(wait_e);
            let name = if flash { "ConstrWarnLightFlash" } else { "ConstrWarnLight" };
            let a: Vec<&str> = a.into_iter().map(|s| s.as_str()).collect();
            (format!("{name}{}({})", cnt.trim(), a.join(", ")), Some(format!("C1-{}", cnt.trim())))
        }
        _ => (macro_text, None),
    };

    let named = |n: &str| -> Option<String> { params.iter().position(|p| p == n).and_then(|_| values.get(n).cloned()) };

    let single = crate::vba::val_i(&def.single_led_cnt);
    let leds = match def.leds.trim() {
        _ if special_leds.is_some() => special_leds.unwrap_or_default(),
        "Cx" => {
            let cx = named("Cx").or_else(|| named("B_LED_Cx")).unwrap_or_default();
            let leds = cx_to_leds(&cx, single);
            if leds.is_empty() {
                // Without it the effect would get no LED number and silently drop off the LED bus
                return Err(Error::Invalid(format!(
                    "the LED channel of {} must be C1, C2, C3, C12, C23 or C_ALL, not '{cx}'",
                    def.name
                )));
            }
            leds
        }
        "n" => extra.len().to_string(),
        l if l.starts_with("LedCnt") => {
            let v = named(l).unwrap_or_default();
            if l == "LedCnt_Cpy" {
                crate::vba::val_i(&v).abs().to_string()
            } else {
                v
            }
        }
        l => l.to_string(),
    };

    let in_cnt = match def.in_cnt.trim() {
        "n" => crate::vba::val_i(&named("InCh_Cnt").unwrap_or_default()),
        "States" => crate::vba::val_i(&named("States").unwrap_or_default()),
        "BinStates" => {
            let states = crate::vba::val_i(&named("BinStates").unwrap_or_default()).max(1);
            // Get_BinSize: number of bits to store the states
            let mut bits = 0;
            while (1i64 << bits) < states {
                bits += 1;
            }
            bits
        }
        "2?" => {
            let n = args.iter().filter(|a| a.contains("#InCh")).count() as i64;
            if n == 0 {
                2
            } else {
                n
            }
        }
        "Logic" => {
            return Err(Error::Invalid(format!(
                "{} is not supported by the effect dialog yet; enter the macro in the expert fields",
                def.name
            )))
        }
        s => crate::vba::val_i(s),
    };

    Ok(FilledMacro {
        macro_text,
        leds,
        in_cnt: in_cnt.max(0) as u32,
        loc_in_ch: crate::vba::val_i(&def.loc_in_ch).max(0) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(proto: &str, leds: &str, in_cnt: &str, single: &str) -> MacroDef {
        MacroDef {
            name: proto.split('(').next().unwrap().trim().into(),
            prototype: proto.into(),
            find: format!("{}(", proto.split('(').next().unwrap().trim()),
            dialog: String::new(),
            mode: String::new(),
            leds: leds.into(),
            in_cnt: in_cnt.into(),
            out_cnt: String::new(),
            loc_in_ch: String::new(),
            single_led_cnt: single.into(),
            default_channel: String::new(),
            store_type: String::new(),
            list_sort: 0,
            tree_sort: 0,
            icons: String::new(),
            text: BTreeMap::new(),
        }
    }

    #[test]
    fn parses_prototypes_and_calls() {
        assert_eq!(
            prototype_params("House(         LED,InCh, On_Min,On_Limit, ...)"),
            vec!["LED", "InCh", "On_Min", "On_Limit", "..."]
        );
        assert_eq!(call_args("Logic(#InCh, A OR (B AND C), 3)"), vec!["#InCh", "A OR (B AND C)", "3"]);
    }

    #[test]
    fn fills_house() {
        let d = def("House(LED,InCh, On_Min,On_Limit, ...)", "n", "1", "");
        let mut v = BTreeMap::new();
        v.insert("On_Min".into(), "2".into());
        v.insert("On_Limit".into(), "5".into());
        let f = fill_macro(&d, &v, &["ROOM_DARK".into(), "ROOM_BRIGHT".into(), "NEON_LIGHT".into()]).unwrap();
        assert_eq!(f.macro_text, "House(#LED, #InCh, 2, 5, ROOM_DARK, ROOM_BRIGHT, NEON_LIGHT)");
        assert_eq!(f.leds, "3");
        assert_eq!(f.in_cnt, 1);
    }

    #[test]
    fn fills_single_channel_macro() {
        let d = def("Const(LED, Cx, InCh, Val0, Val1)", "Cx", "1", "1");
        let mut v = BTreeMap::new();
        v.insert("Cx".into(), "C2".into());
        v.insert("Val0".into(), "0".into());
        v.insert("Val1".into(), "255".into());
        let f = fill_macro(&d, &v, &[]).unwrap();
        assert_eq!(f.macro_text, "Const(#LED, C2, #InCh, 0, 255)");
        assert_eq!(f.leds, "C2-2");
    }

    #[test]
    fn constr_warn_light_like_excel() {
        // Test_Special_ConstrWarnLight in the workbook
        let d =
            def("ConstrWarnLight(LED,InCh, LEDcnt, MinBrightness, MaxBrightness, OnT, WaitT, WaitE)", "C1-n", "1", "");
        let mut v = BTreeMap::new();
        for (k, x) in [
            ("LEDcnt", "6"),
            ("MinBrightness", "20"),
            ("MaxBrightness", "255"),
            ("OnT", "100 ms"),
            ("WaitT", "0 ms"),
            ("WaitE", "300 ms"),
        ] {
            v.insert(k.into(), x.into());
        }
        let f = fill_macro(&d, &v, &[]).unwrap();
        assert_eq!(
            (f.macro_text.as_str(), f.leds.as_str()),
            ("ConstrWarnLight6(#LED, #InCh, 20, 255, 100 ms, 300 ms)", "C1-6")
        );
        v.insert("WaitT".into(), "50 ms".into());
        let f = fill_macro(&d, &v, &[]).unwrap();
        assert_eq!(f.macro_text, "ConstrWarnLightFlash6(#LED, #InCh, 20, 255, 100 ms, 50 ms, 300 ms)");
    }

    #[test]
    fn comment_macros_stay_comments() {
        let d = def("// Set_LEDNr(LedCnt_Set)", "LedCnt_Set", "", "");
        let mut v = BTreeMap::new();
        v.insert("LedCnt_Set".into(), "10".into());
        let f = fill_macro(&d, &v, &[]).unwrap();
        assert_eq!((f.macro_text.as_str(), f.leds.as_str()), ("// Set_LEDNr(10)", "10"));
    }

    #[test]
    fn hash_placeholders_are_kept() {
        let d = def("Flash(LED, Cx, InCh, #LocInCh, MinTime, MaxTime)", "Cx", "1", "1");
        let mut v = BTreeMap::new();
        for (k, x) in [("Cx", "C1"), ("MinTime", "1 Sec"), ("MaxTime", "5 Sec")] {
            v.insert(k.into(), x.into());
        }
        assert_eq!(fill_macro(&d, &v, &[]).unwrap().macro_text, "Flash(#LED, C1, #InCh, #LocInCh, 1 Sec, 5 Sec)");
    }

    #[test]
    fn servo_gets_its_channel_on_the_led_bus() {
        let d = def("Servo2(LED, InCh, B_LED_Cx, Servo21, Servo22, ServoT)", "Cx", "2", "-1");
        let mut v = BTreeMap::new();
        for (k, x) in [("B_LED_Cx", "C2"), ("Servo21", "1"), ("Servo22", "220"), ("ServoT", "5")] {
            v.insert(k.into(), x.into());
        }
        let f = fill_macro(&d, &v, &[]).unwrap();
        assert_eq!(f.macro_text, "Servo2(#LED, #InCh, C2, 1, 220, 5)");
        assert_eq!(f.leds, "C2-2");

        v.insert("B_LED_Cx".into(), "1".into());
        assert!(fill_macro(&d, &v, &[]).is_err(), "an unknown channel must not give a servo without LEDs");
    }

    #[test]
    fn missing_parameter_is_an_error() {
        let d = def("Const(LED, Cx, InCh, Val0, Val1)", "Cx", "1", "1");
        assert!(fill_macro(&d, &BTreeMap::new(), &[]).is_err());
    }
}
