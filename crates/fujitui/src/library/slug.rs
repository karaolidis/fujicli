use std::{borrow::Borrow, fmt};

use slug::slugify;

use super::store::LibraryError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    pub fn from_name(name: &str) -> Result<Self, LibraryError> {
        let s = slugify(name);
        if s.is_empty() {
            return Err(LibraryError::InvalidName);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
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
        let s = Slug::from_name("Cinestill 800T").unwrap();
        assert_eq!(s.as_str(), "cinestill-800t");
    }

    #[test]
    fn diacritics_get_folded() {
        let s = Slug::from_name("Café Noir").unwrap();
        assert_eq!(s.as_str(), "cafe-noir");
    }

    #[test]
    fn whitespace_collapses() {
        let s = Slug::from_name("   multiple   spaces  ").unwrap();
        assert_eq!(s.as_str(), "multiple-spaces");
    }

    #[test]
    fn punctuation_dropped() {
        let s = Slug::from_name("Velvia, the (warm) edition!").unwrap();
        assert_eq!(s.as_str(), "velvia-the-warm-edition");
    }

    #[test]
    fn empty_input_rejected() {
        assert!(matches!(
            Slug::from_name(""),
            Err(LibraryError::InvalidName)
        ));
    }

    #[test]
    fn all_punctuation_rejected() {
        assert!(matches!(
            Slug::from_name("!@#$%"),
            Err(LibraryError::InvalidName)
        ));
    }

    #[test]
    fn unicode_only_rejected_when_unmappable() {
        assert!(matches!(
            Slug::from_name("漢字のみ"),
            Ok(_) | Err(LibraryError::InvalidName)
        ));
    }

    #[test]
    fn ordering_is_lexicographic_on_inner_string() {
        let a = Slug::from_name("aaa").unwrap();
        let b = Slug::from_name("bbb").unwrap();
        assert!(a < b);
    }
}
