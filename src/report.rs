//! Pure accounting logic: account kinds, fold-to-depth, month helpers.
//! No SQL, no TUI — everything here is unit-testable in isolation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

/// Group order used by the Accounts and Reports tabs.
pub const KIND_ORDER: [Kind; 5] = [
    Kind::Asset,
    Kind::Liability,
    Kind::Equity,
    Kind::Income,
    Kind::Expense,
];

/// "ASSETS", "LIABILITIES", ...
pub fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Asset => "ASSETS",
        Kind::Liability => "LIABILITIES",
        Kind::Equity => "EQUITY",
        Kind::Income => "INCOME",
        Kind::Expense => "EXPENSES",
    }
}

/// Root segment of the account name, case-insensitively.
pub fn kind_of(name: &str) -> Option<Kind> {
    let root = name.split(':').next()?.trim().to_ascii_lowercase();
    match root.as_str() {
        "assets" | "asset" => Some(Kind::Asset),
        "liabilities" | "liability" => Some(Kind::Liability),
        "equity" => Some(Kind::Equity),
        "income" | "revenue" | "revenues" => Some(Kind::Income),
        "expenses" | "expense" => Some(Kind::Expense),
        _ => None,
    }
}

/// Displayed value from a raw signed balance (debit positive).
pub fn display_amount(kind: Kind, raw: i64) -> i64 {
    match kind {
        Kind::Asset | Kind::Expense => raw,
        _ => -raw,
    }
}

/// Merge `(account, amount)` rows into a tree at most `depth` segments deep,
/// sorted by name.
pub fn fold_depth(rows: &[(String, i64)], depth: usize) -> Vec<(String, i64)> {
    use std::collections::BTreeMap;

    let mut folded: BTreeMap<String, i64> = BTreeMap::new();
    for (name, amount) in rows {
        let key = account_at_depth(name, depth);
        *folded.entry(key).or_insert(0) += amount;
    }
    folded.into_iter().collect()
}

fn account_at_depth(name: &str, depth: usize) -> String {
    if depth == 0 {
        return String::new();
    }
    name.split(':').take(depth).collect::<Vec<_>>().join(":")
}

/// Shorten a set of account names to their distinguishing suffixes, joined
/// with ", ". A single name keeps only its last segment.
pub fn abbrev(names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let split: Vec<Vec<&str>> = names.iter().map(|n| n.split(':').collect()).collect();
    let common = split
        .iter()
        .map(|segments| segments.len())
        .min()
        .unwrap_or(0)
        .saturating_sub(1);

    let mut prefix_len = 0;
    while prefix_len < common {
        let first = split[0][prefix_len];
        if split.iter().all(|segments| segments[prefix_len] == first) {
            prefix_len += 1;
        } else {
            break;
        }
    }

    let parts: Vec<String> = split
        .iter()
        .map(|segments| segments[prefix_len..].join(":"))
        .collect();
    parts.join(", ")
}

const MONTH_LABELS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Shift a `(year, month)` pair by `delta` months, crossing years correctly.
pub fn ym_shift(ym: (i32, u32), delta: i32) -> (i32, u32) {
    let zero_based = ym.0 * 12 + (ym.1 as i32 - 1) + delta;
    let year = zero_based.div_euclid(12);
    let month = zero_based.rem_euclid(12) as u32 + 1;
    (year, month)
}

/// `"2026-11"`.
pub fn ym_fmt(ym: (i32, u32)) -> String {
    format!("{:04}-{:02}", ym.0, ym.1)
}

/// `"2026-11"` -> `(2026, 11)`. Unparsable input yields `(0, 1)` so a bad row
/// never sorts or compares above real months.
pub fn ym_parse(s: &str) -> (i32, u32) {
    let mut parts = s.split('-');
    let year = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let month = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    (year, month)
}

/// `"Nov"`.
pub fn ym_label(ym: (i32, u32)) -> String {
    let idx = (ym.1.clamp(1, 12) - 1) as usize;
    MONTH_LABELS[idx].to_string()
}

/// Half-open date bounds: `("2026-11-01", "2026-12-01")`.
pub fn ym_bounds(ym: (i32, u32)) -> (String, String) {
    let start = format!("{}-01", ym_fmt(ym));
    let end = format!("{}-01", ym_fmt(ym_shift(ym, 1)));
    (start, end)
}

