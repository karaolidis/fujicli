#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer<T> {
    pub fetched: T,
    pub working: T,
}

impl<T: PartialEq> Buffer<T> {
    pub fn dirty(&self) -> bool {
        self.fetched != self.working
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
