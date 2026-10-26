//! The layout data model. This is what gets saved as JSON.
//!
//! Row fields deliberately mirror the columns of the Excel Program Generator so that
//! `.MLL_pgf` files round-trip and the generated header matches the Excel output.

use serde::{Deserialize, Serialize};

pub const LAYOUT_FORMAT: &str = "mobaledlib-studio-layout";
pub const LAYOUT_VERSION: u32 = 1;

/// How the layout receives commands. Matches the Excel `Page_ID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    #[serde(rename = "DCC")]
    Dcc,
    #[serde(rename = "Selectrix")]
    Selectrix,
    #[serde(rename = "CAN")]
    Can,
    #[serde(rename = "LNet")]
    LNet,
}

impl Protocol {
    pub fn page_id(self) -> &'static str {
        match self {
            Protocol::Dcc => "DCC",
            Protocol::Selectrix => "Selectrix",
            Protocol::Can => "CAN",
            Protocol::LNet => "LNet",
        }
    }

    pub fn from_page_id(s: &str) -> Option<Self> {
        match s.trim() {
            "DCC" => Some(Protocol::Dcc),
            "Selectrix" => Some(Protocol::Selectrix),
            "CAN" => Some(Protocol::Can),
            "LNet" => Some(Protocol::LNet),
            _ => None,
        }
    }
}

/// The "Typ" column: how an address is switched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputType {
    /// "AnAus": one address used as on/off switch
    OnOff,
    /// "Rot": red button
    Red,
    /// "Grün": green button
    Green,
    /// "Tast": push button
    Push,
}

impl InputType {
    /// The German text used by Excel (and written to `.MLL_pgf` exports).
    pub fn german(self) -> &'static str {
        match self {
            InputType::OnOff => "AnAus",
            InputType::Red => "Rot",
            InputType::Green => "Grün",
            InputType::Push => "Tast",
        }
    }

    /// Recognise the type text in any of the languages the Excel tool supports.
    pub fn from_text(s: &str) -> Option<Self> {
        const RED: &[&str] = &["Rot", "Red", "Rood", "Rouge", "Rosso", "Rojo", "Rød"];
        const GREEN: &[&str] = &["Grün", "Green", "Groen", "Vert", "Verde", "Grøn"];
        const ONOFF: &[&str] =
            &["AnAus", "OnOff", "Aan uit", "Allumé éteint", "Acceso spento", "Encendido apagado", "Tænd sluk"];
        const PUSH: &[&str] = &["Tast", "Push", "Duwen", "Druk", "Pousser", "Appuyer", "Spingere", "Empujar", "Skubbe"];
        let t = s.trim();
        let hit = |list: &[&str]| list.iter().any(|x| x.eq_ignore_ascii_case(t));
        if hit(RED) {
            Some(InputType::Red)
        } else if hit(GREEN) {
            Some(InputType::Green)
        } else if hit(ONOFF) {
            Some(InputType::OnOff)
        } else if hit(PUSH) {
            Some(InputType::Push)
        } else {
            None
        }
    }
}

/// Controller that drives the LEDs. Matches the Excel board type ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    #[serde(rename = "AM328")]
    Am328,
    #[serde(rename = "ESP32")]
    Esp32,
    #[serde(rename = "PICO")]
    Pico,
}

impl Platform {
    pub fn id(self) -> &'static str {
        match self {
            Platform::Am328 => "AM328",
            Platform::Esp32 => "ESP32",
            Platform::Pico => "PICO",
        }
    }

    pub fn default_fqbn(self) -> &'static str {
        match self {
            Platform::Am328 => "arduino:avr:nano",
            Platform::Esp32 => "esp32:esp32:esp32",
            Platform::Pico => "rp2040:rp2040:rpipico",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub platform: Platform,
    /// arduino-cli board id of the LED Arduino
    pub fqbn: String,
    /// Serial port of the LED Arduino ("" = ask / auto-detect)
    #[serde(default)]
    pub port: String,
}

impl Default for Board {
    fn default() -> Self {
        Board { platform: Platform::Am328, fqbn: Platform::Am328.default_fqbn().into(), port: String::new() }
    }
}

