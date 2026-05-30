use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ReqId(u64);

impl fmt::Display for ReqId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req#{}", self.0)
    }
}

pub struct ReqIdGen {
    next: AtomicU64,
}

impl ReqIdGen {
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(0),
        }
    }

    pub fn next(&self) -> ReqId {
        ReqId(self.next.fetch_add(1, Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_monotonic() {
        let ids = ReqIdGen::new();
        let a = ids.next();
        let b = ids.next();
        let c = ids.next();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }
}
