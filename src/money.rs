//! Money as signed `i64` minor units (2 decimals). Never `f64`.

/// `-1234567` -> `"-12,345.67"`.
pub fn format_minor(v: i64) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let abs = v.unsigned_abs();
    format!(
        "{}{}.{:02}",
        sign,
        group_thousands(abs / 100),
        abs % 100
    )
}

/// `1234567` -> `"1,234,567"`.
fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Strict amount parser: trust boundary for user input.
pub fn parse_amount(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty amount".into());
    }
    let (neg, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    if body.is_empty() {
        return Err(format!("invalid amount: {s}"));
    }
    let mut parts = body.split('.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next();
    if parts.next().is_some() {
        return Err(format!("invalid amount: {s}"));
    }

    // Integer part: digits with optional comma groups, groups exactly 3 after the first.
    let mut int_digits = String::new();
    if !int_part.is_empty() {
        let groups: Vec<&str> = int_part.split(',').collect();
        if !int_part.contains(',') {
            if !int_part.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("invalid amount: {s}"));
            }
            int_digits.push_str(int_part);
        } else {
            let (first, rest) = groups.split_first().expect("split yields at least one group");
            if first.is_empty() || first.len() > 3 || !first.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("invalid amount: {s}"));
            }
            int_digits.push_str(first);
            for group in rest {
                if group.len() != 3 || !group.chars().all(|c| c.is_ascii_digit()) {
                    return Err(format!("invalid amount: {s}"));
                }
                int_digits.push_str(group);
            }
        }
    }

    let frac_digits = match frac_part {
        None => "00".to_string(),
        Some(f) => {
            if f.is_empty() || f.len() > 2 || !f.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("invalid amount: {s}"));
            }
            format!("{f:0<2}")
        }
    };

    let mut value: i64 = 0;
    for ch in int_digits.chars() {
        let d = ch.to_digit(10).expect("validated digit") as i64;
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(d))
            .ok_or_else(|| format!("amount too large: {s}"))?;
    }
    value = value
        .checked_mul(100)
        .and_then(|v| v.checked_add(frac_digits.parse::<i64>().expect("2 digits")))
        .ok_or_else(|| format!("amount too large: {s}"))?;

    Ok(if neg { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_formatted_amounts() {
        assert_eq!(parse_amount("0"), Ok(0));
        assert_eq!(parse_amount("12"), Ok(1200));
        assert_eq!(parse_amount("12.3"), Ok(1230));
        assert_eq!(parse_amount("12.34"), Ok(1234));
        assert_eq!(parse_amount("-12.34"), Ok(-1234));
        assert_eq!(parse_amount("+12.34"), Ok(1234));
        assert_eq!(parse_amount("1,234.56"), Ok(123456));
        assert_eq!(parse_amount(" 1,234.56 "), Ok(123456));
        assert_eq!(parse_amount(".5"), Ok(50));
    }

    #[test]
    fn rejects_malformed_amounts() {
        assert!(parse_amount("1,5").is_err());
        assert!(parse_amount("1.234").is_err());
        assert!(parse_amount("").is_err());
        assert!(parse_amount("-").is_err());
        assert!(parse_amount(".").is_err());
        assert!(parse_amount("abc").is_err());
        assert!(parse_amount("1,2345.00").is_err());
        assert!(parse_amount("1.2.3").is_err());
        assert!(parse_amount("12a").is_err());
    }

    #[test]
    fn formats_minor_units() {
        assert_eq!(format_minor(0), "0.00");
        assert_eq!(format_minor(-5), "-0.05");
        assert_eq!(format_minor(1234567), "12,345.67");
        assert_eq!(format_minor(100000), "1,000.00");
        assert_eq!(format_minor(-1234567), "-12,345.67");
    }

    #[test]
    fn roundtrips_through_parse_and_format() {
        for v in [0i64, 1, -1, 99, -99, 100, 123456, -9876543] {
            assert_eq!(parse_amount(&format_minor(v)), Ok(v), "value {v}");
        }
    }
}
