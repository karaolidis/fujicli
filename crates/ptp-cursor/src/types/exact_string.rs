use derive_more::{
    AsMut, AsRef, Debug, Deref, DerefMut, Display, From, FromStr, Index, IndexMut, Into,
};

#[derive(
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    AsMut,
    AsRef,
    Debug,
    Deref,
    DerefMut,
    Display,
    From,
    FromStr,
    Index,
    IndexMut,
    Into,
)]
pub struct ExactString(pub String);

impl ExactString {
    #[must_use]
    pub const fn new(s: String) -> Self {
        Self(s)
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}
