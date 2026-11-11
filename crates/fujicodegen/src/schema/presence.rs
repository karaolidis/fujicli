use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ast::{Conjunction, Dnf, Leaf, LeafPresent, PredAll, Predicate, Scope, Severity},
    schema::alias::NormalizedRule,
};

#[derive(Debug, Default)]
pub struct PresenceDag {
    pub conditions: BTreeMap<String, Dnf>,
    pub edges: BTreeSet<(String, String)>,
}

impl PresenceDag {
    pub fn from_rules(rules: &[NormalizedRule]) -> Self {
        let mut contributions: BTreeMap<String, Vec<Dnf>> = BTreeMap::new();
        let mut edges: BTreeSet<(String, String)> = BTreeSet::new();

        rules
            .iter()
            .filter(|r| r.severity == Severity::Error)
            .flat_map(|r| r.gating.iter())
            .for_each(|conj| Self::process_disjunct(conj, &mut contributions, &mut edges));

        let conditions = contributions
            .into_iter()
            .map(|(target, gates)| {
                (
                    target,
                    Predicate::All(PredAll {
                        all: gates.into_iter().map(Predicate::from).collect(),
                    })
                    .into(),
                )
            })
            .collect();

        Self { conditions, edges }
    }

    fn process_disjunct(
        conj: &Conjunction,
        contributions: &mut BTreeMap<String, Vec<Dnf>>,
        edges: &mut BTreeSet<(String, String)>,
    ) {
        let Some(Gate {
            polarity,
            anchors,
            clauses,
            refs,
        }) = Gate::try_from_disjunct(conj)
        else {
            return;
        };

        edges.extend(
            refs.iter()
                .flat_map(|gref| anchors.iter().map(|anchor| (gref.clone(), anchor.clone()))),
        );

        // An anchor that must be absent while the clauses hold is read unless they definitely hold.
        let gate = if polarity {
            Dnf(clauses.into_iter().flat_map(Self::complement).collect()).simplified()
        } else {
            Dnf(vec![Conjunction(clauses)])
        };

        for anchor in anchors {
            contributions.entry(anchor).or_default().push(gate.clone());
        }
    }

    /// The classical complement of a leaf: a negated value test does not hold on an absent field.
    fn complement(leaf: Leaf) -> Vec<Conjunction> {
        let absent = match &leaf {
            Leaf::Present(_) => None,
            test => Some(Conjunction(vec![Leaf::Present(LeafPresent {
                r#ref: test.r#ref().to_owned(),
                scope: test.scope(),
                present: false,
            })])),
        };
        absent
            .into_iter()
            .chain([Conjunction(vec![leaf.negated()])])
            .collect()
    }
}

struct Gate {
    polarity: bool,
    anchors: BTreeSet<String>,
    clauses: Vec<Leaf>,
    refs: BTreeSet<String>,
}

