//! Import/export of the Excel Program Generator's `.MLL_pgf` files.
//!
//! Format (written by `M18_Save_Load.bas`):
//! ```text
//! Head:  <tab> Program_Generator configuration file <tab> V1.0|V2.0
//! Sheet: <tab> <Page_ID> <tab> <sheet name>
//! Line:  <tab> Act|- <tab> <one field per sheet column ...>
//! Board: <tab> <build options>          (V2.0 only, after the lines of a sheet)
//! ```
//! Newlines inside a cell are stored as `{NewLine}`. Excel writes the file in the Windows
//! ANSI code page (Windows-1252).

use crate::leds;
use crate::model::{InputType, Layout, Platform, Protocol, Row, Sheet, LAYOUT_FORMAT, LAYOUT_VERSION};
use crate::Error;

const IDENT: &str = "Program_Generator configuration file";
const NEWLINE_TOKEN: &str = "{NewLine}";

/// Decode the raw file. Excel writes Windows-1252; accept UTF-8 too.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

fn cell(s: &str) -> String {
    s.replace(NEWLINE_TOKEN, "\n")
}

fn fqbn_from_build_options(opts: &str) -> Option<String> {
    let mut it = opts.split_whitespace();
    while let Some(tok) = it.next() {
        if tok == "--board" {
            return it.next().map(|s| s.to_string());
        }
    }
    None
}

/// Parse a `.MLL_pgf` file into a layout.
pub fn import(bytes: &[u8], name: &str) -> Result<Layout, Error> {
    let text = decode(bytes);
    let mut sheets: Vec<Sheet> = Vec::new();
    let mut fqbn: Option<String> = None;
    let mut saw_head = false;

    for (nr, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        match parts[0] {
            "Head:" => {
                if parts.get(1).map(|s| s.trim()) != Some(IDENT) {
                    return Err(Error::Format("not a Program Generator configuration file".into()));
                }
                saw_head = true;
            }
            "Sheet:" => {
                let page_id = parts.get(1).copied().unwrap_or("");
                let protocol = Protocol::from_page_id(page_id)
                    .ok_or_else(|| Error::Format(format!("line {}: unknown sheet type '{}'", nr + 1, page_id)))?;
                let sheet_name = parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
                sheets.push(Sheet {
                    name: if sheet_name.is_empty() { page_id.to_string() } else { sheet_name },
                    protocol,
                    rows: Vec::new(),
                });
            }
            "Line:" => {
                if !saw_head {
                    return Err(Error::Format("missing 'Head:' line".into()));
                }
                if sheets.is_empty() {
                    sheets.push(Sheet { name: "DCC".into(), protocol: Protocol::Dcc, rows: Vec::new() });
                }
                let sheet = sheets.last_mut().unwrap();
                sheet.rows.push(parse_row(&parts, sheet.protocol));
            }
            "Board:" => {
                if let Some(f) = parts.get(1).and_then(|o| fqbn_from_build_options(o)) {
                    fqbn = Some(f);
                }
            }
            _ => {} // Ignore unknown lines like the Excel reader does
        }
    }
    if !saw_head {
        return Err(Error::Format("not a Program Generator configuration file".into()));
    }
    if sheets.is_empty() {
        return Err(Error::Format("the file contains no sheets".into()));
    }

    let mut layout = Layout {
        format: LAYOUT_FORMAT.into(),
        version: LAYOUT_VERSION,
        name: name.to_string(),
        board: Default::default(),
        settings: Default::default(),
        sheets,
    };
    if let Some(f) = fqbn {
        if f.starts_with("esp32:") {
            layout.board.platform = Platform::Esp32;
        } else if f.starts_with("rp2040:") || f.contains("pico") {
            layout.board.platform = Platform::Pico;
        }
        layout.board.fqbn = f;
    }
    Ok(layout)
}

fn parse_row(parts: &[&str], protocol: Protocol) -> Row {
    let get = |i: usize| parts.get(i).map(|s| cell(s)).unwrap_or_default();
    let mut row = Row { enabled: parts.get(1).map(|s| s.trim()) == Some("Act"), filter: get(2), ..Row::default() };
    // Selectrix sheets have the extra "Bitposition" column after the channel
    let o = if protocol == Protocol::Selectrix {
        row.bit_pos = get(4);
        1
    } else {
        0
    };
    row.address = get(3);
    let typ = get(4 + o);
    row.input_type = InputType::from_text(&typ);
    row.start_value = get(5 + o);
    row.description = get(6 + o);
    row.distributor = get(7 + o);
    row.plug = get(8 + o);
    row.macro_text = get(9 + o);
    // 10 + o: Start LedNr (computed, recalculated on load)
    row.leds = get(11 + o);
    row.in_cnt = crate::vba::val_i(&get(12 + o)).max(0) as u32;
    row.loc_in_ch = crate::vba::val_i(&get(13 + o)).max(0) as u32;
    let ch = get(14 + o);
    row.led_channel = if ch.trim().is_empty() { None } else { Some(crate::vba::val_i(&ch).max(0) as u32) };
    row.extra = parts.iter().skip(15 + o).map(|s| cell(s)).collect();
    while row.extra.last().is_some_and(|s| s.is_empty()) {
        row.extra.pop();
    }
    row
}

