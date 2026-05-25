use std::{
    collections::BTreeSet,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    slice::Iter,
    vec::IntoIter,
};

use serde::Deserialize;
use serde_json::Value;

use crate::util::multiset;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged, deny_unknown_fields)]
pub enum Predicate {
    Bool(bool),
    All(PredAll),
    Any(PredAny),
    Not(PredNot),
    Equals(LeafEquals),
    In(LeafIn),
    Between(LeafBetween),
    LessThan(LeafLt),
    LessThanOrEqual(LeafLte),
    GreaterThan(LeafGt),
    GreaterThanOrEqual(LeafGte),
    Present(LeafPresent),
}

#[derive(Clone, Debug, Deserialize, Eq)]
#[serde(deny_unknown_fields)]
pub struct PredAll {
    pub all: Vec<Predicate>,
}

impl Deref for PredAll {
    type Target = Vec<Predicate>;

    fn deref(&self) -> &Self::Target {
        &self.all
    }
}

impl DerefMut for PredAll {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.all
    }
}

impl<'a> IntoIterator for &'a PredAll {
    type Item = &'a Predicate;
    type IntoIter = Iter<'a, Predicate>;

    fn into_iter(self) -> Self::IntoIter {
        self.all.iter()
    }
}

impl IntoIterator for PredAll {
    type Item = Predicate;
    type IntoIter = IntoIter<Predicate>;

    fn into_iter(self) -> Self::IntoIter {
        self.all.into_iter()
    }
}

impl PartialEq for PredAll {
    fn eq(&self, other: &Self) -> bool {
        multiset::eq(&self.all, &other.all)
    }
}

impl Hash for PredAll {
    fn hash<H: Hasher>(&self, state: &mut H) {
        multiset::hash(&self.all, state);
    }
}

#[derive(Clone, Debug, Deserialize, Eq)]
#[serde(deny_unknown_fields)]
pub struct PredAny {
    pub any: Vec<Predicate>,
}

impl Deref for PredAny {
    type Target = Vec<Predicate>;

    fn deref(&self) -> &Self::Target {
        &self.any
    }
}

impl DerefMut for PredAny {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.any
    }
}

impl<'a> IntoIterator for &'a PredAny {
    type Item = &'a Predicate;
    type IntoIter = Iter<'a, Predicate>;

    fn into_iter(self) -> Self::IntoIter {
        self.any.iter()
    }
}

impl IntoIterator for PredAny {
    type Item = Predicate;
    type IntoIter = IntoIter<Predicate>;

    fn into_iter(self) -> Self::IntoIter {
        self.any.into_iter()
    }
}

impl PartialEq for PredAny {
    fn eq(&self, other: &Self) -> bool {
        multiset::eq(&self.any, &other.any)
    }
}