impl Gate {
    fn try_from_disjunct(conj: &Conjunction) -> Option<Self> {
        let mut true_anchors: BTreeSet<String> = BTreeSet::new();
        let mut false_anchors: BTreeSet<String> = BTreeSet::new();
        let mut other_clauses: Vec<Leaf> = Vec::new();

        for lit in &conj.0 {
            if lit.scope() == Scope::Original {
                continue;
            }
            match lit {
                Leaf::Present(p) => {
                    if p.present {
                        true_anchors.insert(p.r#ref.clone());
                    } else {
                        false_anchors.insert(p.r#ref.clone());
                    }
                }
                other => other_clauses.push(other.clone()),
            }
        }

        let (polarity, anchors, clauses) = if !true_anchors.is_empty() {
            let extended: Vec<Leaf> = other_clauses
                .into_iter()
                .chain(false_anchors.iter().map(|r| {
                    Leaf::Present(LeafPresent {
                        r#ref: r.clone(),
                        scope: Scope::Current,
                        present: false,
                    })
                }))
                .collect();
            (true, true_anchors, extended)
        } else if !false_anchors.is_empty() {
            (false, false_anchors, other_clauses)
        } else {
            return None;
        };

        let refs: BTreeSet<String> = clauses.iter().map(|l| l.r#ref().to_string()).collect();

        if clauses.is_empty() || refs.is_empty() {
            return None;
        }
        debug_assert!(
            refs.is_disjoint(&anchors),
            "gate tests its own anchor; the conjunction was not simplified: {conj:?}"
        );

        Some(Self {
            polarity,
            anchors,
            clauses,
            refs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::{
            Assignment, AssignmentEffect, LeafEquals, LeafIn, LeafPresent, PredAll, PredAny,
            PredNot, Rule, Transformation,
        },
        schema::alias::NormalizedTransformation,
        util::dag::Dag,
    };
    use serde_json::json;

    fn rule(when: Predicate) -> Rule {
        Rule {
            severity: Severity::Error,
            message: "test".to_string(),
            when,
        }
    }

    fn rule_with_severity(severity: Severity, when: Predicate) -> Rule {
        Rule {
            severity,
            message: "test".to_string(),
            when,
        }
    }

    fn edge(from: &str, to: &str) -> (String, String) {
        (from.to_string(), to.to_string())
    }

    fn collect_raw(rules: &[Rule]) -> PresenceDag {
        let normalised: Vec<NormalizedRule> = rules
            .iter()
            .map(|r| NormalizedRule::from_rule(r, &[]))
            .collect();
        PresenceDag::from_rules(&normalised)
    }

    fn assert_gate_is_complement_of_single_value_leaf(gate: &Dnf) {
        assert_eq!(gate.0.len(), 2, "expected two disjuncts, got {gate:?}");
        assert!(gate.0.iter().all(|c| c.0.len() == 1), "got {gate:?}");
        let (absent, negated) = (&gate.0[0].0[0], &gate.0[1].0[0]);
        assert!(
            matches!(absent, Leaf::Present(LeafPresent { present: false, .. })),
            "expected an absence literal, got {absent:?}"
        );
        assert_eq!(absent.r#ref(), negated.r#ref());
        assert!(
            matches!(
                negated,
                Leaf::NotEquals(_)
                    | Leaf::NotIn(_)
                    | Leaf::NotBetween(_)
                    | Leaf::NotLessThan(_)
                    | Leaf::NotLessThanOrEqual(_)
                    | Leaf::NotGreaterThan(_)
                    | Leaf::NotGreaterThanOrEqual(_)
            ),
            "expected a negated value test, got {negated:?}"
        );
    }

    #[test]
    fn empty_rules_yield_empty_info() {
        let info = PresenceDag::from_rules(&[]);
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
    }

    #[test]
    fn present_true_rule_negates_other_clauses() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        let cond = info.conditions.get("A").expect("A condition");
        assert_gate_is_complement_of_single_value_leaf(cond);
        assert_eq!(info.edges, std::iter::once(edge("B", "A")).collect());
    }

    #[test]
    fn original_scope_leaf_is_dropped_from_presence_dag() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "A".into(),
                        scope: Scope::Original,
                        equals: json!("x"),
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
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
    }

    #[test]
    fn original_scope_leaf_does_not_gate_alongside_a_current_clause() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "A".into(),
                        scope: Scope::Original,
                        equals: json!("x"),
                    }
                    .into(),
                    LeafPresent {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "C".into(),
                        scope: Scope::Current,
                        equals: json!("y"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        let cond = info.conditions.get("B").expect("B condition");
        assert_gate_is_complement_of_single_value_leaf(cond);
        assert_eq!(info.edges, std::iter::once(edge("C", "B")).collect());
    }

    #[test]
    fn present_false_rule_does_not_negate() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: false,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        let cond = info.conditions.get("A").expect("A condition");
        assert_eq!(cond.0.len(), 1);
        assert_eq!(cond.0[0].0.len(), 1);
        assert!(matches!(&cond.0[0].0[0], Leaf::Equals(_)));
        assert_eq!(info.edges, std::iter::once(edge("B", "A")).collect());
    }

    #[test]
    fn non_error_severity_ignored() {
        for sev in [Severity::Warning, Severity::Info] {
            let rules = vec![rule_with_severity(
                sev,
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            equals: json!("x"),
                        }
                        .into(),
                    ],
                }
                .into(),
            )];
            assert!(collect_raw(&rules).conditions.is_empty());
        }
    }

