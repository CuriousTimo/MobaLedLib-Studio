//! The module library: MobaLedLib hardware modules (boards, LEDs, lamps) as data and SVGs,
//! for a view of the physical LED chain (`data/modules/`).
//!
//! Not from the Excel workbook: a deliberate exception. The facts come from wiki.mobaledlib.de
//! (each module lists its source). Nothing here changes the generated `LEDs_AutoProg.h`.

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
}
