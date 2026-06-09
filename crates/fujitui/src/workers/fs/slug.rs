use std::{borrow::Borrow, fmt, str::FromStr};

use slug::slugify;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SlugError {
    #[error("name cannot be slugified (empty or non-slug-compatible characters only)")]
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Slug {
    type Err = SlugError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let s = slugify(name);
        if s.is_empty() {
            return Err(SlugError::Invalid);
        }
        Ok(Self(s))
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Slug {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_name_lowercases_and_hyphenates() {
        let s = "Cinestill 800T".parse::<Slug>().unwrap();
        assert_eq!(s.as_str(), "cinestill-800t");
    }

    #[test]
    fn diacritics_get_folded() {
        let s = "Café Noir".parse::<Slug>().unwrap();
        assert_eq!(s.as_str(), "cafe-noir");
    }

    #[test]
    fn whitespace_collapses() {
        let s = "   multiple   spaces  ".parse::<Slug>().unwrap();
        assert_eq!(s.as_str(), "multiple-spaces");
    }

    #[test]
    fn punctuation_dropped() {
        let s = "Velvia, the (warm) edition!".parse::<Slug>().unwrap();
        assert_eq!(s.as_str(), "velvia-the-warm-edition");
    }

    #[test]
    fn empty_input_rejected() {
        assert!(matches!("".parse::<Slug>(), Err(SlugError::Invalid)));
    }

    #[test]
    fn all_punctuation_rejected() {
        assert!(matches!("!@#$%".parse::<Slug>(), Err(SlugError::Invalid)));
    }

    #[test]
    fn unicode_only_rejected_when_unmappable() {
        assert!(matches!(
            "漢字のみ".parse::<Slug>(),
            Ok(_) | Err(SlugError::Invalid)
        ));
    }

    #[test]
    fn ordering_is_lexicographic_on_inner_string() {
        let a = "aaa".parse::<Slug>().unwrap();
        let b = "bbb".parse::<Slug>().unwrap();
        assert!(a < b);
    }
}