impl Hash for PredAny {
    fn hash<H: Hasher>(&self, state: &mut H) {
        multiset::hash(&self.any, state);
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct PredNot {
    pub not: Box<Predicate>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct LeafEquals {
    pub r#ref: String,
    pub equals: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct LeafIn {
    pub r#ref: String,
    #[serde(rename = "in")]
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct LeafBetween {
    pub r#ref: String,
    pub min: Value,
    pub max: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct LeafLt {
    pub r#ref: String,
    pub lt: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct LeafLte {
    pub r#ref: String,
    pub lte: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct LeafGt {
    pub r#ref: String,
    pub gt: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct LeafGte {
    pub r#ref: String,
    pub gte: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct LeafPresent {
    pub r#ref: String,
    pub present: bool,
}

macro_rules! impl_into_predicate {
    ($($variant:ident($inner:ty)),* $(,)?) => {
        $(
            impl From<$inner> for Predicate {
                fn from(value: $inner) -> Self {
                    Predicate::$variant(value)
                }
            }
        )*
    };
}

impl_into_predicate!(
    All(PredAll),
    Any(PredAny),
    Not(PredNot),
    Equals(LeafEquals),
    In(LeafIn),
    Between(LeafBetween),
    LessThan(LeafLt),
    LessThanOrEqual(LeafLte),
    GreaterThan(LeafGt),
    GreaterThanOrEqual(LeafGte),
    Present(LeafPresent),
);

impl Predicate {
    pub fn collect<T: Ord>(&self, out: &mut BTreeSet<T>, f: &impl Fn(&Predicate) -> Option<T>) {
        match self {
            Predicate::All(p) => p.all.iter().for_each(|c| c.collect(out, f)),
            Predicate::Any(p) => p.any.iter().for_each(|c| c.collect(out, f)),
            Predicate::Not(p) => p.not.collect(out, f),
            leaf => {
                if let Some(v) = f(leaf) {
                    out.insert(v);
                }
            }
        }
    }

    pub fn refs(&self, out: &mut BTreeSet<String>) {
        self.collect(out, &|p| match p {
            Predicate::Present(p) => Some(p.r#ref.clone()),
            Predicate::Equals(p) => Some(p.r#ref.clone()),
            Predicate::In(p) => Some(p.r#ref.clone()),
            Predicate::Between(p) => Some(p.r#ref.clone()),
            Predicate::LessThan(p) => Some(p.r#ref.clone()),
            Predicate::LessThanOrEqual(p) => Some(p.r#ref.clone()),
            Predicate::GreaterThan(p) => Some(p.r#ref.clone()),
            Predicate::GreaterThanOrEqual(p) => Some(p.r#ref.clone()),
            _ => None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(json: &str) -> Predicate {
        serde_json::from_str(json).unwrap()
    }

    fn parse_err(json: &str) {
        assert!(
            serde_json::from_str::<Predicate>(json).is_err(),
            "expected parse error for: {json}"
        );
    }

    #[test]
    fn nested_logic_round_trips() {
        let pred = parse(
            r#"{ "all": [
                { "ref": "x", "equals": 5 },
                { "not": { "ref": "y", "present": true } }
            ] }"#,
        );
        assert!(matches!(pred, Predicate::All(_)));
    }

    #[test]
    fn bool_literal_parses_both_polarities() {
        assert!(matches!(parse("true"), Predicate::Bool(true)));
        assert!(matches!(parse("false"), Predicate::Bool(false)));
    }

    #[test]
    fn empty_all_and_any_arrays_parse() {
        assert!(matches!(parse(r#"{ "all": [] }"#), Predicate::All(_)));
        assert!(matches!(parse(r#"{ "any": [] }"#), Predicate::Any(_)));
    }

    #[test]
    fn not_is_recursively_boxed() {
        let pred = parse(r#"{ "not": { "not": { "ref": "x", "present": true } } }"#);
        let Predicate::Not(outer) = pred else {
            panic!()
        };
        let Predicate::Not(inner) = *outer.not else {
            panic!()
        };
        assert!(matches!(*inner.not, Predicate::Present(_)));
    }

    #[test]
    fn comparator_leaves_disambiguate_by_key_name() {
        assert!(matches!(
            parse(r#"{ "ref": "x", "lt": 5 }"#),
            Predicate::LessThan(_)
        ));
        assert!(matches!(
            parse(r#"{ "ref": "x", "lte": 5 }"#),
            Predicate::LessThanOrEqual(_)
        ));
        assert!(matches!(
            parse(r#"{ "ref": "x", "gt": 5 }"#),
            Predicate::GreaterThan(_)
        ));
        assert!(matches!(
            parse(r#"{ "ref": "x", "gte": 5 }"#),
            Predicate::GreaterThanOrEqual(_)
        ));
    }

    #[test]
    fn equals_accepts_any_json_scalar() {
        let eq: Predicate = parse(r#"{ "ref": "x", "equals": "hello" }"#);
        let Predicate::Equals(leaf) = eq else {
            panic!()
        };
        assert!(leaf.equals.is_string());

        let eq: Predicate = parse(r#"{ "ref": "x", "equals": true }"#);
        let Predicate::Equals(leaf) = eq else {
            panic!()
        };
        assert_eq!(leaf.equals.as_bool(), Some(true));
    }

    #[test]
    fn between_requires_both_bounds() {
        parse_err(r#"{ "ref": "x", "min": 5 }"#);
        parse_err(r#"{ "ref": "x", "max": 5 }"#);
        let bt = parse(r#"{ "ref": "x", "min": 0, "max": 10 }"#);
        assert!(matches!(bt, Predicate::Between(_)));
    }

    #[test]
    fn present_accepts_both_polarities() {
        let t = parse(r#"{ "ref": "x", "present": true }"#);
        let f = parse(r#"{ "ref": "x", "present": false }"#);
        let Predicate::Present(t) = t else { panic!() };
        let Predicate::Present(f) = f else { panic!() };
        assert!(t.present);
        assert!(!f.present);
    }

    #[test]
    fn mixing_logic_keys_is_rejected() {
        parse_err(r#"{ "all": [], "any": [] }"#);
    }

    #[test]
    fn ref_required_on_all_leaves() {
        parse_err(r#"{ "equals": 5 }"#);
        parse_err(r#"{ "present": true }"#);
        parse_err(r#"{ "min": 0, "max": 10 }"#);
    }

    #[test]
    fn collect_refs_leaves() {
        let mut out = BTreeSet::new();
        Predicate::from(LeafEquals {
            r#ref: "A".into(),
            equals: json!("x"),
        })
        .refs(&mut out);
        Predicate::from(LeafPresent {
            r#ref: "B".into(),
            present: true,
        })
        .refs(&mut out);
        Predicate::from(LeafIn {
            r#ref: "C".into(),
            values: vec![json!(1), json!(2)],
        })
        .refs(&mut out);
        assert_eq!(out, ["A", "B", "C"].iter().map(|s| s.to_string()).collect());
    }

    #[test]
    fn collect_refs_nested() {
        let mut out = BTreeSet::new();
        Predicate::from(PredAll {
            all: vec![
                PredNot {
                    not: Box::new(
                        LeafEquals {
                            r#ref: "A".into(),
                            equals: json!("x"),
                        }
                        .into(),
                    ),
                }
                .into(),
                PredAny {
                    any: vec![
                        LeafPresent {
                            r#ref: "B".into(),
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "C".into(),
                            equals: json!(1),
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        })
        .refs(&mut out);
        assert_eq!(out, ["A", "B", "C"].iter().map(|s| s.to_string()).collect());
    }

    #[test]
    fn collect_refs_bool_has_no_refs() {
        let mut out = BTreeSet::new();
        Predicate::Bool(true).refs(&mut out);
        Predicate::Bool(false).refs(&mut out);
        assert!(out.is_empty());
    }
}
