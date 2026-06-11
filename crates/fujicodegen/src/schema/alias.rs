use std::cmp::Ordering;

use serde_json::Value;

use crate::ast::{
    Conjunction, Dnf, Leaf, PredAll, PredNot, Predicate, Rule, Severity, Transformation,
};

#[derive(Clone, Debug)]
pub struct NormalizedTransformation {
    pub trigger: Dnf,
    pub expansion: Conjunction,
}

impl From<Transformation> for Option<NormalizedTransformation> {
    fn from(t: Transformation) -> Self {
        let when = t.when?;
        if t.apply.is_empty() {
            return None;
        }
        let trigger = Dnf::from(when);
        if trigger.is_contradiction() {
            return None;
        }
        let expansion = Conjunction(t.apply.iter().map(Leaf::from).collect());
        Some(NormalizedTransformation { trigger, expansion })
    }
}

impl Dnf {
    #[must_use]
    pub fn transform(self, alias: &NormalizedTransformation) -> Self {
        Self(self.into_iter().map(|c| c.transform(alias)).collect())
    }
}

impl Conjunction {
    #[must_use]
    pub fn transform(mut self, alias: &NormalizedTransformation) -> Self {
        for disjunct in &alias.trigger {
            if self.contains_all(disjunct) {
                for lit in disjunct {
                    if let Some(pos) = self.iter().position(|l| l == lit) {
                        self.swap_remove(pos);
                    }
                }
                self.extend(alias.expansion.clone());
                return self;
            }
        }
        self
    }
}

#[derive(Clone, Debug)]
pub struct NormalizedRule {
    pub severity: Severity,
    pub message: String,
    pub when: Dnf,
}

impl NormalizedRule {
    pub fn from_rule(rule: &Rule, aliases: &[NormalizedTransformation]) -> Self {
        let initial = Dnf::from(rule.when.clone());
        let substituted = aliases.iter().fold(initial, Dnf::transform);
        let when = Self::exempt_alias_expansions(substituted, aliases, &rule.message);
        Self {
            severity: rule.severity,
            message: rule.message.clone(),
            when,
        }
    }