    #[test]
    fn non_canonical_top_level_shapes_normalised_to_validation_when_anchor_or_gating_missing() {
        let rules = vec![rule(
            LeafPresent {
                r#ref: "A".into(),
                scope: Scope::Current,
                present: true,
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());

        let rules = vec![rule(
            PredNot {
                not: Box::new(
                    LeafEquals {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ),
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.is_empty());

        let rules = vec![rule(
            PredAny {
                any: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        equals: json!(1),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.is_empty());
    }

    #[test]
    fn top_level_any_of_disjoint_atomic_rules_splits_into_independent_anchors() {
        let rules = vec![rule(
            PredAny {
                any: vec![
                    PredAll {
                        all: vec![
                            LeafPresent {
                                r#ref: "A".into(),
                                scope: Scope::Current,
                                present: true,
                            }
                            .into(),
                            LeafEquals {
                                r#ref: "B".into(),
                                scope: Scope::Current,
                                equals: json!("x"),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                    PredAll {
                        all: vec![
                            LeafPresent {
                                r#ref: "C".into(),
                                scope: Scope::Current,
                                present: true,
                            }
                            .into(),
                            LeafEquals {
                                r#ref: "D".into(),
                                scope: Scope::Current,
                                equals: json!("y"),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.contains_key("A"));
        assert!(info.conditions.contains_key("C"));
        assert_eq!(
            info.edges,
            [edge("B", "A"), edge("D", "C")].into_iter().collect(),
        );
    }

    #[test]
    fn top_level_not_of_any_demorgans_into_anchor_rule() {
        let rules = vec![rule(
            PredNot {
                not: Box::new(
                    PredAny {
                        any: vec![
                            LeafPresent {
                                r#ref: "A".into(),
                                scope: Scope::Current,
                                present: false,
                            }
                            .into(),
                            PredNot {
                                not: Box::new(
                                    LeafEquals {
                                        r#ref: "B".into(),
                                        scope: Scope::Current,
                                        equals: json!("x"),
                                    }
                                    .into(),
                                ),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                ),
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.contains_key("A"));
        assert_eq!(info.edges, std::iter::once(edge("B", "A")).collect());
    }

    #[test]
    fn negated_presence_clause_flips_polarity_via_normalisation() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    PredNot {
                        not: Box::new(
                            LeafPresent {
                                r#ref: "A".into(),
                                scope: Scope::Current,
                                present: false,
                            }
                            .into(),
                        ),
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.contains_key("A"));
        assert_eq!(info.edges, std::iter::once(edge("B", "A")).collect());
    }

    #[test]
    fn tautological_predicate_allows_unconditional_rule() {
        let info = collect_raw(&[rule(Predicate::Bool(true))]);
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
        let info = collect_raw(&[rule(PredAll { all: vec![] }.into())]);
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
    }

    #[test]
    fn unsatisfiable_predicate_is_skipped() {
        let info = collect_raw(&[rule(Predicate::Bool(false))]);
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
        let info = collect_raw(&[rule(PredAny { any: vec![] }.into())]);
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
    }

    #[test]
    fn equivalent_forms_yield_same_edges_and_anchors() {
        let direct = vec![rule(
            PredAll {
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
                                equals: json!("x"),
                            }
                            .into(),
                            LeafEquals {
                                r#ref: "C".into(),
                                scope: Scope::Current,
                                equals: json!("y"),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let distributed = vec![rule(
            PredAny {
                any: vec![
                    PredAll {
                        all: vec![
                            LeafPresent {
                                r#ref: "A".into(),
                                scope: Scope::Current,
                                present: true,
                            }
                            .into(),
                            LeafEquals {
                                r#ref: "B".into(),
                                scope: Scope::Current,
                                equals: json!("x"),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                    PredAll {
                        all: vec![
                            LeafPresent {
                                r#ref: "A".into(),
                                scope: Scope::Current,
                                present: true,
                            }
                            .into(),
                            LeafEquals {
                                r#ref: "C".into(),
                                scope: Scope::Current,
                                equals: json!("y"),
                            }
                            .into(),
                        ],
                    }
                    .into(),
                ],
            }
            .into(),
        )];

        let info_d = collect_raw(&direct);
        let info_dd = collect_raw(&distributed);

        assert_eq!(info_d.edges, info_dd.edges);
        let keys_d: BTreeSet<_> = info_d.conditions.keys().cloned().collect();
        let keys_dd: BTreeSet<_> = info_dd.conditions.keys().cloned().collect();
        assert_eq!(keys_d, keys_dd);
    }

    #[test]
    fn rule_with_no_presence_clauses_is_validation_only() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        equals: json!("y"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
    }

    #[test]
    fn mixed_polarity_prefers_present_true_as_target() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafPresent {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        present: false,
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);

        assert!(info.conditions.contains_key("A"));
        assert!(!info.conditions.contains_key("B"));
        assert_eq!(info.edges, std::iter::once(edge("B", "A")).collect());

        // Polarity is `true` so the gate is the negation of
        // `Present(B, false)` which is `Present(B, true)`.
        let cond = &info.conditions["A"];
        assert_eq!(cond.0.len(), 1);
        assert_eq!(cond.0[0].0.len(), 1);
        assert!(matches!(&cond.0[0].0[0], Leaf::Present(p) if p.r#ref == "B" && p.present));
    }

    #[test]
    fn pure_present_false_used_as_anchor_when_no_present_true() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: false,
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
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.contains_key("B"));
        assert!(!info.conditions.contains_key("A"));
        assert_eq!(info.edges, std::iter::once(edge("A", "B")).collect());
    }

    #[test]
    fn no_present_true_yields_present_false_anchor() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: false,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.contains_key("A"));
        assert_eq!(info.edges, std::iter::once(edge("B", "A")).collect());
    }

    #[test]
    fn any_group_of_present_true_anchors_all_targets() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    PredAny {
                        any: vec![
                            LeafPresent {
                                r#ref: "A".into(),
                                scope: Scope::Current,
                                present: true,
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
                    LeafEquals {
                        r#ref: "C".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);

        assert!(info.conditions.contains_key("A"));
        assert!(info.conditions.contains_key("B"));
        assert_eq!(
            info.edges,
            [edge("C", "A"), edge("C", "B")].into_iter().collect()
        );
        assert_gate_is_complement_of_single_value_leaf(&info.conditions["A"]);
        assert_gate_is_complement_of_single_value_leaf(&info.conditions["B"]);
    }

    #[test]
    fn multiple_top_level_present_true_anchors() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafPresent {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "C".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.contains_key("A"));
        assert!(info.conditions.contains_key("B"));
        assert_eq!(
            info.edges,
            [edge("C", "A"), edge("C", "B")].into_iter().collect()
        );
    }

    #[test]
    fn multiple_rules_per_target_and_combine() {
        let rules = vec![
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            equals: json!("x"),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "C".into(),
                            scope: Scope::Current,
                            equals: json!("y"),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
        ];
        let info = collect_raw(&rules);

        // {B absent or B!=x} and {C absent or C!=y}, distributed.
        let cond = &info.conditions["A"];
        assert_eq!(cond.0.len(), 4, "expected four disjuncts, got {cond:?}");
        for conj in cond {
            let mut refs: Vec<&str> = conj.iter().map(Leaf::r#ref).collect();
            refs.sort_unstable();
            assert_eq!(refs, vec!["B", "C"]);
        }

        assert_eq!(
            info.edges,
            [edge("B", "A"), edge("C", "A")].into_iter().collect()
        );
    }

    #[test]
    fn single_rule_does_not_wrap_gate_in_all() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "B".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert_gate_is_complement_of_single_value_leaf(&info.conditions["A"]);
    }

    #[test]
    fn presence_beside_a_value_test_on_the_same_field_does_not_gate() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    LeafEquals {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        equals: json!("x"),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.is_empty(), "got: {:?}", info.conditions);
        assert!(info.edges.is_empty(), "got: {:?}", info.edges);
    }

    #[test]
    fn alias_exemption_self_reference_only_validates() {
        let alias = Transformation {
            when: Some(
                LeafEquals {
                    r#ref: "A".into(),
                    scope: Scope::Current,
                    equals: json!("ab"),
                }
                .into(),
            ),
            apply: vec![
                Assignment {
                    r#ref: "A".into(),
                    effect: AssignmentEffect::Set(json!("a")),
                },
                Assignment {
                    r#ref: "B".into(),
                    effect: AssignmentEffect::Set(json!("b")),
                },
            ],
        };
        let aliases: Vec<NormalizedTransformation> = Option::from(alias).into_iter().collect();
        let rules = [NormalizedRule::from_rule(
            &rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        PredNot {
                            not: Box::new(
                                LeafEquals {
                                    r#ref: "B".into(),
                                    scope: Scope::Current,
                                    equals: json!("off"),
                                }
                                .into(),
                            ),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            &aliases,
        )];

        let info = PresenceDag::from_rules(&rules);
        let b = |leaf: Leaf| Conjunction(vec![leaf]);
        let expected = Dnf(vec![
            b(Leaf::Present(LeafPresent {
                r#ref: "B".into(),
                scope: Scope::Current,
                present: false,
            })),
            b(Leaf::Equals(LeafEquals {
                r#ref: "B".into(),
                scope: Scope::Current,
                equals: json!("off"),
            })),
            b(Leaf::Equals(LeafEquals {
                r#ref: "B".into(),
                scope: Scope::Current,
                equals: json!("b"),
            })),
        ]);
        assert_eq!(info.conditions["A"], expected);
        assert_eq!(info.edges, std::iter::once(edge("B", "A")).collect());
    }

    #[test]
    fn alias_guards_on_value_rules_do_not_gate() {
        let alias = Transformation {
            when: Some(
                LeafEquals {
                    r#ref: "A".into(),
                    scope: Scope::Current,
                    equals: json!("ab"),
                }
                .into(),
            ),
            apply: vec![
                Assignment {
                    r#ref: "A".into(),
                    effect: AssignmentEffect::Set(json!("a")),
                },
                Assignment {
                    r#ref: "B".into(),
                    effect: AssignmentEffect::Set(json!("b")),
                },
            ],
        };
        let aliases: Vec<NormalizedTransformation> = Option::from(alias).into_iter().collect();
        let not_in = |r: &str| -> Predicate {
            PredNot {
                not: Box::new(
                    LeafIn {
                        r#ref: r.into(),
                        scope: Scope::Current,
                        values: vec![json!("x")],
                    }
                    .into(),
                ),
            }
            .into()
        };
        let rules = [
            NormalizedRule::from_rule(&rule(not_in("A")), &aliases),
            NormalizedRule::from_rule(&rule(not_in("B")), &aliases),
        ];

        let info = PresenceDag::from_rules(&rules);
        assert!(info.conditions.is_empty(), "got: {:?}", info.conditions);
        assert!(info.edges.is_empty(), "got: {:?}", info.edges);
    }

    #[test]
    fn presence_beside_a_nested_value_test_on_the_same_field_does_not_gate() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    PredNot {
                        not: Box::new(
                            PredAny {
                                any: vec![
                                    LeafEquals {
                                        r#ref: "A".into(),
                                        scope: Scope::Current,
                                        equals: json!("x"),
                                    }
                                    .into(),
                                    LeafEquals {
                                        r#ref: "B".into(),
                                        scope: Scope::Current,
                                        equals: json!("y"),
                                    }
                                    .into(),
                                ],
                            }
                            .into(),
                        ),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert!(info.conditions.is_empty(), "got: {:?}", info.conditions);
        assert!(info.edges.is_empty(), "got: {:?}", info.edges);
    }

    #[test]
    fn nested_gating_clause_pulls_all_referenced_fields_as_edges() {
        let rules = vec![rule(
            PredAll {
                all: vec![
                    LeafPresent {
                        r#ref: "A".into(),
                        scope: Scope::Current,
                        present: true,
                    }
                    .into(),
                    PredNot {
                        not: Box::new(
                            PredAny {
                                any: vec![
                                    LeafEquals {
                                        r#ref: "B".into(),
                                        scope: Scope::Current,
                                        equals: json!("x"),
                                    }
                                    .into(),
                                    LeafEquals {
                                        r#ref: "C".into(),
                                        scope: Scope::Current,
                                        equals: json!("y"),
                                    }
                                    .into(),
                                ],
                            }
                            .into(),
                        ),
                    }
                    .into(),
                ],
            }
            .into(),
        )];
        let info = collect_raw(&rules);
        assert_eq!(
            info.edges,
            [edge("B", "A"), edge("C", "A")].into_iter().collect()
        );
    }

    #[test]
    fn edges_dedup_across_rules() {
        let rules = vec![
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            equals: json!("x"),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            equals: json!("y"),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
        ];
        let info = collect_raw(&rules);
        assert_eq!(info.edges, std::iter::once(edge("B", "A")).collect());
    }

    #[test]
    fn synthesised_cycle_caught_by_dag_toposort() {
        let rules = vec![
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            equals: json!("x"),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            equals: json!("y"),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
        ];
        let info = collect_raw(&rules);
        assert_eq!(info.edges.len(), 2);

        let nodes = vec!["A", "B"];
        let edges: Vec<(&str, &str)> = info
            .edges
            .iter()
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .collect();
        let err = Dag::new(nodes, edges)
            .topological_order()
            .unwrap_err()
            .to_string();
        assert!(err.contains("ordering cycle"), "got: {err}");
    }

    #[test]
    fn linear_chain_of_dependencies_orders_correctly() {
        let rules = vec![
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "A".into(),
                            scope: Scope::Current,
                            equals: json!("x"),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "C".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        LeafEquals {
                            r#ref: "B".into(),
                            scope: Scope::Current,
                            equals: json!("y"),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
        ];
        let info = collect_raw(&rules);
        let nodes = vec!["C", "B", "A"];
        let edges: Vec<(&str, &str)> = info
            .edges
            .iter()
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .collect();
        let order = Dag::new(nodes, edges).topological_order().unwrap();
        let a_pos = order.iter().position(|n| *n == "A").unwrap();
        let b_pos = order.iter().position(|n| *n == "B").unwrap();
        let c_pos = order.iter().position(|n| *n == "C").unwrap();
        assert!(a_pos < b_pos, "A must precede B: {order:?}");
        assert!(b_pos < c_pos, "B must precede C: {order:?}");
    }

    #[test]
    fn full_rule_set_produces_expected_edges() {
        let rules = vec![
            rule(
                PredAll {
                    all: vec![
                        PredAny {
                            any: vec![
                                LeafPresent {
                                    r#ref: "monochromatic_color_temperature".into(),
                                    scope: Scope::Current,
                                    present: true,
                                }
                                .into(),
                                LeafPresent {
                                    r#ref: "monochromatic_color_tint".into(),
                                    scope: Scope::Current,
                                    present: true,
                                }
                                .into(),
                            ],
                        }
                        .into(),
                        PredNot {
                            not: Box::new(
                                LeafIn {
                                    r#ref: "film_simulation".into(),
                                    scope: Scope::Current,
                                    values: vec![json!("monochrome"), json!("acros")],
                                }
                                .into(),
                            ),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "white_balance_temperature".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        PredNot {
                            not: Box::new(
                                LeafEquals {
                                    r#ref: "white_balance".into(),
                                    scope: Scope::Current,
                                    equals: json!("temperature"),
                                }
                                .into(),
                            ),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            rule(
                PredAll {
                    all: vec![
                        LeafPresent {
                            r#ref: "dynamic_range".into(),
                            scope: Scope::Current,
                            present: true,
                        }
                        .into(),
                        PredNot {
                            not: Box::new(
                                LeafEquals {
                                    r#ref: "dynamic_range_priority".into(),
                                    scope: Scope::Current,
                                    equals: json!("off"),
                                }
                                .into(),
                            ),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
        ];
        let info = collect_raw(&rules);
        let expected: BTreeSet<(String, String)> = [
            edge("film_simulation", "monochromatic_color_temperature"),
            edge("film_simulation", "monochromatic_color_tint"),
            edge("white_balance", "white_balance_temperature"),
            edge("dynamic_range_priority", "dynamic_range"),
        ]
        .into_iter()
        .collect();
        assert_eq!(info.edges, expected);
    }
}
