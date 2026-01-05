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
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}