/// Running totals over already ascending deltas.
pub fn running_totals(deltas: &[i64]) -> Vec<i64> {
    let mut total = 0i64;
    deltas
        .iter()
        .map(|d| {
            total += d;
            total
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_account_roots_to_kinds() {
        assert_eq!(kind_of("Assets:Bank"), Some(Kind::Asset));
        assert_eq!(kind_of("assets"), Some(Kind::Asset));
        assert_eq!(kind_of("Liabilities:Card"), Some(Kind::Liability));
        assert_eq!(kind_of("Equity"), Some(Kind::Equity));
        assert_eq!(kind_of("Expenses:Food"), Some(Kind::Expense));
        assert_eq!(kind_of("Revenue"), Some(Kind::Income));
        assert_eq!(kind_of("revenues:Sales"), Some(Kind::Income));
        assert_eq!(kind_of("Foo:Bar"), None);
        assert_eq!(kind_of(""), None);
    }

    #[test]
    fn folds_to_requested_depth() {
        let rows = vec![
            ("Assets:Bank:BCA".to_string(), 100),
            ("Assets:Bank:Mandiri".to_string(), 250),
            ("Assets:Cash".to_string(), 50),
        ];
        assert_eq!(
            fold_depth(&rows, 1),
            vec![("Assets".to_string(), 400)]
        );
        assert_eq!(
            fold_depth(&rows, 2),
            vec![
                ("Assets:Bank".to_string(), 350),
                ("Assets:Cash".to_string(), 50),
            ]
        );
        assert_eq!(
            fold_depth(&rows, 3),
            vec![
                ("Assets:Bank:BCA".to_string(), 100),
                ("Assets:Bank:Mandiri".to_string(), 250),
                ("Assets:Cash".to_string(), 50),
            ]
        );
    }

    #[test]
    fn root_only_rows_are_counted_as_themselves() {
        let rows = vec![
            ("Assets".to_string(), 10),
            ("Assets:Cash".to_string(), 5),
        ];
        assert_eq!(
            fold_depth(&rows, 2),
            vec![("Assets".to_string(), 10), ("Assets:Cash".to_string(), 5)]
        );
    }

    #[test]
    fn display_sign_follows_normal_balance() {
        assert_eq!(display_amount(Kind::Asset, 100), 100);
        assert_eq!(display_amount(Kind::Expense, 250), 250);
        assert_eq!(display_amount(Kind::Liability, -500), 500);
        assert_eq!(display_amount(Kind::Equity, -500), 500);
        assert_eq!(display_amount(Kind::Income, -1000), 1000);
    }

    #[test]
    fn shortens_names_to_distinguishing_suffix() {
        assert_eq!(
            abbrev(&[
                "Assets:Bank:BCA".to_string(),
                "Assets:Bank:Mandiri".to_string()
            ]),
            "BCA, Mandiri"
        );
        assert_eq!(abbrev(&["Assets:Cash".to_string()]), "Cash");
        assert_eq!(abbrev(&[]), "");
    }

    #[test]
    fn shifts_months_across_years() {
        assert_eq!(ym_shift((2026, 1), -1), (2025, 12));
        assert_eq!(ym_shift((2026, 12), 1), (2027, 1));
        assert_eq!(ym_shift((2026, 6), 0), (2026, 6));
        assert_eq!(ym_shift((2026, 1), -13), (2024, 12));
    }

    #[test]
    fn formats_and_parses_year_months() {
        assert_eq!(ym_fmt((2026, 11)), "2026-11");
        assert_eq!(ym_parse("2026-11"), (2026, 11));
        assert_eq!(ym_label((2026, 11)), "Nov");
        assert_eq!(ym_bounds((2026, 12)), ("2026-12-01".to_string(), "2027-01-01".to_string()));
        assert_eq!(ym_bounds((2026, 11)), ("2026-11-01".to_string(), "2026-12-01".to_string()));
    }

    #[test]
    fn accumulates_running_totals() {
        assert_eq!(running_totals(&[100, -250, 50]), vec![100, -150, -100]);
        assert_eq!(running_totals(&[]), Vec::<i64>::new());
    }
}
