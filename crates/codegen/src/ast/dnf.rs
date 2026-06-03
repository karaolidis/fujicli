use std::{
    collections::BTreeSet,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    slice::Iter,
    vec::IntoIter,
};

use anyhow::bail;

use crate::{
    ast::{
        Assignment, AssignmentEffect, LeafBetween, LeafEquals, LeafGt, LeafGte, LeafIn, LeafLt,
        LeafLte, LeafPresent, PredAll, PredAny, PredNot, Predicate, Scope,
    },
    util::multiset,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Leaf {
    Present(LeafPresent),
    Equals(LeafEquals),
    NotEquals(LeafEquals),
    In(LeafIn),
    NotIn(LeafIn),
    Between(LeafBetween),
    NotBetween(LeafBetween),
    LessThan(LeafLt),
    NotLessThan(LeafLt),
    LessThanOrEqual(LeafLte),
    NotLessThanOrEqual(LeafLte),
    GreaterThan(LeafGt),
    NotGreaterThan(LeafGt),
    GreaterThanOrEqual(LeafGte),
    NotGreaterThanOrEqual(LeafGte),
}

impl Leaf {
    pub fn r#ref(&self) -> &str {
        match self {
            Self::Present(p) => &p.r#ref,
            Self::Equals(p) | Self::NotEquals(p) => &p.r#ref,
            Self::In(p) | Self::NotIn(p) => &p.r#ref,
            Self::Between(p) | Self::NotBetween(p) => &p.r#ref,
            Self::LessThan(p) | Self::NotLessThan(p) => &p.r#ref,
            Self::LessThanOrEqual(p) | Self::NotLessThanOrEqual(p) => &p.r#ref,
            Self::GreaterThan(p) | Self::NotGreaterThan(p) => &p.r#ref,
            Self::GreaterThanOrEqual(p) | Self::NotGreaterThanOrEqual(p) => &p.r#ref,
        }
    }

    pub fn scope(&self) -> Scope {
        match self {
            Self::Present(p) => p.scope,
            Self::Equals(p) | Self::NotEquals(p) => p.scope,
            Self::In(p) | Self::NotIn(p) => p.scope,
            Self::Between(p) | Self::NotBetween(p) => p.scope,
            Self::LessThan(p) | Self::NotLessThan(p) => p.scope,
            Self::LessThanOrEqual(p) | Self::NotLessThanOrEqual(p) => p.scope,
            Self::GreaterThan(p) | Self::NotGreaterThan(p) => p.scope,
            Self::GreaterThanOrEqual(p) | Self::NotGreaterThanOrEqual(p) => p.scope,
        }
    }

    pub fn negated(self) -> Self {
        match self {
            Self::Present(LeafPresent {
                r#ref,
                scope,
                present,
            }) => Self::Present(LeafPresent {
                r#ref,
                scope,
                present: !present,
            }),
            Self::Equals(l) => Self::NotEquals(l),
            Self::NotEquals(l) => Self::Equals(l),
            Self::In(l) => Self::NotIn(l),
            Self::NotIn(l) => Self::In(l),
            Self::Between(l) => Self::NotBetween(l),
            Self::NotBetween(l) => Self::Between(l),
            Self::LessThan(l) => Self::NotLessThan(l),
            Self::NotLessThan(l) => Self::LessThan(l),
            Self::LessThanOrEqual(l) => Self::NotLessThanOrEqual(l),
            Self::NotLessThanOrEqual(l) => Self::LessThanOrEqual(l),
            Self::GreaterThan(l) => Self::NotGreaterThan(l),
            Self::NotGreaterThan(l) => Self::GreaterThan(l),
            Self::GreaterThanOrEqual(l) => Self::NotGreaterThanOrEqual(l),
            Self::NotGreaterThanOrEqual(l) => Self::GreaterThanOrEqual(l),
        }
    }
}

impl From<&Assignment> for Leaf {
    fn from(a: &Assignment) -> Self {
        match &a.effect {
            AssignmentEffect::Set(v) => Leaf::Equals(LeafEquals {
                r#ref: a.r#ref.clone(),
                scope: Scope::Current,
                equals: v.clone(),
            }),
            AssignmentEffect::Clear => Leaf::Present(LeafPresent {
                r#ref: a.r#ref.clone(),
                scope: Scope::Current,
                present: false,
            }),
        }
    }
}