fn out(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', NEWLINE_TOKEN)
}

/// Write the layout as a `.MLL_pgf` file (Windows-1252, CRLF), readable by the Excel tool.
pub fn export(layout: &Layout) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&format!("Head:\t{IDENT}\tV2.0\r\n"));
    for sheet in &layout.sheets {
        let numbers = leds::compute(sheet);
        s.push_str(&format!("Sheet:\t{}\t{}\r\n", sheet.protocol.page_id(), out(&sheet.name)));
        for (i, row) in sheet.rows.iter().enumerate() {
            let mut f: Vec<String> = Vec::with_capacity(20);
            f.push("Line:".into());
            f.push(if row.enabled { "Act".into() } else { "-".into() });
            f.push(out(&row.filter));
            f.push(out(&row.address));
            if sheet.protocol == Protocol::Selectrix {
                f.push(out(&row.bit_pos));
            }
            f.push(row.input_type.map(|t| t.german().to_string()).unwrap_or_default());
            f.push(out(&row.start_value));
            f.push(out(&row.description));
            f.push(out(&row.distributor));
            f.push(out(&row.plug));
            f.push(out(&row.macro_text));
            f.push(numbers.rows[i].map(|n| n.to_string()).unwrap_or_default());
            f.push(out(&row.leds));
            f.push(row.in_cnt.to_string());
            f.push(row.loc_in_ch.to_string());
            f.push(row.led_channel.map(|c| c.to_string()).unwrap_or_default());
            f.extend(row.extra.iter().map(|e| out(e)));
            s.push_str(&f.join("\t"));
            s.push_str("\r\n");
        }
        s.push_str(&format!("Board:\tAutoDet --board {}\r\n", layout.board.fqbn));
    }
    encoding_rs::WINDOWS_1252.encode(&s).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Head:\tProgram_Generator configuration file\tV1.0\r\n\
Sheet:\tDCC\tMB Test 1 DCC\r\n\
Line:\tAct\tRGB_LED1\t\t\t\tZeigt an\t\t\tRGB_Heartbeat(#LED)\t0\t1\t0\t0\t\t\t\t\r\n\
Line:\t-\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\r\n\
Line:\tAct\tDCC_Arduino\t1\tAnAus\t\t*** DCC Test ***\t\t\tAndreaskrRGB(#LED, #InCh)\t4\t2\t1\t0\t0\t\t\t\r\n";

    #[test]
    fn imports_excel_sample() {
        let l = import(SAMPLE.as_bytes(), "x").unwrap();
        assert_eq!(l.sheets.len(), 1);
        let s = &l.sheets[0];
        assert_eq!(s.name, "MB Test 1 DCC");
        assert_eq!(s.rows.len(), 3);
        assert!(s.rows[0].enabled);
        assert_eq!(s.rows[0].macro_text, "RGB_Heartbeat(#LED)");
        assert_eq!(s.rows[0].leds, "1");
        assert!(!s.rows[1].enabled);
        let r = &s.rows[2];
        assert_eq!(r.address, "1");
        assert_eq!(r.input_type, Some(InputType::OnOff));
        assert_eq!(r.in_cnt, 1);
        assert_eq!(r.leds, "2");
        assert_eq!(r.led_channel, Some(0));
    }

    #[test]
    fn decodes_windows_1252() {
        let bytes = b"Head:\tProgram_Generator configuration file\tV1.0\r\nSheet:\tDCC\tGr\xfcn\r\n";
        let l = import(bytes, "x").unwrap();
        assert_eq!(l.sheets[0].name, "Grün");
    }

    #[test]
    fn round_trip() {
        let l = import(SAMPLE.as_bytes(), "x").unwrap();
        let again = import(&export(&l), "x").unwrap();
        assert_eq!(l.sheets, again.sheets);
    }

    #[test]
    fn new_lines_in_cells() {
        let src = "Head:\tProgram_Generator configuration file\tV1.0\nSheet:\tDCC\tA\nLine:\tAct\t\t\t\t\t\t\t\tLine1{NewLine}Line2\t0\t1\t0\t0\t0\n";
        let l = import(src.as_bytes(), "x").unwrap();
        assert_eq!(l.sheets[0].rows[0].macro_text, "Line1\nLine2");
        let text = decode(&export(&l));
        assert!(text.contains("Line1{NewLine}Line2"));
    }
}
