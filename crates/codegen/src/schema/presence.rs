use std::collections::{BTreeMap, BTreeSet};

use anyhow::bail;

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
    pub fn try_from_rules(rules: &[NormalizedRule]) -> anyhow::Result<Self> {
        let mut contributions: BTreeMap<String, Vec<Dnf>> = BTreeMap::new();
        let mut edges: BTreeSet<(String, String)> = BTreeSet::new();

        rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r.severity == Severity::Error)
            .try_for_each(|(rule_idx, rule)| {
                rule.when.iter().try_for_each(|conj| {
                    Self::process_disjunct(rule_idx, conj, &mut contributions, &mut edges)
                })
            })?;

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

        Ok(Self { conditions, edges })
    }

    fn process_disjunct(
        rule_idx: usize,
        conj: &Conjunction,
        contributions: &mut BTreeMap<String, Vec<Dnf>>,
        edges: &mut BTreeSet<(String, String)>,
    ) -> anyhow::Result<()> {
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

        let (polarity, anchors, other_clauses) = if !true_anchors.is_empty() {
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
            return Ok(());
        };

        let gating_refs: BTreeSet<String> = other_clauses
            .iter()
            .map(|l| l.r#ref().to_string())
            .collect();

        if other_clauses.is_empty() || gating_refs.is_empty() {
            return Ok(());
        }

        for gref in &gating_refs {
            if anchors.contains(gref) {
                bail!(
                    "rule #{rule_idx}: gating clauses reference anchor target `{gref}`; \
                 deciding whether to read the target would require already knowing its value.",
                );
            }
        }

        edges.extend(
            gating_refs
                .iter()
                .flat_map(|gref| anchors.iter().map(|anchor| (gref.clone(), anchor.clone()))),
        );

        let gate = if polarity {
            Dnf(other_clauses
                .into_iter()
                .map(|l| Conjunction(vec![l.negated()]))
                .collect())
        } else {
            Dnf(vec![Conjunction(other_clauses)])
        };

        for anchor in anchors {
            contributions.entry(anchor).or_default().push(gate.clone());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::{LeafEquals, LeafIn, LeafPresent, PredAll, PredAny, PredNot, Rule},
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

    fn collect_raw(rules: &[Rule]) -> anyhow::Result<PresenceDag> {
        let normalised: Vec<NormalizedRule> = rules
            .iter()
            .map(|r| NormalizedRule::from_rule(r, &[]))
            .collect();
        PresenceDag::try_from_rules(&normalised)
    }

    fn assert_gate_is_single_negated_value_leaf(gate: &Dnf) {
        assert_eq!(gate.0.len(), 1, "expected one disjunct, got {gate:?}");
        assert_eq!(
            gate.0[0].0.len(),
            1,
            "expected one literal in the disjunct, got {gate:?}"
        );
        assert!(
            matches!(
                gate.0[0].0[0],
                Leaf::NotEquals(_)
                    | Leaf::NotIn(_)
                    | Leaf::NotBetween(_)
                    | Leaf::NotLessThan(_)
                    | Leaf::NotLessThanOrEqual(_)
                    | Leaf::NotGreaterThan(_)
                    | Leaf::NotGreaterThanOrEqual(_)
                    | Leaf::Present(LeafPresent { present: false, .. }),
            ),
            "expected a negated value-leaf literal, got {:?}",
            gate.0[0].0[0],
        );
    }

    #[test]
    fn empty_rules_yield_empty_info() {
        let info = PresenceDag::try_from_rules(&[]).unwrap();
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
        let info = collect_raw(&rules).unwrap();
        let cond = info.conditions.get("A").expect("A condition");
        assert_gate_is_single_negated_value_leaf(cond);
        assert_eq!(info.edges, [edge("B", "A")].into_iter().collect());
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();
        let cond = info.conditions.get("B").expect("B condition");
        assert_gate_is_single_negated_value_leaf(cond);
        assert_eq!(info.edges, [edge("C", "B")].into_iter().collect());
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
        let info = collect_raw(&rules).unwrap();
        let cond = info.conditions.get("A").expect("A condition");
        assert_eq!(cond.0.len(), 1);
        assert_eq!(cond.0[0].0.len(), 1);
        assert!(matches!(&cond.0[0].0[0], Leaf::Equals(_)));
        assert_eq!(info.edges, [edge("B", "A")].into_iter().collect());
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
            assert!(collect_raw(&rules).unwrap().conditions.is_empty());
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();
        assert!(info.conditions.contains_key("A"));
        assert_eq!(info.edges, [edge("B", "A")].into_iter().collect());
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
        let info = collect_raw(&rules).unwrap();
        assert!(info.conditions.contains_key("A"));
        assert_eq!(info.edges, [edge("B", "A")].into_iter().collect());
    }

    #[test]
    fn tautological_predicate_allows_unconditional_rule() {
        let info = collect_raw(&[rule(Predicate::Bool(true))]).unwrap();
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
        let info = collect_raw(&[rule(PredAll { all: vec![] }.into())]).unwrap();
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
    }

    #[test]
    fn unsatisfiable_predicate_is_skipped() {
        let info = collect_raw(&[rule(Predicate::Bool(false))]).unwrap();
        assert!(info.conditions.is_empty());
        assert!(info.edges.is_empty());
        let info = collect_raw(&[rule(PredAny { any: vec![] }.into())]).unwrap();
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

        let info_d = collect_raw(&direct).unwrap();
        let info_dd = collect_raw(&distributed).unwrap();

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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();

        assert!(info.conditions.contains_key("A"));
        assert!(!info.conditions.contains_key("B"));
        assert_eq!(info.edges, [edge("B", "A")].into_iter().collect());

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
        let info = collect_raw(&rules).unwrap();
        assert!(info.conditions.contains_key("B"));
        assert!(!info.conditions.contains_key("A"));
        assert_eq!(info.edges, [edge("A", "B")].into_iter().collect());
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
        let info = collect_raw(&rules).unwrap();
        assert!(info.conditions.contains_key("A"));
        assert_eq!(info.edges, [edge("B", "A")].into_iter().collect());
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
        let info = collect_raw(&rules).unwrap();

        assert!(info.conditions.contains_key("A"));
        assert!(info.conditions.contains_key("B"));
        assert_eq!(
            info.edges,
            [edge("C", "A"), edge("C", "B")].into_iter().collect()
        );
        assert_gate_is_single_negated_value_leaf(&info.conditions["A"]);
        assert_gate_is_single_negated_value_leaf(&info.conditions["B"]);
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();

        // Per-rule gates were {!B=x} and {!C=y}; All-combined gives a
        // single conjunction over both negated literals.
        let cond = &info.conditions["A"];
        assert_eq!(cond.0.len(), 1, "expected one disjunct, got {cond:?}");
        assert_eq!(cond.0[0].0.len(), 2);
        let mut refs: Vec<&str> = cond.0[0].0.iter().map(|l| l.r#ref()).collect();
        refs.sort();
        assert_eq!(refs, vec!["B", "C"]);
        assert!(cond.0[0].0.iter().all(|l| matches!(l, Leaf::NotEquals(_))));

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
        let info = collect_raw(&rules).unwrap();
        assert_gate_is_single_negated_value_leaf(&info.conditions["A"]);
    }

    #[test]
    fn self_referential_gating_rejected() {
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
        let err = collect_raw(&rules).unwrap_err().to_string();
        assert!(err.contains("anchor target"), "got: {err}");
    }

    #[test]
    fn self_reference_via_nested_clause_also_rejected() {
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
        let err = collect_raw(&rules).unwrap_err().to_string();
        assert!(err.contains("anchor target"), "got: {err}");
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();
        assert_eq!(info.edges, [edge("B", "A")].into_iter().collect());
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();
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
        let info = collect_raw(&rules).unwrap();
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
