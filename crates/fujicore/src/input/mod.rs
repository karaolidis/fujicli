pub mod error;

pub use error::OptionError;

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use clap::ValueEnum;

    use crate::generated::options::{CustomSetting, DynamicRange, GrainEffect, HighlightTone};

    fn parse<T: ValueEnum>(input: &str) -> Option<T> {
        T::from_str(input, true).ok()
    }

    #[test]
    fn enum_accepts_kebab_id_in_any_case() {
        assert_eq!(parse("weak-small"), Some(GrainEffect::WeakSmall));
        assert_eq!(parse("Weak-Small"), Some(GrainEffect::WeakSmall));
    }

    #[test]
    fn enum_rejects_unlisted_spellings() {
        assert_eq!(parse::<GrainEffect>("weaksmall"), None);
        assert_eq!(parse::<GrainEffect>("Weak Small"), None);
        assert_eq!(parse::<GrainEffect>("weak_small"), None);
    }

    #[test]
    fn enum_aliases_match_exactly() {
        assert_eq!(parse("800+"), Some(DynamicRange::Hdr800Plus));
        assert_eq!(parse("DR800+"), Some(DynamicRange::Hdr800Plus));
        assert_eq!(parse("800"), Some(DynamicRange::Hdr800));
        assert_eq!(parse("1"), Some(CustomSetting::C1));
        assert_eq!(parse("large-weak"), Some(GrainEffect::WeakLarge));
        assert_eq!(parse::<GrainEffect>("large_weak"), None);
    }

    #[test]
    fn enum_serializes_as_id() {
        let json = serde_json::to_string(&DynamicRange::Hdr800Plus).unwrap();
        assert_eq!(json, "\"hdr800_plus\"");
        let back: DynamicRange = serde_json::from_str(&json).unwrap();
        assert_eq!(back, DynamicRange::Hdr800Plus);
        assert!(serde_json::from_str::<DynamicRange>("\"HDR800+\"").is_err());
    }

    #[test]
    fn number_is_parsed_verbatim() {
        assert!(HighlightTone::from_str("1.5").is_ok());
        assert!(HighlightTone::from_str("-1").is_ok());
        assert!(HighlightTone::from_str("+1").is_ok());
        assert!(HighlightTone::from_str(" 1").is_err());
        assert!(HighlightTone::from_str("1x").is_err());
    }
}
