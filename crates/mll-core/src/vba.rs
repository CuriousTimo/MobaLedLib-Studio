//! Small emulations of VBA built-ins. The header generator depends on their exact
//! (sometimes surprising) behavior to produce the same output as the Excel tool.

/// VBA `Val()`: parses the leading number of a string, ignoring spaces. Returns 0 if none.
pub fn val(s: &str) -> f64 {
    let mut out = String::new();
    let mut seen_digit = false;
    let mut seen_dot = false;
    for c in s.chars() {
        match c {
            ' ' | '\t' | '\n' | '\r' => continue,
            '+' | '-' if out.is_empty() => out.push(c),
            '0'..='9' => {
                seen_digit = true;
                out.push(c);
            }
            '.' if !seen_dot => {
                seen_dot = true;
                out.push(c);
            }
            _ => break,
        }
    }
    if !seen_digit {
        return 0.0;
    }
    out.parse().unwrap_or(0.0)
}

/// `Val()` truncated to an integer, as used for Long variables.
pub fn val_i(s: &str) -> i64 {
    val(s).trunc() as i64
}

/// VBA `IsNumeric()` for the cases that occur in the sheets.
pub fn is_numeric(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.parse::<f64>().is_ok()
}

/// `AddSpaceToLen`: pad with spaces on the right to at least `n` characters.
pub fn pad(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len >= n {
        s.to_string()
    } else {
        let mut r = String::with_capacity(n);
        r.push_str(s);
        r.extend(std::iter::repeat_n(' ', n - len));
        r
    }
}

/// `AddSpaceToLenLeft`: pad with spaces on the left.
pub fn pad_left(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len >= n {
        s.to_string()
    } else {
        let mut r = " ".repeat(n - len);
        r.push_str(s);
        r
    }
}

/// `CellLinesSum`: sum of `Val()` of every line in a cell.
pub fn cell_lines_sum(s: &str) -> i64 {
    s.split('\n').map(val_i).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn val_behaves_like_vba() {
        assert_eq!(val("12"), 12.0);
        assert_eq!(val(" 1 2abc"), 12.0);
        assert_eq!(val("C1-2"), 0.0);
        assert_eq!(val("-3"), -3.0);
        assert_eq!(val(""), 0.0);
        assert_eq!(val("2.5x"), 2.5);
    }

    #[test]
    fn padding() {
        assert_eq!(pad("ab", 4), "ab  ");
        assert_eq!(pad("abcde", 4), "abcde");
        assert_eq!(pad_left("7", 3), "  7");
    }

    #[test]
    fn numeric() {
        assert!(is_numeric(" 12 "));
        assert!(!is_numeric("1-2"));
        assert!(!is_numeric(""));
    }
}
