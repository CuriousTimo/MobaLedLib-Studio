//! Tests against the real catalog extracted from the Excel workbook.

use std::collections::BTreeMap;

use mll_core::catalog::{fill_macro, Catalog};
use mll_core::codegen::{generate, GenOptions};
use mll_core::model::{InputType, Layout, Platform, Protocol, Row};

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn catalog_loads() {
    let c = Catalog::bundled();
    assert!(c.macros.len() > 200);
    assert!(c.params.len() > 100);
    assert_eq!(c.platform("AM328", "MaxLed"), Some("256"));
    assert!(c.find_macro("House(#LED, #InCh, 2, 5, ROOM_DARK)").is_some());
}

#[test]
fn every_macro_has_a_prototype_and_name() {
    for m in Catalog::bundled().macros {
        assert!(!m.name.is_empty(), "{m:?}");
        assert!(!m.prototype.is_empty(), "{m:?}");
    }
}

#[test]
fn fill_real_macros() {
    let c = Catalog::bundled();
    let house = c.macros.iter().find(|m| m.name == "House").unwrap();
    let f = fill_macro(house, &values(&[("On_Min", "2"), ("On_Limit", "5")]), &["ROOM_DARK".into(), "ROOM_TV0".into()])
        .unwrap();
    assert_eq!(f.macro_text, "House(#LED, #InCh, 2, 5, ROOM_DARK, ROOM_TV0)");
    assert_eq!((f.leds.as_str(), f.in_cnt), ("2", 1));

    let servo = c.macros.iter().find(|m| m.name == "Servo2").unwrap();
    let f = fill_macro(
        servo,
        &values(&[("B_LED_Cx", "C1"), ("Servo21", "40"), ("Servo22", "210"), ("ServoT", "10 Sek")]),
        &[],
    )
    .unwrap();
    assert_eq!(f.macro_text, "Servo2(#LED, #InCh, C1, 40, 210, 10 Sek)");
    assert_eq!(f.in_cnt, 2);
    // Negative SingleLEDCnt forces exactly one channel
    assert_eq!(f.leds, "C1-1");
}

#[test]
fn generate_a_small_layout() {
    let c = Catalog::bundled();
    let mut l = Layout::new("Test", Protocol::Dcc, Platform::Am328);
    l.sheets[0].rows.push(Row {
        enabled: true,
        address: "3".into(),
        input_type: Some(InputType::Red),
        description: "Schranke".into(),
        macro_text: "Servo2(#LED, #InCh, C1, 40, 210, 10 Sek)".into(),
        leds: "C1-1".into(),
        in_cnt: 2,
        ..Row::default()
    });
    let opts = GenOptions { lib_version: c.lib_version.clone(), ..GenOptions::default() };
    let g = generate(&l, 0, &c, &opts).unwrap();
    assert!(g.header.contains("  Servo2(1, 0, C1, 40, 210, 10 Sek)"));
    assert!(g.header.contains("{ 3    + B_RED,     2 }"));
    // Servo2 prevents storing its state (store type 4)
    assert!(!g.header.contains("IS_PULSE"));
}
