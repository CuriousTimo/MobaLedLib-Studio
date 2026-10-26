//! Adds every effect of the catalog the way the effect gallery does (with the default values of its
//! form) and runs LED numbering and header generation on it.

use std::collections::BTreeMap;

use mll_core::catalog::{Catalog, MacroDef};
use mll_core::codegen::{generate, GenOptions};
use mll_core::leds;
use mll_core::model::{InputType, Layout, Platform, Protocol, Row};

/// Parameter names of a prototype, like `protoParams()` in app.js.
fn proto_params(proto: &str) -> Vec<String> {
    let Some(i) = proto.find('(') else { return Vec::new() };
    let inner = proto[i + 1..].trim_end().trim_end_matches(')');
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(',').map(|s| s.trim().to_string()).collect()
}

/// The form defaults, like `selectMacro()` in app.js.
fn form_defaults(c: &Catalog, def: &MacroDef) -> (BTreeMap<String, String>, Vec<String>) {
    let mut values = BTreeMap::new();
    let mut extra = Vec::new();
    for n in proto_params(&def.prototype) {
        if n == "..." {
            extra = if def.name.starts_with("GasLight") {
                vec!["GAS_LIGHT".into(), "GAS_LIGHT".into()]
            } else {
                vec!["ROOM_DARK".into(), "ROOM_BRIGHT".into(), "ROOM_WARM_W".into()]
            };
            break;
        }
        if ["LED", "B_LED", "InCh", ""].contains(&n.as_str()) || n.starts_with('#') {
            continue;
        }
        let p = c.params.iter().find(|p| p.name == n);
        let v = match p {
            Some(p) if !p.default.is_empty() => p.default.clone(),
            Some(p) if p.kind == "CX" => "C1".into(),
            _ => String::new(),
        };
        values.insert(n, v);
    }
    (values, extra)
}

/// Fields without a default need the user; fill them with a plausible value of their type.
fn with_test_input(c: &Catalog, mut values: BTreeMap<String, String>) -> (BTreeMap<String, String>, bool) {
    let mut needed = false;
    for (n, v) in values.iter_mut().filter(|(_, v)| v.is_empty()) {
        needed = true;
        let p = c.params.iter().find(|p| &p.name == n);
        *v = match p {
            Some(p) if !p.min.is_empty() => p.min.clone(),
            Some(p) if p.kind == "Time" => "1 Sec".into(),
            _ if n.contains("Var") => "TestVar".into(),
            _ if n.contains("List") => "4".into(),
            _ => "1".into(),
        };
    }
    (values, needed)
}

#[test]
fn every_effect_with_its_form_defaults() {
    let c = Catalog::bundled();
    let opts = GenOptions { lib_version: c.lib_version.clone(), ..GenOptions::default() };
    let mut report: BTreeMap<&str, Vec<String>> = BTreeMap::new();

    for def in &c.macros {
        let (values, extra) = form_defaults(&c, def);
        let (values, needed_input) = with_test_input(&c, values);
        let filled = match c.fill(def, &values, &extra) {
            Ok(f) => f,
            Err(e) => {
                report.entry("fill error").or_default().push(format!("{}: {e}", def.name));
                continue;
            }
        };
        if needed_input {
            report.entry("needs input (filled with test values)").or_default().push(def.name.clone());
        }

        let mut l = Layout::new("Sweep", Protocol::Dcc, Platform::Am328);
        let in_cnt = filled.in_cnt;
        l.sheets[0].rows.push(Row {
            enabled: true,
            address: if in_cnt > 0 { "10".into() } else { String::new() },
            input_type: match in_cnt {
                0 => None,
                1 => Some(InputType::OnOff),
                _ => Some(InputType::Red),
            },
            macro_text: filled.macro_text.clone(),
            leds: filled.leds.clone(),
            in_cnt,
            loc_in_ch: filled.loc_in_ch,
            led_channel: Some(def.default_channel.parse().unwrap_or(0)),
            ..Row::default()
        });

        let numbers = leds::compute(&l.sheets[0]);
        if filled.macro_text.contains("#LED") && numbers.rows[1].is_none() {
            report
                .entry("uses an LED but has no LED number")
                .or_default()
                .push(format!("{} (LEDs '{}')", def.name, filled.leds));
        }
        match generate(&l, 0, &c, &opts) {
            Ok(g) => {
                for w in g.warnings {
                    report.entry("generates with a warning").or_default().push(format!("{}: {}", def.name, w.message));
                }
                if g.header.contains("#LED") || g.header.contains("#InCh") {
                    report.entry("placeholder left in header").or_default().push(def.name.clone());
                }
            }
            Err(e) => {
                for d in e.errors {
                    report.entry("header error").or_default().push(format!("{}: {}", def.name, d.message));
                }
            }
        }
    }

    for (kind, items) in &report {
        println!("\n== {kind} ({})", items.len());
        for i in items {
            println!("   {i}");
        }
    }
    println!("\n{} effects checked", c.macros.len());

    // Known: Logic needs its own dialog; Herz_2MoRelais_I names its channel parameter "K232" in the workbook
    let known_fill_errors = ["Logic:", "Herz_2MoRelais_I:"];
    let unexpected: Vec<&String> = report
        .iter()
        .filter(|(kind, _)| !kind.starts_with("needs input"))
        .flat_map(|(kind, items)| {
            items.iter().filter(move |i| {
                !(*kind == "fill error" && known_fill_errors.iter().any(|k| i.starts_with(k)))
                    && !(*kind == "header error" && i.contains("not supported by MobaLedLib Studio yet"))
            })
        })
        .collect();
    assert!(unexpected.is_empty(), "unexpected problems: {unexpected:#?}");
}
