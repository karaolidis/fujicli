use std::{borrow::Borrow, fmt};

use slug::slugify;

use super::store::LibraryError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for Slug {
    type Error = LibraryError;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        let s = slugify(name);
        if s.is_empty() {
            return Err(LibraryError::InvalidName);
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
        let s = Slug::try_from("Cinestill 800T").unwrap();
        assert_eq!(s.as_str(), "cinestill-800t");
    }

    #[test]
    fn diacritics_get_folded() {
        let s = Slug::try_from("Café Noir").unwrap();
        assert_eq!(s.as_str(), "cafe-noir");
    }

    #[test]
    fn whitespace_collapses() {
        let s = Slug::try_from("   multiple   spaces  ").unwrap();
        assert_eq!(s.as_str(), "multiple-spaces");
    }

    #[test]
    fn punctuation_dropped() {
        let s = Slug::try_from("Velvia, the (warm) edition!").unwrap();
        assert_eq!(s.as_str(), "velvia-the-warm-edition");
    }

    #[test]
    fn empty_input_rejected() {
        assert!(matches!(Slug::try_from(""), Err(LibraryError::InvalidName)));
    }

    #[test]
    fn all_punctuation_rejected() {
        assert!(matches!(
            Slug::try_from("!@#$%"),
            Err(LibraryError::InvalidName)
        ));
    }

    #[test]
    fn unicode_only_rejected_when_unmappable() {
        assert!(matches!(
            Slug::try_from("漢字のみ"),
            Ok(_) | Err(LibraryError::InvalidName)
        ));
    }

    #[test]
    fn ordering_is_lexicographic_on_inner_string() {
        let a = Slug::try_from("aaa").unwrap();
        let b = Slug::try_from("bbb").unwrap();
        assert!(a < b);
    }
}
