use std::collections::BTreeMap;

use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::Lifetime;

use crate::{
    ast::{Conjunction, Dnf, Leaf, Scope, Severity},
    schema::{
        alias::NormalizedRule,
        grammar::{Scopes, SettingInfo, generate_conjunction, generate_dnf, generate_value_expr},
    },
};

pub fn generate_solve(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    rules: &[NormalizedRule],
    has_original: bool,
) -> anyhow::Result<TokenStream> {
    let error_rules: Vec<&NormalizedRule> = rules
        .iter()
        .filter(|r| r.severity == Severity::Error)
        .collect();
    let n_lit = Literal::usize_suffixed(error_rules.len());

    let self_acc = quote! { self };
    let state_acc = quote! { state };
    let original_acc = quote! { original };

    let original_param = if has_original {
        quote! { , original: &Self }
    } else {
        TokenStream::new()
    };
    let original_arg = if has_original {
        quote! { , original }
    } else {
        TokenStream::new()
    };
    let original_acc_opt: Option<&TokenStream> = if has_original {
        Some(&original_acc)
    } else {
        None
    };
    let seed_scopes = Scopes {
        current: &self_acc,
        original: original_acc_opt,
    };
    let break_scopes = Scopes {
        current: &state_acc,
        original: original_acc_opt,
    };

    let seeds = error_rules
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let i_lit = Literal::usize_suffixed(i);
            let pred = generate_dnf(settings, &r.when, seed_scopes)?;
            Ok(quote! { ok[#i_lit] = !( #pred ); })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let process: Vec<TokenStream> = error_rules
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let i_lit = Literal::usize_suffixed(i);
            let fn_name = format_ident!("try_repair_rule_{}", i);
            let msg = &r.message;
            quote! {
                if !ok[#i_lit] {
                    if !self.#fn_name(pin, &ok #original_arg) {
                        ::anyhow::bail!(#msg);
                    }
                    ok[#i_lit] = true;
                }
            }
        })
        .collect();

    let repair_fns = error_rules
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let fn_name = format_ident!("try_repair_rule_{}", i);
            let i_lit = Literal::usize_suffixed(i);
            let mut counter = 0usize;
            let walk = generate_dnf_walk(settings, &r.when, &mut counter, has_original)?;
            Ok(quote! {
                #[allow(unused_variables)]
                fn #fn_name(
                    &mut self,
                    pin: &::std::collections::HashSet<&'static str>,
                    ok: &[bool; #n_lit]
                    #original_param,
                ) -> bool {
                    let current: usize = #i_lit;
                    #walk
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let breakage_arms = error_rules
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let i_lit = Literal::usize_suffixed(i);
            let pred = generate_dnf(settings, &r.when, break_scopes)?;
            Ok(quote! {
                if ok[#i_lit] && current != #i_lit && ( #pred ) { return true; }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        #[allow(unused_assignments, unused_variables)]
        pub fn solve(
            &mut self,
            pin: &::std::collections::HashSet<&'static str>
            #original_param,
        ) -> ::anyhow::Result<()> {
            let mut ok: [bool; #n_lit] = [false; #n_lit];
            #( #seeds )*
            #( #process )*
            ::std::result::Result::Ok(())
        }

        #( #repair_fns )*

        #[allow(unused_variables)]
        fn re_fires_other_ok(
            state: &Self,
            ok: &[bool; #n_lit],
            current: usize
            #original_param,
        ) -> bool {
            #( #breakage_arms )*
            false
        }
    })
}

pub fn generate_pin_set(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    accessor: &TokenStream,
) -> TokenStream {
    let insertions = settings.values().map(|info| {
        let id = info.id;
        let ident = info.field_ident();
        quote! {
            if #accessor.#ident.is_some() {
                pin.insert(#id);
            }
        }
    });
    quote! {
        {
            let mut pin: ::std::collections::HashSet<&'static str> =
                ::std::collections::HashSet::new();
            #( #insertions )*
            pin
        }
    }
}

fn generate_dnf_walk(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    dnf: &Dnf,
    counter: &mut usize,
    has_original: bool,
) -> anyhow::Result<TokenStream> {
    if dnf.is_contradiction() {
        return Ok(quote! { true });
    }
    if dnf.is_tautology() {
        return Ok(quote! { false });
    }

    let depth = *counter;
    *counter += 1;
    let outer = walk_label(depth);
    let inner = inner_label(depth);

    let self_acc = quote! { self };
    let original_acc = quote! { original };
    let scopes = Scopes {
        current: &self_acc,
        original: has_original.then_some(&original_acc),
    };
    let dnf_eval = generate_dnf(settings, dnf, scopes)?;

    let steps = dnf
        .0
        .iter()
        .map(|conj| {
            let sub = generate_conjunction_walk(settings, conj, counter, has_original)?;
            Ok(quote! {
                {
                    let succ: bool = #sub;
                    if !succ { break #inner false; }
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        #outer: {
            if !( #dnf_eval ) { break #outer true; }
            let snap = self.clone();
            let all: bool = #inner: {
                #( #steps )*
                true
            };
            if !all { *self = snap; break #outer false; }
            break #outer true;
        }
    })
}

fn generate_conjunction_walk(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    conj: &Conjunction,
    counter: &mut usize,
    has_original: bool,
) -> anyhow::Result<TokenStream> {
    if conj.is_empty() {
        return Ok(quote! { false });
    }

    let depth = *counter;
    *counter += 1;
    let label = walk_label(depth);

    let self_acc = quote! { self };
    let original_acc = quote! { original };
    let scopes = Scopes {
        current: &self_acc,
        original: has_original.then_some(&original_acc),
    };
    let conj_eval = generate_conjunction(settings, conj, scopes)?;

    let attempts = conj
        .iter()
        .map(|leaf| generate_leaf_attempt(settings, leaf, &label, has_original))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        #label: {
            if !( #conj_eval ) { break #label true; }
            #( #attempts )*
            break #label false;
        }
    })
}

fn generate_leaf_attempt(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    leaf: &Leaf,
    parent: &Lifetime,
    has_original: bool,
) -> anyhow::Result<TokenStream> {
    if leaf.scope() == Scope::Original {
        return Ok(TokenStream::new());
    }
    let Some((info, mutation)) = leaf_flip(settings, leaf)? else {
        return Ok(TokenStream::new());
    };
    let field_id = info.id;
    let field_ident = info.field_ident();
    let original_arg = if has_original {
        quote! { , original }
    } else {
        TokenStream::new()
    };
    Ok(quote! {
        {
            if !pin.contains(#field_id) {
                let saved = self.#field_ident.take();
                #mutation
                if !Self::re_fires_other_ok(self, ok, current #original_arg) {
                    break #parent true;
                }
                self.#field_ident = saved;
            }
        }
    })
}

fn leaf_flip<'a>(
    settings: &'a BTreeMap<&str, SettingInfo<'a>>,
    leaf: &Leaf,
) -> anyhow::Result<Option<(&'a SettingInfo<'a>, TokenStream)>> {
    let r#ref = leaf.r#ref();
    let info = settings.get(r#ref).expect("ref validated");
    let ident = info.field_ident();

    let mutation = match leaf {
        Leaf::Equals(_)
        | Leaf::In(_)
        | Leaf::Between(_)
        | Leaf::LessThan(_)
        | Leaf::LessThanOrEqual(_)
        | Leaf::GreaterThan(_)
        | Leaf::GreaterThanOrEqual(_) => quote! { self.#ident = None; },
        Leaf::Present(p) if p.present => quote! { self.#ident = None; },
        Leaf::NotEquals(l) => {
            let value = generate_value_expr(info, &l.equals)?;
            quote! { self.#ident = Some(#value); }
        }
        Leaf::NotIn(l) => {
            let first = l.values.first().expect("non-empty `in` list");
            let value = generate_value_expr(info, first)?;
            quote! { self.#ident = Some(#value); }
        }
        Leaf::NotBetween(l) => {
            let value = generate_value_expr(info, &l.min)?;
            quote! { self.#ident = Some(#value); }
        }
        Leaf::Present(_)
        | Leaf::NotLessThan(_)
        | Leaf::NotLessThanOrEqual(_)
        | Leaf::NotGreaterThan(_)
        | Leaf::NotGreaterThanOrEqual(_) => return Ok(None),
    };

    Ok(Some((info, mutation)))
}

fn walk_label(depth: usize) -> Lifetime {
    Lifetime::new(&format!("'walk_{depth}"), Span::call_site())
}

fn inner_label(depth: usize) -> Lifetime {
    Lifetime::new(&format!("'conj_{depth}"), Span::call_site())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::ast::{LeafEquals, LeafPresent, Predicate, Scope};

    fn integer_info(id: &'static str) -> SettingInfo<'static> {
        SettingInfo {
            id,
            kind: crate::ast::SpecKind::Integer,
            option: None,
        }
    }

    fn nrule(when: Predicate) -> NormalizedRule {
        NormalizedRule {
            severity: Severity::Error,
            message: "bad".into(),
            when: when.into(),
        }
    }

    fn nrule_msg(when: Predicate, msg: &str) -> NormalizedRule {
        NormalizedRule {
            severity: Severity::Error,
            message: msg.into(),
            when: when.into(),
        }
    }

    #[test]
    fn empty_rule_set_emits_solve_that_does_nothing() {
        let settings = BTreeMap::new();
        let out = generate_solve(&settings, &[], false).unwrap().to_string();
        assert!(out.contains("fn solve"));
        assert!(out.contains("[bool ; 0usize]"));
    }

    #[test]
    fn equals_rule_emits_pin_check_save_restore_and_breakage_check() {
        let mut settings = BTreeMap::new();
        settings.insert("a", integer_info("a"));
        let rules = vec![nrule_msg(
            LeafEquals {
                r#ref: "a".into(),
                scope: Scope::Current,
                equals: json!(1),
            }
            .into(),
            "bad a",
        )];
        let out = generate_solve(&settings, &rules, false)
            .unwrap()
            .to_string();
        assert!(out.contains("try_repair_rule_0"));
        assert!(out.contains("pin . contains (\"a\")"));
        assert!(out.contains("re_fires_other_ok"));
        assert!(out.contains("self . a = None"));
        assert!(out.contains("self . a = saved"));
    }

    #[test]
    fn warning_severity_is_not_part_of_solve() {
        let mut settings = BTreeMap::new();
        settings.insert("a", integer_info("a"));
        let rules = vec![
            NormalizedRule {
                severity: Severity::Warning,
                message: "w".into(),
                when: Predicate::from(LeafEquals {
                    r#ref: "a".into(),
                    scope: Scope::Current,
                    equals: json!(1),
                })
                .into(),
            },
            NormalizedRule {
                severity: Severity::Info,
                message: "i".into(),
                when: Predicate::from(LeafEquals {
                    r#ref: "a".into(),
                    scope: Scope::Current,
                    equals: json!(2),
                })
                .into(),
            },
        ];
        let out = generate_solve(&settings, &rules, false)
            .unwrap()
            .to_string();
        assert!(!out.contains("try_repair_rule_0"));
        assert!(out.contains("[bool ; 0usize]"));
    }

    #[test]
    fn multi_clause_rule_emits_snapshot_for_conjunctive_dnf_walk() {
        let mut settings = BTreeMap::new();
        settings.insert("a", integer_info("a"));
        settings.insert("b", integer_info("b"));
        let rules = vec![nrule(
            crate::ast::PredAll {
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
        let out = generate_solve(&settings, &rules, false)
            .unwrap()
            .to_string();
        assert!(out.contains("pin . contains (\"a\")"));
        assert!(out.contains("pin . contains (\"b\")"));
        assert!(out.contains("let snap = self . clone ()"));
    }

    #[test]
    fn not_equals_walk_sets_field_to_the_named_value() {
        let mut settings = BTreeMap::new();
        settings.insert("a", integer_info("a"));
        let rules = vec![nrule(
            crate::ast::PredNot {
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
        )];
        let out = generate_solve(&settings, &rules, false)
            .unwrap()
            .to_string();
        assert!(out.contains("self . a = Some"));
    }

    #[test]
    fn original_scope_leaf_is_non_flippable_and_threads_original_param() {
        let mut settings = BTreeMap::new();
        let _ = settings.insert("a", integer_info("a"));
        let _ = settings.insert("b", integer_info("b"));
        let rules = vec![nrule(
            crate::ast::PredAll {
                all: vec![
                    LeafEquals {
                        r#ref: "a".into(),
                        scope: Scope::Original,
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
        let out = generate_solve(&settings, &rules, true).unwrap().to_string();
        assert!(out.contains("original : & Self"));
        assert!(out.contains("original . a"));
        assert!(!out.contains("self . a = None"));
        assert!(out.contains("self . b = None"));
    }

    #[test]
    fn solve_without_original_emits_no_original_param() {
        let mut settings = BTreeMap::new();
        let _ = settings.insert("a", integer_info("a"));
        let rules = vec![nrule(
            LeafEquals {
                r#ref: "a".into(),
                scope: Scope::Current,
                equals: json!(1),
            }
            .into(),
        )];
        let out = generate_solve(&settings, &rules, false)
            .unwrap()
            .to_string();
        assert!(!out.contains("original"));
    }

    #[test]
    fn breakage_check_carves_out_the_current_rule() {
        let mut settings = BTreeMap::new();
        settings.insert("a", integer_info("a"));
        let rules = vec![
            nrule_msg(
                LeafEquals {
                    r#ref: "a".into(),
                    scope: Scope::Current,
                    equals: json!(1),
                }
                .into(),
                "r0",
            ),
            nrule_msg(
                LeafEquals {
                    r#ref: "a".into(),
                    scope: Scope::Current,
                    equals: json!(2),
                }
                .into(),
                "r1",
            ),
        ];
        let out = generate_solve(&settings, &rules, false)
            .unwrap()
            .to_string();
        assert!(out.contains("current != 0usize"));
        assert!(out.contains("current != 1usize"));
    }
}
