use proc_macro2::{Ident, Span};

#[macro_export]
macro_rules! upper_camel_case_ident {
    ($($arg:tt)*) => {{
        let raw = ::std::format!($($arg)*).replace('-', "_");
        let cased = <str as ::heck::ToUpperCamelCase>::to_upper_camel_case(&raw);
        let safe = if cased.chars().next().is_none_or(|c| c.is_ascii_digit()) {
            ::std::format!("X{cased}")
        } else {
            cased
        };
        ::proc_macro2::Ident::new(&safe, ::proc_macro2::Span::call_site())
    }};
}

#[macro_export]
macro_rules! uppercase_ident {
    ($($arg:tt)*) => {{
        let cased = ::std::format!($($arg)*).replace('-', "_").to_uppercase();
        let safe = if cased.chars().next().is_none_or(|c| c.is_ascii_digit()) {
            ::std::format!("X{cased}")
        } else {
            cased
        };
        ::proc_macro2::Ident::new(&safe, ::proc_macro2::Span::call_site())
    }};
}

#[macro_export]
macro_rules! snake_case_ident {
    ($($arg:tt)*) => {{
        let raw = ::std::format!($($arg)*).replace('-', "_");
        let cased = <str as ::heck::ToSnakeCase>::to_snake_case(&raw);
        let safe = if cased.chars().next().is_none_or(|c| c.is_ascii_digit()) {
            ::std::format!("x_{cased}")
        } else {
            cased
        };
        ::proc_macro2::Ident::new(&safe, ::proc_macro2::Span::call_site())
    }};
}

/// Convert a numeric lookup key (e.g. `"-4"`, `"0"`, `"3.0"`, `"-0.3"`)
/// into a Rust variant identifier.
pub fn numeric_variant_ident(key: &str) -> Ident {
    let s = key.trim();
    if matches!(s, "0" | "-0" | "0.0" | "-0.0") {
        return Ident::new("Zero", Span::call_site());
    }

    let (sign, abs) = s
        .strip_prefix('-')
        .map_or(("Plus", s), |rest| ("Minus", rest));

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
    fn upper_camel_case_prepends_x_for_digit_start() {
        // Rust idents can't start with a digit, so e.g. an enum variant for
        // image size `7728x5152` becomes `X7728x5152`.
        assert_eq!(
            upper_camel_case_ident!("7728x5152").to_string(),
            "X7728x5152"
        );
    }

    #[test]
    fn upper_camel_case_normal_case() {
        assert_eq!(
            upper_camel_case_ident!("film_simulation").to_string(),
            "FilmSimulation",
        );
    }

    #[test]
    fn upper_camel_case_composes_suffix() {
        assert_eq!(
            upper_camel_case_ident!("{}_simulation", "x_t5").to_string(),
            "XT5Simulation",
        );
    }

    #[test]
    fn uppercase_composes_prefix_and_id() {
        assert_eq!(
            uppercase_ident!("C_{}_SIMULATION", "x_t5").to_string(),
            "C_X_T5_SIMULATION",
        );
        assert_eq!(
            uppercase_ident!("SIMULATION_OPT_{}", "film_simulation").to_string(),
            "SIMULATION_OPT_FILM_SIMULATION",
        );
    }

    #[test]
    fn snake_case_normalises_camel_input() {
        assert_eq!(
            snake_case_ident!("FilmSimulation").to_string(),
            "film_simulation",
        );
    }

    #[test]
    fn snake_case_passes_through_snake_input() {
        assert_eq!(
            snake_case_ident!("film_simulation").to_string(),
            "film_simulation",
        );
    }

    #[test]
    fn snake_case_prepends_x_for_digit_start() {
        assert_eq!(snake_case_ident!("7_eleven").to_string(), "x_7_eleven");
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