    fn exempt_alias_expansions(
        dnf: Dnf,
        aliases: &[NormalizedTransformation],
        message: &str,
    ) -> Dnf {
        let mut out: Vec<Conjunction> = Vec::new();
        for conjunction in dnf {
            let mut guards: Vec<&Conjunction> = Vec::new();
            for alias in aliases {
                let expansion = &alias.expansion;
                if conjunction.contains_all(expansion) {
                    continue;
                }
                let view = Expansion(expansion);
                match view.entails(&conjunction) {
                    Entailment::Proven => guards.push(expansion),
                    Entailment::Unknown => {
                        let fields: Vec<&str> = conjunction
                            .iter()
                            .filter(|conclusion| {
                                view.entails_leaf(conclusion) == Entailment::Unknown
                            })
                            .map(Leaf::r#ref)
                            .collect();
                        println!(
                            "cargo:warning=alias-expansion exemption for rule {message:?} is indeterminate: \
                            a non-numeric ordered comparison on {fields:?} could not be resolved, so the decomposed \
                            alias value may be silently rejected at runtime. Compare against discrete values \
                            (e.g. a `lookup` encoding) or carve the alias out of the rule explicitly."
                        );
                    }
                    Entailment::Denied => {}
                }
            }

            if guards.is_empty() {
                out.push(conjunction);
                continue;
            }

            let mut all: Vec<Predicate> =
                conjunction.iter().cloned().map(Predicate::from).collect();
            for expansion in guards {
                let expansion_pred = PredAll {
                    all: expansion.iter().cloned().map(Predicate::from).collect(),
                };
                all.push(
                    PredNot {
                        not: Box::new(expansion_pred.into()),
                    }
                    .into(),
                );
            }
            out.extend(Dnf::from(Predicate::from(PredAll { all })));
        }
        Dnf(out)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Entailment {
    Proven,
    Denied,
    Unknown,
}

impl Entailment {
    const fn from_bool(holds: bool) -> Self {
        if holds { Self::Proven } else { Self::Denied }
    }

    fn from_option<T>(resolved: Option<T>, holds: impl Fn(T) -> bool) -> Self {
        resolved.map_or(Self::Unknown, |value| Self::from_bool(holds(value)))
    }
}

#[derive(Clone, Copy)]
struct Expansion<'a>(&'a Conjunction);

impl Expansion<'_> {
    fn entails(self, conjunction: &Conjunction) -> Entailment {
        let mut verdict = Entailment::Proven;
        for conclusion in conjunction {
            match self.entails_leaf(conclusion) {
                Entailment::Denied => return Entailment::Denied,
                Entailment::Unknown => verdict = Entailment::Unknown,
                Entailment::Proven => {}
            }
        }
        verdict
    }

    fn entails_leaf(self, conclusion: &Leaf) -> Entailment {
        let mut verdict = Entailment::Denied;
        for premise in self.0 {
            let leaf = if premise.r#ref() != conclusion.r#ref()
                || premise.scope() != conclusion.scope()
            {
                Entailment::Denied
            } else {
                match premise {
                    Leaf::Equals(equals) => {
                        let value: &Value = &equals.equals;
                        let cmp = |target: &Value| value.as_f64()?.partial_cmp(&target.as_f64()?);
                        let between = |lo: &Value, hi: &Value| {
                            let value = value.as_f64()?;
                            Some(lo.as_f64()? <= value && value <= hi.as_f64()?)
                        };
                        match conclusion {
                            Leaf::Present(p) => Entailment::from_bool(p.present),
                            Leaf::Equals(l) => Entailment::from_bool(&l.equals == value),
                            Leaf::NotEquals(l) => Entailment::from_bool(&l.equals != value),
                            Leaf::In(l) => Entailment::from_bool(l.values.contains(value)),
                            Leaf::NotIn(l) => Entailment::from_bool(!l.values.contains(value)),
                            Leaf::Between(l) => {
                                Entailment::from_option(between(&l.min, &l.max), |b| b)
                            }
                            Leaf::NotBetween(l) => {
                                Entailment::from_option(between(&l.min, &l.max), |b| !b)
                            }
                            Leaf::LessThan(l) => {
                                Entailment::from_option(cmp(&l.lt), |o| o == Ordering::Less)
                            }
                            Leaf::NotLessThan(l) => {
                                Entailment::from_option(cmp(&l.lt), |o| o != Ordering::Less)
                            }
                            Leaf::LessThanOrEqual(l) => {
                                Entailment::from_option(cmp(&l.lte), |o| o != Ordering::Greater)
                            }
                            Leaf::NotLessThanOrEqual(l) => {
                                Entailment::from_option(cmp(&l.lte), |o| o == Ordering::Greater)
                            }
                            Leaf::GreaterThan(l) => {
                                Entailment::from_option(cmp(&l.gt), |o| o == Ordering::Greater)
                            }
                            Leaf::NotGreaterThan(l) => {
                                Entailment::from_option(cmp(&l.gt), |o| o != Ordering::Greater)
                            }
                            Leaf::GreaterThanOrEqual(l) => {
                                Entailment::from_option(cmp(&l.gte), |o| o != Ordering::Less)
                            }
                            Leaf::NotGreaterThanOrEqual(l) => {
                                Entailment::from_option(cmp(&l.gte), |o| o == Ordering::Less)
                            }
                        }
                    }
                    Leaf::Present(present) if !present.present => {
                        Entailment::from_bool(matches!(conclusion, Leaf::Present(p) if !p.present))
                    }
                    _ => Entailment::Denied,
                }
            };
            match leaf {
                Entailment::Proven => return Entailment::Proven,
                Entailment::Unknown => verdict = Entailment::Unknown,
                Entailment::Denied => {}
            }
        }
        verdict
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        Assignment, AssignmentEffect, LeafEquals, LeafIn, LeafLt, LeafPresent, PredAll, PredNot,
        Predicate, Rule, Scope, Severity,
    };
    use serde_json::{Value, json};

    fn normalize(
        rules: &[Rule],
        transformations: impl IntoIterator<Item = Transformation>,
    ) -> Vec<NormalizedRule> {
        let aliases: Vec<NormalizedTransformation> = transformations
            .into_iter()
            .filter_map(Option::from)
            .collect();
        rules
            .iter()
            .map(|r| NormalizedRule::from_rule(r, &aliases))
            .collect()
    }

    fn alias_t(when: Predicate, apply: Vec<(&str, Value)>) -> Transformation {
        Transformation {
            when: Some(when),
            apply: apply
                .into_iter()
                .map(|(r, v)| Assignment {
                    r#ref: r.to_string(),
                    effect: AssignmentEffect::Set(v),
                })
                .collect(),
        }
    }

    fn rule(when: Predicate) -> Rule {
        Rule {
            severity: Severity::Error,
            message: "test".into(),
            when,
        }
    }

    fn le(name: &str, v: Value) -> Leaf {
        Leaf::Equals(LeafEquals {
            r#ref: name.into(),
            scope: Scope::Current,
            equals: v,
        })
    }

    fn lp(name: &str, present: bool) -> Leaf {
        Leaf::Present(LeafPresent {
            r#ref: name.into(),
            scope: Scope::Current,
            present,
        })
    }

    fn lne(name: &str, v: Value) -> Leaf {
        Leaf::NotEquals(LeafEquals {
            r#ref: name.into(),
            scope: Scope::Current,
            equals: v,
        })
    }

    fn dr_plus_alias() -> Transformation {
        alias_t(
            LeafEquals {
                r#ref: "dr".into(),
                scope: Scope::Current,
                equals: json!("hdr800_plus"),
            }
            .into(),
            vec![("dr", json!("hdr800")), ("drp", json!("plus"))],
        )
    }

    #[test]
    fn equals_trigger_expands_to_apply_conjunction() {
        let ts = vec![alias_t(
            LeafEquals {
                r#ref: "dr".into(),
                scope: Scope::Current,
                equals: json!("hdr800_plus"),
            }
            .into(),
            vec![("dr", json!("hdr800")), ("drp", json!("plus"))],
        )];
        let rules = vec![rule(
            LeafEquals {
                r#ref: "dr".into(),
                scope: Scope::Current,
                equals: json!("hdr800_plus"),
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        assert_eq!(out[0].when.0.len(), 1);
        let conj = &out[0].when.0[0];
        assert_eq!(conj.0.len(), 2);
        assert!(conj.0.contains(&le("dr", json!("hdr800"))));
        assert!(conj.0.contains(&le("drp", json!("plus"))));
    }

    #[test]
    fn unmatched_leaves_pass_through() {
        let ts = vec![alias_t(
            LeafEquals {
                r#ref: "dr".into(),
                scope: Scope::Current,
                equals: json!("hdr800_plus"),
            }
            .into(),
            vec![("dr", json!("hdr800")), ("drp", json!("plus"))],
        )];
        let rules = vec![rule(
            LeafEquals {
                r#ref: "film_simulation".into(),
                scope: Scope::Current,
                equals: json!("provia"),
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        assert_eq!(
            out[0].when.0[0].0,
            vec![le("film_simulation", json!("provia"))]
        );
    }

    #[test]
    fn clear_in_apply_expands_to_present_false() {
        let ts = vec![Transformation {
            when: Some(
                LeafEquals {
                    r#ref: "wb".into(),
                    scope: Scope::Current,
                    equals: json!("as_shot"),
                }
                .into(),
            ),
            apply: vec![Assignment {
                r#ref: "wb_shift_red".into(),
                effect: AssignmentEffect::Clear,
            }],
        }];
        let rules = vec![rule(
            LeafEquals {
                r#ref: "wb".into(),
                scope: Scope::Current,
                equals: json!("as_shot"),
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        assert_eq!(out[0].when.0[0].0, vec![lp("wb_shift_red", false)]);
    }

    #[test]
    fn mixed_set_and_clear_apply_produces_conjunction() {
        let ts = vec![Transformation {
            when: Some(
                LeafEquals {
                    r#ref: "wb".into(),
                    scope: Scope::Current,
                    equals: json!("as_shot"),
                }
                .into(),
            ),
            apply: vec![
                Assignment {
                    r#ref: "wb_lock".into(),
                    effect: AssignmentEffect::Set(json!(true)),
                },
                Assignment {
                    r#ref: "wb_shift_red".into(),
                    effect: AssignmentEffect::Clear,
                },
            ],
        }];
        let rules = vec![rule(
            LeafEquals {
                r#ref: "wb".into(),
                scope: Scope::Current,
                equals: json!("as_shot"),
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        let conj = &out[0].when.0[0];
        assert!(conj.0.contains(&le("wb_lock", json!(true))));
        assert!(conj.0.contains(&lp("wb_shift_red", false)));
        assert_eq!(conj.0.len(), 2);
    }

    #[test]
    fn duplicate_triggers_apply_first_only_per_conjunction() {
        let ts = vec![
            alias_t(
                LeafEquals {
                    r#ref: "dr".into(),
                    scope: Scope::Current,
                    equals: json!("hdr800_plus"),
                }
                .into(),
                vec![("dr", json!("hdr800"))],
            ),
            alias_t(
                LeafEquals {
                    r#ref: "dr".into(),
                    scope: Scope::Current,
                    equals: json!("hdr800_plus"),
                }
                .into(),
                vec![("drp", json!("plus"))],
            ),
        ];
        let rules = vec![rule(
            LeafEquals {
                r#ref: "dr".into(),
                scope: Scope::Current,
                equals: json!("hdr800_plus"),
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        // First alias substitutes dr -> hdr800; second alias finds no
        // hdr800_plus match in the rewritten conjunction.
        assert_eq!(out[0].when.0[0].0, vec![le("dr", json!("hdr800"))]);
    }

    #[test]
    fn substitution_recurses_into_logic_nodes() {
        let ts = vec![alias_t(
            LeafEquals {
                r#ref: "dr".into(),
                scope: Scope::Current,
                equals: json!("hdr800_plus"),
            }
            .into(),
            vec![("dr", json!("hdr800")), ("drp", json!("plus"))],
        )];
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "dr".into(),
                        scope: Scope::Current,
                        equals: json!("hdr800_plus"),
                    }
                    .into(),
                    PredNot {
                        not: Box::new(
                            LeafEquals {
                                r#ref: "foo".into(),
                                scope: Scope::Current,
                                equals: json!("bar"),
                            }
                            .into(),
                        ),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        let conj = &out[0].when.0[0];
        assert!(conj.0.iter().any(|l| matches!(l, Leaf::NotEquals(_))));
        assert!(conj.0.contains(&le("dr", json!("hdr800"))));
        assert!(conj.0.contains(&le("drp", json!("plus"))));
    }

    #[test]
    fn compound_when_trigger_is_recognised() {
        let ts = vec![alias_t(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "a".into(),
                        scope: Scope::Current,
                        equals: json!(1),
                    }
                    .into(),
                    LeafPresent {
                        r#ref: "b".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                ],
            }
            .into(),
            vec![("x", json!("v1")), ("y", json!("v2"))],
        )];
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "a".into(),
                        scope: Scope::Current,
                        equals: json!(1),
                    }
                    .into(),
                    LeafPresent {
                        r#ref: "b".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        let conj = &out[0].when.0[0];
        assert!(conj.0.contains(&le("x", json!("v1"))));
        assert!(conj.0.contains(&le("y", json!("v2"))));
        assert_eq!(conj.0.len(), 2);
    }

    #[test]
    fn trigger_match_is_order_insensitive_under_all() {
        let ts = vec![alias_t(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "a".into(),
                        scope: Scope::Current,
                        equals: json!(1),
                    }
                    .into(),
                    LeafPresent {
                        r#ref: "b".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                ],
            }
            .into(),
            vec![("x", json!("v"))],
        )];
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "b".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "a".into(),
                        scope: Scope::Current,
                        equals: json!(1),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        assert_eq!(out[0].when.0[0].0, vec![le("x", json!("v"))]);
    }

    #[test]
    fn present_trigger_does_not_match_negated_rule() {
        let ts = vec![alias_t(
            LeafPresent {
                r#ref: "legacy_field".into(),
                scope: Scope::Current,
                present: true,
            }
            .into(),
            vec![("modern_field", json!("default"))],
        )];
        let rules = vec![rule(
            PredNot {
                not: Box::new(
                    LeafPresent {
                        r#ref: "legacy_field".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                ),
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        assert_eq!(out[0].when.0[0].0, vec![lp("legacy_field", false)]);
    }

    #[test]
    fn chained_aliases_substitute_in_declaration_order() {
        let ts = vec![
            alias_t(
                LeafEquals {
                    r#ref: "a".into(),
                    scope: Scope::Current,
                    equals: json!("x"),
                }
                .into(),
                vec![("b", json!("y"))],
            ),
            alias_t(
                LeafEquals {
                    r#ref: "b".into(),
                    scope: Scope::Current,
                    equals: json!("y"),
                }
                .into(),
                vec![("c", json!("z"))],
            ),
        ];
        let rules = vec![rule(
            LeafEquals {
                r#ref: "a".into(),
                scope: Scope::Current,
                equals: json!("x"),
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        assert_eq!(out[0].when.0[0].0, vec![le("c", json!("z"))]);
    }

    #[test]
    fn compound_trigger_matches_superset_conjunction() {
        let ts = vec![alias_t(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "a".into(),
                        scope: Scope::Current,
                        equals: json!(1),
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "b".into(),
                        scope: Scope::Current,
                        equals: json!(2),
                    }
                    .into(),
                ],
            }
            .into(),
            vec![("x", json!("v"))],
        )];
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "a".into(),
                        scope: Scope::Current,
                        equals: json!(1),
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "b".into(),
                        scope: Scope::Current,
                        equals: json!(2),
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "c".into(),
                        scope: Scope::Current,
                        equals: json!(3),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let out = normalize(&rules, ts);
        let conj = &out[0].when.0[0];
        assert!(conj.0.contains(&le("c", json!(3))));
        assert!(conj.0.contains(&le("x", json!("v"))));
        assert_eq!(conj.0.len(), 2);
    }

    #[test]
    fn alias_expansion_exempts_an_incidental_rule() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    lp("dr", true).into(),
                    PredNot {
                        not: Box::new(le("drp", json!("off")).into()),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let out = normalize(&rules, vec![dr_plus_alias()]);

        assert_eq!(out[0].when.0.len(), 2);
        let flat: Vec<&Leaf> = out[0].when.0.iter().flat_map(|c| c.iter()).collect();
        assert!(flat.contains(&&lne("dr", json!("hdr800"))));
        assert!(flat.contains(&&lne("drp", json!("plus"))));
        for conj in &out[0].when.0 {
            assert!(conj.0.contains(&lp("dr", true)));
            assert!(conj.0.contains(&lne("drp", json!("off"))));
        }
    }

    #[test]
    fn rule_written_about_the_alias_value_is_not_exempted() {
        let rules = vec![rule(
            LeafEquals {
                r#ref: "dr".into(),
                scope: Scope::Current,
                equals: json!("hdr800_plus"),
            }
            .into(),
        )];
        let out = normalize(&rules, vec![dr_plus_alias()]);

        assert_eq!(out[0].when.0.len(), 1);
        let conj = &out[0].when.0[0];
        assert_eq!(conj.0.len(), 2);
        assert!(conj.0.contains(&le("dr", json!("hdr800"))));
        assert!(conj.0.contains(&le("drp", json!("plus"))));
    }

    #[test]
    fn original_scope_rule_is_not_exempted() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "dr".into(),
                        scope: Scope::Original,
                        equals: json!("hdr400"),
                    }
                    .into(),
                    PredNot {
                        not: Box::new(
                            LeafIn {
                                r#ref: "dr".into(),
                                scope: Scope::Current,
                                values: vec![json!("hdr400")],
                            }
                            .into(),
                        ),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let out = normalize(&rules, vec![dr_plus_alias()]);

        assert_eq!(out[0].when.0.len(), 1);
        let flat: Vec<&Leaf> = out[0].when.0.iter().flat_map(|c| c.iter()).collect();
        assert!(
            !flat
                .iter()
                .any(|l| matches!(l, Leaf::NotEquals(e) if e.r#ref == "drp")),
            "expansion guard must not be added: {:?}",
            out[0].when,
        );
    }

    #[test]
    fn indeterminate_ordered_comparison_is_left_unexempted() {
        let ts = vec![alias_t(
            LeafEquals {
                r#ref: "mode".into(),
                scope: Scope::Current,
                equals: json!("special"),
            }
            .into(),
            vec![("iso", json!("high"))],
        )];
        let lt = LeafLt {
            r#ref: "iso".into(),
            scope: Scope::Current,
            lt: json!("low"),
        };
        let rules = vec![rule(lt.clone().into())];
        let out = normalize(&rules, ts);

        assert_eq!(out[0].when.0.len(), 1);
        assert_eq!(out[0].when.0[0].0, vec![Leaf::LessThan(lt)]);
    }
}
