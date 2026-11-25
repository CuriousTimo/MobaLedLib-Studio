//! The module library: MobaLedLib hardware modules (boards, LEDs, lamps) as data and SVGs,
//! for a view of the physical LED chain (`data/modules/`).
//!
//! Not from the Excel workbook: a deliberate exception. The board facts come from wiki.mobaledlib.de
//! (each module lists its source); the signal drawings from Hive-Rail, and which output lights which
//! lamp from running the workbook's signal macros in the simulator (issue #19). Nothing here changes
//! the generated `LEDs_AutoProg.h`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub about: String,
    pub categories: BTreeMap<String, Category>,
    #[serde(default)]
    pub connectors: BTreeMap<String, Connector>,
    pub modules: Vec<Module>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connector {
    pub name: String,
    pub pins: BTreeMap<String, String>,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub name: String,
    /// File name of the drawing in `data/modules/`.
    pub svg: String,
    pub category: String,
    /// LED numbers the module takes in the chain (None: not fixed or not known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leds: Option<u32>,
    /// Single outputs the module offers (3 per WS2811).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
    /// Outputs a light (LED, lamp) uses; it takes no LED number of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels_used: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,
    pub ports: Vec<Port>,
    /// For a signal: which output lights each lamp, per workbook signal macro.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drives: Vec<Drive>,
    #[serde(default)]
    pub facts: Vec<String>,
    /// What no source confirms yet.
    #[serde(default)]
    pub unconfirmed: Vec<String>,
    #[serde(default)]
    pub source: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub section: u32,
    pub leds: u32,
    #[serde(rename = "return")]
    pub return_port: String,
}

/// How workbook signal macros light the lamps of a signal drawing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drive {
    /// Macro names of the catalog that light the lamps the same way.
    pub macros: Vec<String>,
    /// The aspect of each input state: state n is input InCh+n (for `Bin` macros the binary value n).
    #[serde(default)]
    pub aspects: Vec<String>,
    /// Lamp port -> output: `Cn` the n-th single output from the macro's LED (WS2811, from 1),
    /// `Ln` the n-th WS2812 after it (from 0). Two lamps may share an output.
    pub lamps: BTreeMap<String, String>,
    /// Outputs the macro lights that have no lamp in the drawing, e.g. "C3 (Zs 3V)".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_drawn: Vec<String>,
}

/// An output of a [`Drive`]: a single output (`Cn`, from 1) or an RGB LED (`Ln`, from 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Single(u32),
    Rgb(u32),
}

impl Output {
    pub fn parse(s: &str) -> Option<Output> {
        let n = |r: &str| r.parse::<u32>().ok();
        match s.split_at_checked(1)? {
            ("C", r) => n(r).filter(|&v| v >= 1).map(Output::Single),
            ("L", r) => n(r).map(Output::Rgb),
            _ => None,
        }
    }
}

/// A connection point; `id` is the `data-port` of its element in the SVG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub id: String,
    pub kind: String,
    /// "in", "out" or "out-return" (data goes out and comes back on the same connector).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
}

/// The drawings, bundled like the catalog: (file name, SVG text).
const SVGS: &[(&str, &str)] = &[
    ("mll101.svg", include_str!("../../../data/modules/mll101.svg")),
    ("mll201.svg", include_str!("../../../data/modules/mll201.svg")),
    ("mll295.svg", include_str!("../../../data/modules/mll295.svg")),
    ("mll503.svg", include_str!("../../../data/modules/mll503.svg")),
    ("mll504.svg", include_str!("../../../data/modules/mll504.svg")),
    ("mll521.svg", include_str!("../../../data/modules/mll521.svg")),
    ("ws2811.svg", include_str!("../../../data/modules/ws2811.svg")),
    ("ws2812.svg", include_str!("../../../data/modules/ws2812.svg")),
    ("led.svg", include_str!("../../../data/modules/led.svg")),
    ("lamp.svg", include_str!("../../../data/modules/lamp.svg")),
    ("lamp-rgb.svg", include_str!("../../../data/modules/lamp-rgb.svg")),
    ("signal-hv-entry.svg", include_str!("../../../data/modules/signal-hv-entry.svg")),
    ("signal-hv-departure.svg", include_str!("../../../data/modules/signal-hv-departure.svg")),
    ("signal-hv-departure-distant.svg", include_str!("../../../data/modules/signal-hv-departure-distant.svg")),
    ("signal-hv-entry-distant.svg", include_str!("../../../data/modules/signal-hv-entry-distant.svg")),
    ("signal-hv-block.svg", include_str!("../../../data/modules/signal-hv-block.svg")),
    ("signal-hv-block-distant.svg", include_str!("../../../data/modules/signal-hv-block-distant.svg")),
    ("signal-hv-distant.svg", include_str!("../../../data/modules/signal-hv-distant.svg")),
    ("signal-ks-distant.svg", include_str!("../../../data/modules/signal-ks-distant.svg")),
    ("signal-ks-main.svg", include_str!("../../../data/modules/signal-ks-main.svg")),
    ("signal-ks-multi.svg", include_str!("../../../data/modules/signal-ks-multi.svg")),
    ("signal-sh-light.svg", include_str!("../../../data/modules/signal-sh-light.svg")),
];

impl Library {
    pub fn from_json(s: &str) -> Result<Self, crate::Error> {
        Ok(serde_json::from_str(s)?)
    }

    pub fn bundled() -> Self {
        Self::from_json(include_str!("../../../data/modules/modules.json")).expect("bundled module library is valid")
    }

    pub fn module(&self, id: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.id == id)
    }
}

/// The bundled SVG of a module's `svg` file name.
pub fn bundled_svg(file: &str) -> Option<&'static str> {
    SVGS.iter().find(|(f, _)| *f == file).map(|(_, s)| *s)
}

/// File names of all bundled drawings.
pub fn bundled_svg_files() -> impl Iterator<Item = &'static str> {
    SVGS.iter().map(|(f, _)| *f)
}

/// The `data-port` ids in an SVG, in drawing order.
pub fn svg_ports(svg: &str) -> Vec<String> {
    const KEY: &str = "data-port=\"";
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find(KEY) {
        rest = &rest[i + KEY.len()..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
            rest = &rest[end..];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_ports_in_order() {
        let svg = r#"<g class="port" data-port="in"><rect/></g><g data-port="out1"/>"#;
        assert_eq!(svg_ports(svg), ["in", "out1"]);
        assert!(svg_ports("<svg/>").is_empty());
    }

    #[test]
    fn outputs_parse() {
        assert_eq!(Output::parse("C1"), Some(Output::Single(1)));
        assert_eq!(Output::parse("L0"), Some(Output::Rgb(0)));
        assert_eq!(Output::parse("C0"), None);
        assert_eq!(Output::parse("X1"), None);
        assert_eq!(Output::parse(""), None);
    }
}
