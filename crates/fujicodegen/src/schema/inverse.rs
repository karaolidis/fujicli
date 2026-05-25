use std::{collections::BTreeMap, ptr::eq};

use anyhow::bail;
use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{Assignment, AssignmentEffect, PredAll, Predicate, Scope, Transformation},
    schema::grammar::{Scopes, SettingInfo, generate_assignment, generate_predicate},
};

impl Transformation {
    pub fn is_invertible(&self, all: &[Self]) -> bool {
        let Some(Predicate::Equals(when_leaf)) = self.when.as_ref() else {
            return false;
        };
        if when_leaf.scope != Scope::Current {
            return false;
        }

        let Some(my_setters) = self.setters() else {
            return false;
        };

        for other in all {
            if eq(other, self) {
                continue;
            }
            let Some(other_setters) = other.setters() else {
                continue;
            };
            if pattern_matches(&my_setters, &other_setters) {
                return false;
            }
        }

        true
    }

    fn setters(&self) -> Option<Vec<(&str, &serde_json::Value)>> {
        self.apply
            .iter()
            .map(|a| match &a.effect {
                AssignmentEffect::Set(v) => Some((a.r#ref.as_str(), v)),
                AssignmentEffect::Clear => None,
            })
            .collect()
    }
}

pub fn generate_inverses(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    transformations: &[Transformation],
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let mut blocks: Vec<TokenStream> = Vec::new();
    for t in transformations.iter().rev() {
        if !t.is_invertible(transformations) {
            println!(
                "cargo:warning=skipping inverse for non-invertible transformation with when={:?} and apply={:?}.",
                t.when, t.apply
            );
            continue;
        }
        blocks.push(generate_one_inverse(settings, t, accessor)?);
    }
    Ok(quote! { #( #blocks )* })
}

fn generate_one_inverse(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    t: &Transformation,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let set_leaves: Vec<Predicate> = t
        .apply
        .iter()
        .filter(|a| matches!(a.effect, AssignmentEffect::Set(_)))
        .map(Predicate::from)
        .collect();
    let match_pred = if set_leaves.len() == 1 {
        set_leaves
            .into_iter()
            .next()
            .expect("set_leaves has exactly one element")
    } else {
        PredAll { all: set_leaves }.into()
    };
    let condition = generate_predicate(settings, &match_pred, Scopes::new(accessor))?;

    let Some(Predicate::Equals(when_leaf)) = t.when.as_ref() else {
        bail!("generate_one_inverse called on non-alias transformation");
    };
    let when_assignment = Assignment {
        r#ref: when_leaf.r#ref.clone(),
        effect: AssignmentEffect::Set(when_leaf.equals.clone()),
    };
    let when_apply = generate_assignment(settings, &when_assignment, accessor)?;

    let mut clears = TokenStream::new();
    for a in &t.apply {
        if a.r#ref == when_leaf.r#ref {
            continue;
        }
        let info = settings.get(a.r#ref.as_str()).expect("ctx validated field");
        let ident = info.field_ident();
        clears.extend(quote! { #accessor.#ident = ::std::option::Option::None; });
    }

    Ok(quote! {
        if #condition {
            #when_apply
            #clears
        }
    })
}

fn pattern_matches(a: &[(&str, &serde_json::Value)], b: &[(&str, &serde_json::Value)]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .all(|(field, value)| b.iter().any(|(of, ov)| of == field && ov == value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Assignment, AssignmentEffect, LeafEquals, Scope};
    use serde_json::json;

    fn alias_t(
        trigger_field: &str,
        trigger_v: serde_json::Value,
        apply: Vec<(&str, serde_json::Value)>,
    ) -> Transformation {
        Transformation {
            when: Some(
                LeafEquals {
                    r#ref: trigger_field.into(),
                    scope: Scope::Current,
                    equals: trigger_v,
                }
                .into(),
            ),
            apply: apply
                .into_iter()
                .map(|(r, v)| Assignment {
                    r#ref: r.into(),
                    effect: AssignmentEffect::Set(v),
                })
                .collect(),
        }
    }

    #[test]
    fn multi_field_alias_is_invertible() {
        let t = alias_t(
            "dr",
            json!("hdr800_plus"),
            vec![("dr", json!("hdr800")), ("drp", json!("plus"))],
        );
        let all = vec![t];
        assert!(all[0].is_invertible(&all));
    }

    #[test]
    fn duplicate_pattern_blocks_invertibility() {
        let a = alias_t("x", json!("alpha"), vec![("p", json!(1)), ("q", json!(2))]);
        let b = alias_t("x", json!("beta"), vec![("p", json!(1)), ("q", json!(2))]);
        let all = vec![a, b];
        assert!(!all[0].is_invertible(&all));
        assert!(!all[1].is_invertible(&all));
    }

    #[test]
    fn original_scope_when_blocks_invertibility() {
        let t = Transformation {
            when: Some(
                LeafEquals {
                    r#ref: "dr".into(),
                    scope: Scope::Original,
                    equals: json!("hdr800_plus"),
                }
                .into(),
            ),
            apply: vec![Assignment {
                r#ref: "dr".into(),
                effect: AssignmentEffect::Set(json!("hdr800")),
            }],
        };
        let all = vec![t];
        assert!(!all[0].is_invertible(&all));
    }

    #[test]
    fn unconditional_transformation_not_invertible() {
        let t = Transformation {
            when: None,
            apply: vec![Assignment {
                r#ref: "head_0".into(),
                effect: AssignmentEffect::Set(json!(0)),
            }],
        };
        let all = vec![t];
        assert!(!all[0].is_invertible(&all));
    }
}