/// Global settings, the equivalent of the Excel "Config" sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Added to every DCC address (e.g. 4 for Roco)
    pub dcc_offset: i64,
    /// House(): minimal seconds until the next room changes (None = library default 50)
    pub house_min_t: Option<u32>,
    /// House(): maximal seconds until the next room changes (None = library default 150)
    pub house_max_t: Option<u32>,
    /// Lets the color test program send colors via RS232
    pub color_test_mode: bool,
    /// Use SPI between the DCC and the LED Arduino
    pub use_spi: bool,
    /// Simulate button release (0 = legacy, 1 = off, 2/3 = modes)
    pub gen_button_release_com: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dcc_offset: 0,
            house_min_t: None,
            house_max_t: None,
            color_test_mode: true,
            use_spi: false,
            gen_button_release_com: 0,
        }
    }
}

/// One line of a sheet: one effect.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Row {
    pub enabled: bool,
    /// Free text used for grouping/filtering ("Filter" column)
    pub filter: String,
    /// DCC/CAN/LNet address, Selectrix channel, or a variable name. May be a range like "10-12".
    pub address: String,
    /// Selectrix only: bit position 1..8
    pub bit_pos: String,
    pub input_type: Option<InputType>,
    /// Start value; "*" forces storing the state, "0" disables it
    pub start_value: String,
    pub description: String,
    pub distributor: String,
    pub plug: String,
    /// The macro call(s), e.g. `House(#LED, #InCh, 2, 5, ROOM_DARK)`. Several lines are allowed.
    pub macro_text: String,
    /// LEDs column: a number, `C1-2` style single channels, `^ C1`, or several lines
    pub leds: String,
    pub in_cnt: u32,
    pub loc_in_ch: u32,
    /// LED channel (output pin group). None = same as the previous row.
    pub led_channel: Option<u32>,
    /// Unknown trailing `.MLL_pgf` columns, kept so exports round-trip
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<String>,
}

impl Row {
    /// Name of the first macro in the macro column, e.g. "House".
    pub fn macro_name(&self) -> &str {
        let first = self.macro_text.lines().next().unwrap_or("").trim();
        first.split('(').next().unwrap_or("").trim()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub board: Board,
    #[serde(default)]
    pub settings: Settings,
    pub sheets: Vec<Sheet>,
}

impl Layout {
    /// A fresh layout with one sheet that contains the heartbeat LED, like the Excel template.
    pub fn new(name: &str, protocol: Protocol, platform: Platform) -> Self {
        Layout {
            format: LAYOUT_FORMAT.into(),
            version: LAYOUT_VERSION,
            name: name.into(),
            board: Board { platform, fqbn: platform.default_fqbn().into(), port: String::new() },
            settings: Settings::default(),
            sheets: vec![Sheet {
                name: protocol.page_id().into(),
                protocol,
                rows: vec![Row {
                    enabled: true,
                    description: "LED auf dem Mainboard".into(),
                    macro_text: "RGB_Heartbeat(#LED)".into(),
                    leds: "1".into(),
                    led_channel: Some(0),
                    ..Row::default()
                }],
            }],
        }
    }

    pub fn from_json(s: &str) -> Result<Self, crate::Error> {
        let layout: Layout = serde_json::from_str(s)?;
        if layout.format != LAYOUT_FORMAT {
            return Err(crate::Error::Format(format!("not a MobaLedLib Studio layout (format = '{}')", layout.format)));
        }
        if layout.version > LAYOUT_VERSION {
            return Err(crate::Error::Format(format!(
                "layout was saved by a newer version (file version {}, supported {})",
                layout.version, LAYOUT_VERSION
            )));
        }
        Ok(layout)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("layout serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip() {
        let l = Layout::new("Test", Protocol::Dcc, Platform::Am328);
        let back = Layout::from_json(&l.to_json()).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn input_type_languages() {
        assert_eq!(InputType::from_text("Grün"), Some(InputType::Green));
        assert_eq!(InputType::from_text("OnOff"), Some(InputType::OnOff));
        assert_eq!(InputType::from_text("Pousser"), Some(InputType::Push));
        assert_eq!(InputType::from_text("xyz"), None);
    }
}
