use heck::ToUpperCamelCase;
use proc_macro2::{Ident, Span};

fn safe_ident(raw: String) -> Ident {
    let raw = raw.replace("-", "_");
    let safe = if raw.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        format!("X{raw}")
    } else {
        raw
    };

    Ident::new(&safe, Span::call_site())
}

pub fn safe_upper_camel_case_ident(s: &str) -> Ident {
    let raw = s.to_upper_camel_case();
    safe_ident(raw)
}

pub fn safe_uppercase_ident(s: &str) -> Ident {
    let raw = s.to_uppercase();
    safe_ident(raw)
}

/// Convert a numeric lookup key (e.g. `"-4"`, `"0"`, `"3.0"`, `"-0.3"`)
/// into a Rust variant identifier.
pub fn numeric_variant_ident(key: &str) -> Ident {
    let s = key.trim();
    if matches!(s, "0" | "-0" | "0.0" | "-0.0") {
        return Ident::new("Zero", Span::call_site());
    }

    let (sign, abs) = match s.strip_prefix('-') {
        Some(rest) => ("Minus", rest),
        None => ("Plus", s),
    };

    let mut digits: String = abs
        .chars()
        .map(|c| if c == '.' { '_' } else { c })
        .collect();

    while digits.ends_with("_0") {
        digits.truncate(digits.len() - 2);
    }

    Ident::new(&format!("{sign}{digits}"), Span::call_site())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_ident_prepends_x_for_digit_start() {
        // Rust idents can't start with a digit, so e.g. an enum variant for
        // image size `7728x5152` becomes `X7728x5152`.
        assert_eq!(
            safe_upper_camel_case_ident("7728x5152").to_string(),
            "X7728x5152"
        );
    }

    #[test]
    fn variant_ident_normal_case() {
        assert_eq!(
            safe_upper_camel_case_ident("film_simulation").to_string(),
            "FilmSimulation",
        );
    }

    #[test]
    fn numeric_variant_zero_collapses() {
        assert_eq!(numeric_variant_ident("0").to_string(), "Zero");
        assert_eq!(numeric_variant_ident("-0").to_string(), "Zero");
        assert_eq!(numeric_variant_ident("0.0").to_string(), "Zero");
        assert_eq!(numeric_variant_ident("-0.0").to_string(), "Zero");
    }

    #[test]
    fn numeric_variant_positive_drops_trailing_zero_fraction() {
        assert_eq!(numeric_variant_ident("3").to_string(), "Plus3");
        assert_eq!(numeric_variant_ident("3.0").to_string(), "Plus3");
        assert_eq!(numeric_variant_ident("0.3").to_string(), "Plus0_3");
        assert_eq!(numeric_variant_ident("2.7").to_string(), "Plus2_7");
    }

    #[test]
    fn numeric_variant_negative() {
        assert_eq!(numeric_variant_ident("-4").to_string(), "Minus4");
        assert_eq!(numeric_variant_ident("-0.3").to_string(), "Minus0_3");
        assert_eq!(numeric_variant_ident("-3.0").to_string(), "Minus3");
    }
}
