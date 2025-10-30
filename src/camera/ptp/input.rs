use std::fmt::Display;

use strsim::damerau_levenshtein;
use strum::IntoEnumIterator;

pub trait CleanAlphanumeric {
    fn clean(&self) -> String;
}

impl<T: AsRef<str>> CleanAlphanumeric for T {
    fn clean(&self) -> String {
        self.as_ref()
            .trim()
            .to_lowercase()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect()
    }
}

const SIMILARITY_THRESHOLD: usize = 8;

pub trait Choices {
    fn choices() -> Vec<String>;

    fn closest(input: &str) -> Option<String> {
        let input_lower = input.to_lowercase();

        let mut best_score = usize::MAX;
        let mut best_match: Option<String> = None;

        for choice in Self::choices() {
            let dist = damerau_levenshtein(&input_lower, &choice.to_lowercase());
            if dist < best_score {
                best_score = dist;
                best_match = Some(choice.clone());
            }
        }

        if best_score <= SIMILARITY_THRESHOLD {
            best_match
        } else {
            None
        }
    }
}

impl<T> Choices for T
where
    T: IntoEnumIterator + Display,
{
    fn choices() -> Vec<String> {
        T::iter().map(|v| v.to_string()).collect()
    }
}
