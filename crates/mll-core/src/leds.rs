//! Automatic LED numbering. Port of `Update_Start_LedNr()` from `M20_PageEvents_a_Functions.bas`.
//!
//! Every active row gets a start LED number inside its LED channel. Rows that address
//! single channels of a WS2811 (`C1-2`, `C3-3`, ...) share one LED until a channel is reused.

use crate::model::Sheet;
use crate::vba::{cell_lines_sum, val_i};

/// Number of LED channels (8 LED pins, 1 DMX, 1 virtual) like `LED_CHANNELS` in Excel.
pub const LED_CHANNELS: usize = 10;

/// Marker in the LEDs column for serial sound channels.
pub const SERIAL_CHANNEL_PREFIX: &str = "S";

#[derive(Debug, Clone, PartialEq)]
pub struct LedNumbers {
    /// Start LED number of each row inside its channel, None for inactive rows.
    pub rows: Vec<Option<i64>>,
    /// Number of LEDs used in each channel (the hidden "SH_VARS_ROW" cells in Excel).
    pub per_channel: [i64; LED_CHANNELS],
}

impl LedNumbers {
    pub fn total(&self) -> i64 {
        self.per_channel.iter().sum()
    }
}

fn used_modules(channels: i64) -> i64 {
    (channels + 2) / 3
}

/// `Get_Parameter_from_Leds_Line`: "C1-2" -> (1, 2). Lines may start with "^".
pub fn single_channel_range(line: &str) -> Option<(i64, i64)> {
    let mut l = line.trim();
    if let Some(rest) = l.strip_prefix('^') {
        l = rest.trim();
    }
    let rest = l.strip_prefix('C')?;
    let mut it = rest.split('-');
    let a = it.next()?.trim().parse().ok()?;
    let b = it.next()?.trim().parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b))
}

pub fn is_single_channel(leds: &str) -> bool {
    leds.contains('C')
}

fn first_used_channel(leds: &str) -> i64 {
    leds.split('\n').filter_map(|l| single_channel_range(l).map(|r| r.0)).min().unwrap_or(0)
}

fn last_used_channel(leds: &str, last: &mut i64) {
    for l in leds.split('\n') {
        if let Some((_, b)) = single_channel_range(l) {
            if b > *last {
                *last = b;
            }
        }
    }
}

fn set_led_nr_target(macro_text: &str) -> Option<i64> {
    macro_text.trim().strip_prefix("// Set_LEDNr(").map(val_i)
}

/// Compute the start LED numbers of a sheet.
pub fn compute(sheet: &Sheet) -> LedNumbers {
    let mut led_nr = [0i64; LED_CHANNELS];
    let mut last_used = [0i64; LED_CHANNELS];
    let mut max_nr = [0i64; LED_CHANNELS];
    let mut rows = Vec::with_capacity(sheet.rows.len());
    let mut ch = 0usize;

    for row in &sheet.rows {
        let leds = row.leds.trim();
        if row.enabled && !leds.is_empty() {
            ch = (row.led_channel.unwrap_or(0) as usize).min(LED_CHANNELS - 1);
            let single = is_single_channel(leds);

            // Check if the previous lines addressed single channels
            if last_used[ch] > 0 {
                if single {
                    if leds.starts_with('^') {
                        last_used[ch] = 0;
                    }
                    if first_used_channel(leds) <= last_used[ch] {
                        led_nr[ch] += used_modules(last_used[ch]);
                        last_used[ch] = 0;
                    }
                } else {
                    led_nr[ch] += used_modules(last_used[ch]);
                    last_used[ch] = 0;
                }
            }

            rows.push(Some(led_nr[ch]));

            if leds == SERIAL_CHANNEL_PREFIX {
                // Serial sound channel: doesn't use LEDs
            } else if single {
                last_used_channel(leds, &mut last_used[ch]);
                if last_used[ch] > 3 {
                    // Single LEDs which don't fit into one WS2811 module
                    led_nr[ch] += last_used[ch] / 3;
                    last_used[ch] %= 3;
                }
            } else {
                let sum = cell_lines_sum(leds);
                if sum < 0 && led_nr[ch] > max_nr[ch] {
                    max_nr[ch] = led_nr[ch];
                }
                led_nr[ch] += sum;
            }

            if let Some(n) = set_led_nr_target(&row.macro_text) {
                led_nr[ch] = n;
            }
        } else {
            rows.push(None);
        }
        if led_nr[ch] > max_nr[ch] {
            max_nr[ch] = led_nr[ch];
        }
    }

    let mut per_channel = [0i64; LED_CHANNELS];
    for c in 0..LED_CHANNELS {
        per_channel[c] = max_nr[c] + used_modules(last_used[c]);
    }
    LedNumbers { rows, per_channel }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Protocol, Row};

    fn sheet(leds: &[&str]) -> Sheet {
        Sheet {
            name: "t".into(),
            protocol: Protocol::Dcc,
            rows: leds
                .iter()
                .map(|l| Row { enabled: true, leds: l.to_string(), macro_text: "X(#LED)".into(), ..Row::default() })
                .collect(),
        }
    }

    #[test]
    fn rgb_leds_are_consecutive() {
        let n = compute(&sheet(&["1", "5", "4"]));
        assert_eq!(n.rows, vec![Some(0), Some(1), Some(6)]);
        assert_eq!(n.per_channel[0], 10);
    }

    #[test]
    fn single_channels_share_a_module() {
        // C1-1 and C2-3 fit into one WS2811, the next C1-1 starts a new one
        let n = compute(&sheet(&["1", "C1-1", "C2-3", "C1-1", "1"]));
        assert_eq!(n.rows, vec![Some(0), Some(1), Some(1), Some(2), Some(3)]);
        assert_eq!(n.per_channel[0], 4);
    }

    #[test]
    fn disabled_rows_get_no_number() {
        let mut s = sheet(&["1", "2", "1"]);
        s.rows[1].enabled = false;
        let n = compute(&s);
        assert_eq!(n.rows, vec![Some(0), None, Some(1)]);
    }

    #[test]
    fn channels_count_separately() {
        let mut s = sheet(&["2", "3", "1"]);
        s.rows[1].led_channel = Some(1);
        let n = compute(&s);
        assert_eq!(n.rows, vec![Some(0), Some(0), Some(2)]);
        assert_eq!(n.per_channel[0], 3);
        assert_eq!(n.per_channel[1], 3);
    }
}
