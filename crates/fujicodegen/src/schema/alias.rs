use crate::ast::{Conjunction, Dnf, Leaf, Rule, Severity, Transformation};

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
    pub fn transform(self, alias: &NormalizedTransformation) -> Self {
        Self(self.into_iter().map(|c| c.transform(alias)).collect())
    }
}

impl Conjunction {
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
        let when = aliases.iter().fold(initial, Dnf::transform);
        Self {
            severity: rule.severity,
            message: rule.message.clone(),
            when,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        Assignment, AssignmentEffect, LeafEquals, LeafPresent, PredAll, PredNot, Predicate, Rule,
        Scope, Severity,
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
}
