#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer<T> {
    pub fetched: T,
    pub working: T,
}

impl<T: PartialEq> Buffer<Shadowed<T>> {
    pub fn dirty(&self) -> bool {
        self.fetched.canonical != self.working.canonical
    }
}

impl<T: Clone> From<T> for Buffer<T> {
    fn from(value: T) -> Self {
        Self {
            fetched: value.clone(),
            working: value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shadowed<T> {
    pub canonical: T,
    pub shadow: T,
}

impl<T: Clone> From<T> for Shadowed<T> {
    fn from(value: T) -> Self {
        Self {
            canonical: value.clone(),
            shadow: value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_ignores_shadow_only_changes() {
        let buffer = Buffer {
            fetched: Shadowed {
                canonical: 1,
                shadow: 1,
            },
            working: Shadowed {
                canonical: 1,
                shadow: 2,
            },
        };
        assert!(!buffer.dirty());
    }

    #[test]
    fn dirty_tracks_canonical_changes() {
        let buffer = Buffer {
            fetched: Shadowed {
                canonical: 1,
                shadow: 9,
            },
            working: Shadowed {
                canonical: 2,
                shadow: 9,
            },
        };
        assert!(buffer.dirty());
    }
}
