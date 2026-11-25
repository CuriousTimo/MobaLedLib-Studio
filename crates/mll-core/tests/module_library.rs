//! Keeps the module library whole: data/modules/modules.json and the SVGs must agree.

use std::collections::BTreeSet;

use mll_core::modules::{bundled_svg, bundled_svg_files, svg_ports, Library, Output};
use mll_core::Catalog;

#[test]
fn library_loads() {
    let lib = Library::bundled();
    assert!(lib.module("mll201").is_some());
    assert_eq!(lib.module("ws2811").and_then(|m| m.channels), Some(3));
}

#[test]
fn ids_are_unique() {
    let lib = Library::bundled();
    let mut seen = BTreeSet::new();
    for m in &lib.modules {
        assert!(seen.insert(&m.id), "module {} twice", m.id);
        let mut ports = BTreeSet::new();
        for p in &m.ports {
            assert!(ports.insert(&p.id), "{}: port {} twice", m.id, p.id);
        }
    }
}

#[test]
fn every_module_has_a_known_category_and_a_drawing() {
    let lib = Library::bundled();
    for m in &lib.modules {
        assert!(lib.categories.contains_key(&m.category), "{}: unknown category {}", m.id, m.category);
        assert!(bundled_svg(&m.svg).is_some(), "{}: {} is not bundled", m.id, m.svg);
    }
}

#[test]
fn every_drawing_belongs_to_a_module() {
    let lib = Library::bundled();
    for f in bundled_svg_files() {
        assert!(lib.modules.iter().any(|m| m.svg == f), "{f} belongs to no module");
    }
}

#[test]
fn ports_match_the_drawing() {
    let lib = Library::bundled();
    for m in &lib.modules {
        let drawn: BTreeSet<String> = svg_ports(bundled_svg(&m.svg).unwrap()).into_iter().collect();
        let listed: BTreeSet<String> = m.ports.iter().map(|p| p.id.clone()).collect();
        assert_eq!(drawn, listed, "{}: ports in {} differ from modules.json", m.id, m.svg);
    }
}

#[test]
fn connector_kinds_and_section_returns_exist() {
    let lib = Library::bundled();
    for m in &lib.modules {
        for p in &m.ports {
            if p.kind.starts_with("wanne") {
                assert!(lib.connectors.contains_key(&p.kind), "{}: connector {} unknown", m.id, p.kind);
            }
        }
        for s in &m.sections {
            assert!(
                m.ports.iter().any(|p| p.id == s.return_port),
                "{}: section {} return {} is no port",
                m.id,
                s.section,
                s.return_port
            );
        }
        if !m.sections.is_empty() {
            let sum: u32 = m.sections.iter().map(|s| s.leds).sum();
            assert_eq!(Some(sum), m.leds, "{}: sections don't add up to its LEDs", m.id);
        }
    }
}

/// The outputs a catalog macro has: `C1-n` is n single outputs, a number that many RGB LEDs.
fn macro_outputs(leds: &str) -> Option<Output> {
    match leds.strip_prefix("C1-") {
        Some(n) => n.parse().ok().map(Output::Single),
        None => leds.parse().ok().map(Output::Rgb),
    }
}

#[test]
fn signal_drives_fit_the_catalog_and_the_drawing() {
    let lib = Library::bundled();
    let cat = Catalog::bundled();
    let mut seen = BTreeSet::new();
    for m in &lib.modules {
        let ports: BTreeSet<&str> = m.ports.iter().map(|p| p.id.as_str()).collect();
        for d in &m.drives {
            assert!(!d.lamps.is_empty(), "{}: a drive without lamps", m.id);
            for name in &d.macros {
                assert!(seen.insert(name), "{}: macro {name} is mapped twice", m.id);
                let def = cat.macros.iter().find(|x| &x.name == name);
                let def = def.unwrap_or_else(|| panic!("{}: {name} is not in the catalog", m.id));
                let have = macro_outputs(&def.leds).unwrap_or_else(|| panic!("{name}: LEDs {:?}", def.leds));
                let in_cnt: u32 = def.in_cnt.parse().unwrap_or_else(|_| panic!("{name}: InCnt {:?}", def.in_cnt));
                let states = if name.contains("Bin") { 1 << in_cnt } else { in_cnt };
                let n = d.aspects.len() as u32;
                assert!(n > 0 && n <= states, "{}: {name} has {states} input states, {n} aspects", m.id);
                if !name.contains("Bin") {
                    assert_eq!(n, states, "{}: {name}: one aspect per input", m.id);
                }
                for (lamp, out) in &d.lamps {
                    assert!(ports.contains(lamp.as_str()), "{}: lamp {lamp} is no port", m.id);
                    let o = Output::parse(out).unwrap_or_else(|| panic!("{}: output {out}", m.id));
                    let fits = match (o, have) {
                        (Output::Single(n), Output::Single(max)) => n <= max,
                        (Output::Rgb(n), Output::Rgb(count)) => n < count,
                        _ => false,
                    };
                    assert!(fits, "{}: {name} has no output {out} (LEDs {})", m.id, def.leds);
                }
            }
        }
    }
}