impl From<Leaf> for Predicate {
    fn from(l: Leaf) -> Self {
        match l {
            Leaf::Present(p) => p.into(),
            Leaf::Equals(p) => p.into(),
            Leaf::NotEquals(p) => {
                let p = p.into();
                PredNot { not: Box::new(p) }.into()
            }
            Leaf::In(p) => p.into(),
            Leaf::NotIn(p) => PredNot {
                not: Box::new(p.into()),
            }
            .into(),
            Leaf::Between(p) => p.into(),
            Leaf::NotBetween(p) => {
                let p = p.into();
                PredNot { not: Box::new(p) }.into()
            }
            Leaf::LessThan(p) => p.into(),
            Leaf::NotLessThan(p) => {
                let p = p.into();
                PredNot { not: Box::new(p) }.into()
            }
            Leaf::LessThanOrEqual(p) => p.into(),
            Leaf::NotLessThanOrEqual(p) => {
                let p = p.into();
                PredNot { not: Box::new(p) }.into()
            }
            Leaf::GreaterThan(p) => p.into(),
            Leaf::NotGreaterThan(p) => {
                let p = p.into();
                PredNot { not: Box::new(p) }.into()
            }
            Leaf::GreaterThanOrEqual(p) => p.into(),
            Leaf::NotGreaterThanOrEqual(p) => {
                let p = p.into();
                PredNot { not: Box::new(p) }.into()
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq)]
pub struct Conjunction(pub Vec<Leaf>);

impl Conjunction {
    pub fn refs(&self, out: &mut BTreeSet<String>) {
        self.iter().for_each(|leaf| {
            out.insert(leaf.r#ref().to_string());
        });
    }

    pub fn contains_all(&self, needle: &Conjunction) -> bool {
        multiset::subset(needle, self)
    }
}

impl Deref for Conjunction {
    type Target = Vec<Leaf>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Conjunction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a Conjunction {
    type Item = &'a Leaf;
    type IntoIter = Iter<'a, Leaf>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl IntoIterator for Conjunction {
    type Item = Leaf;
    type IntoIter = IntoIter<Leaf>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl PartialEq for Conjunction {
    fn eq(&self, other: &Self) -> bool {
        multiset::eq(self, other)
    }
}

impl Hash for Conjunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        multiset::hash(self, state);
    }
}

#[derive(Clone, Debug, Default, Eq)]
pub struct Dnf(pub Vec<Conjunction>);

impl Dnf {
    pub fn is_tautology(&self) -> bool {
        self.iter().any(|c| c.is_empty())
    }

    pub fn is_contradiction(&self) -> bool {
        self.is_empty()
    }

    pub fn refs(&self, out: &mut BTreeSet<String>) {
        self.iter().for_each(|c| c.refs(out));
    }
}

impl Deref for Dnf {
    type Target = Vec<Conjunction>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Dnf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a Dnf {
    type Item = &'a Conjunction;
    type IntoIter = Iter<'a, Conjunction>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl IntoIterator for Dnf {
    type Item = Conjunction;
    type IntoIter = IntoIter<Conjunction>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl PartialEq for Dnf {
    fn eq(&self, other: &Self) -> bool {
        multiset::eq(self, other)
    }
}

impl Hash for Dnf {
    fn hash<H: Hasher>(&self, state: &mut H) {
        multiset::hash(self, state);
    }
}

impl From<Predicate> for Dnf {
    fn from(p: Predicate) -> Self {
        Dnf(p.into_disjuncts())
    }
}

impl From<Dnf> for Predicate {
    fn from(dnf: Dnf) -> Self {
        if dnf.is_contradiction() {
            return Predicate::Bool(false);
        }

        if dnf.is_tautology() {
            return Predicate::Bool(true);
        }

        let mut disjuncts: Vec<Predicate> = dnf
            .into_iter()
            .map(|c| {
                if c.len() == 1 {
                    Predicate::from(
                        c.into_iter()
                            .next()
                            .expect("conjunction has exactly one element"),
                    )
                } else {
                    PredAll {
                        all: c.into_iter().map(Predicate::from).collect(),
                    }
                    .into()
                }
            })
            .collect();

        if disjuncts.len() == 1 {
            disjuncts.pop().expect("disjuncts has exactly one element")
        } else {
            PredAny { any: disjuncts }.into()
        }
    }
}

impl Predicate {
    pub fn into_disjuncts(self) -> Vec<Conjunction> {
        match self {
            Predicate::All(a) => {
                a.all
                    .into_iter()
                    .fold(vec![Conjunction(Vec::new())], |acc, clause| {
                        if acc.is_empty() {
                            return acc;
                        }

                        let parts = clause.into_disjuncts();
                        if parts.is_empty() {
                            return Vec::new();
                        }

                        acc.iter()
                            .flat_map(|prefix| {
                                parts.iter().map(move |part| {
                                    Conjunction(
                                        prefix
                                            .iter()
                                            .cloned()
                                            .chain(part.iter().cloned())
                                            .collect(),
                                    )
                                })
                            })
                            .collect()
                    })
            }
            Predicate::Any(a) => a.any.into_iter().flat_map(|p| p.into_disjuncts()).collect(),
            Predicate::Not(n) => Self::into_negation(*n.not),
            Predicate::Bool(true) => vec![Conjunction(Vec::new())],
            Predicate::Bool(false) => Vec::new(),
            leaf => {
                let leaf_val =
                    Leaf::try_from(leaf).expect("into_leaf called on non-leaf predicate");
                vec![Conjunction(vec![leaf_val])]
            }
        }
    }

    pub fn into_negation(p: Predicate) -> Vec<Conjunction> {
        match p {
            Predicate::All(a) => Predicate::Any(PredAny {
                any: a
                    .all
                    .into_iter()
                    .map(|c| PredNot { not: Box::new(c) }.into())
                    .collect(),
            })
            .into_disjuncts(),
            Predicate::Any(a) => Predicate::All(PredAll {
                all: a
                    .any
                    .into_iter()
                    .map(|c| PredNot { not: Box::new(c) }.into())
                    .collect(),
            })
            .into_disjuncts(),
            Predicate::Not(inner) => inner.not.into_disjuncts(),
            Predicate::Bool(b) => {
                if b {
                    Vec::new()
                } else {
                    vec![Conjunction(Vec::new())]
                }
            }
            leaf => {
                let leaf_val =
                    Leaf::try_from(leaf).expect("into_leaf called on non-leaf predicate");
                vec![Conjunction(vec![leaf_val.negated()])]
            }
        }
    }
}

impl TryFrom<Predicate> for Leaf {
    type Error = anyhow::Error;

    fn try_from(p: Predicate) -> Result<Self, Self::Error> {
        match p {
            Predicate::Present(p) => Ok(Leaf::Present(p)),
            Predicate::Equals(p) => Ok(Leaf::Equals(p)),
            Predicate::In(p) => Ok(Leaf::In(p)),
            Predicate::Between(p) => Ok(Leaf::Between(p)),
            Predicate::LessThan(p) => Ok(Leaf::LessThan(p)),
            Predicate::LessThanOrEqual(p) => Ok(Leaf::LessThanOrEqual(p)),
            Predicate::GreaterThan(p) => Ok(Leaf::GreaterThan(p)),
            Predicate::GreaterThanOrEqual(p) => Ok(Leaf::GreaterThanOrEqual(p)),
            Predicate::All(_) | Predicate::Any(_) | Predicate::Not(_) | Predicate::Bool(_) => {
                bail!("into_leaf called on non-leaf predicate")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lp(name: &str, present: bool) -> Leaf {
        Leaf::Present(LeafPresent {
            r#ref: name.into(),
            scope: Scope::Current,
            present,
        })
    }

    fn le(name: &str, v: serde_json::Value) -> Leaf {
        Leaf::Equals(LeafEquals {
            r#ref: name.into(),
            scope: Scope::Current,
            equals: v,
        })
    }

    #[test]
    fn literal_negation_round_trips() {
        let l = le("x", json!(1));
        assert_eq!(l.clone().negated().negated(), l);
        let p = lp("y", true);
        assert_eq!(p.clone().negated().negated(), p);
    }

    #[test]
    fn into_dnf_bool_true_is_tautology() {
        assert!(Dnf::from(Predicate::Bool(true)).is_tautology());
    }

    #[test]
    fn into_dnf_bool_false_is_contradiction() {
        assert!(Dnf::from(Predicate::Bool(false)).is_contradiction());
    }

    #[test]
    fn into_dnf_empty_all_is_tautology() {
        let dnf = Dnf::from(Predicate::from(PredAll { all: vec![] }));
        assert!(dnf.is_tautology());
    }

    #[test]
    fn into_dnf_empty_any_is_contradiction() {
        let dnf = Dnf::from(Predicate::from(PredAny { any: vec![] }));
        assert!(dnf.is_contradiction());
    }

    #[test]
    fn not_equals_becomes_dedicated_literal() {
        let dnf = Dnf::from(Predicate::from(PredNot {
            not: Box::new(
                LeafEquals {
                    r#ref: "a".into(),
                    scope: Scope::Current,
                    equals: json!(1),
                }
                .into(),
            ),
        }));

        assert_eq!(dnf.len(), 1);
        assert_eq!(dnf[0].len(), 1);
        assert!(matches!(dnf[0][0], Leaf::NotEquals(_)));
    }

    #[test]
    fn not_present_flips_polarity_in_place() {
        let dnf = Dnf::from(Predicate::from(PredNot {
            not: Box::new(
                LeafPresent {
                    r#ref: "a".into(),
                    scope: Scope::Current,
                    present: true,
                }
                .into(),
            ),
        }));
        assert_eq!(dnf[0][0], lp("a", false));
    }

    #[test]
    fn double_negation_cancels() {
        let dnf = Dnf::from(Predicate::from(PredNot {
            not: Box::new(
                PredNot {
                    not: Box::new(
                        LeafEquals {
                            r#ref: "a".into(),
                            scope: Scope::Current,
                            equals: json!(1),
                        }
                        .into(),
                    ),
                }
                .into(),
            ),
        }));
        assert_eq!(dnf[0][0], le("a", json!(1)));
    }

    #[test]
    fn distribution_across_inner_any() {
        let dnf = Dnf::from(Predicate::from(PredAll {
            all: vec![
                LeafPresent {
                    r#ref: "A".into(),
                    scope: Scope::Current,
                    present: true,
                }
                .into(),
                PredAny {
                    any: vec![
                        LeafEquals {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            equals: json!(1),
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "C".into(),
                            scope: Scope::Current,
                            equals: json!(2),
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        }));
        assert_eq!(dnf.len(), 2);
        for c in dnf {
            assert_eq!(c.len(), 2);
            assert!(c.iter().any(|l| *l == lp("A", true)));
        }
    }

    #[test]
    fn conjunction_eq_is_multiset() {
        let a = Conjunction(vec![le("x", json!(1)), lp("y", true)]);
        let b = Conjunction(vec![lp("y", true), le("x", json!(1))]);
        assert_eq!(a, b);
    }

    #[test]
    fn conjunction_contains_all_respects_multiset() {
        let haystack = Conjunction(vec![le("x", json!(1)), lp("y", true), le("x", json!(2))]);
        let needle_ok = Conjunction(vec![le("x", json!(1))]);
        let needle_dup = Conjunction(vec![le("x", json!(1)), le("x", json!(1))]);
        let needle_missing = Conjunction(vec![le("z", json!(0))]);
        assert!(haystack.contains_all(&needle_ok));
        assert!(!haystack.contains_all(&needle_dup));
        assert!(!haystack.contains_all(&needle_missing));
    }

    #[test]
    fn round_trip_predicate_to_dnf_and_back_is_logically_equivalent() {
        let p: Predicate = PredAll {
            all: vec![
                LeafEquals {
                    r#ref: "A".into(),
                    scope: Scope::Current,
                    equals: json!(1),
                }
                .into(),
                PredAny {
                    any: vec![
                        LeafPresent {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafPresent {
                            r#ref: "C".into(),
                            scope: Scope::Current,
                            present: false,
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        }
        .into();
        let dnf = Dnf::from(p.clone());
        let p2: Predicate = dnf.clone().into();
        assert_eq!(Dnf::from(p2), dnf);
    }

    #[test]
    fn de_morgan_not_all_becomes_any_of_negated_literals() {
        let dnf = Dnf::from(Predicate::from(PredNot {
            not: Box::new(
                PredAll {
                    all: vec![
                        LeafEquals {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            equals: json!(1),
                        }
                        .into(),
                        LeafPresent {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
        }));
        assert_eq!(dnf.len(), 2);
        let flat: Vec<&Leaf> = dnf.iter().flat_map(|c| c.iter()).collect();
        assert!(flat.iter().any(|l| matches!(l, Leaf::NotEquals(_))));
        assert!(
            flat.iter()
                .any(|l| matches!(l, Leaf::Present(p) if !p.present))
        );
    }
}
