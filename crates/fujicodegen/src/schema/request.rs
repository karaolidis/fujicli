use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, ensure};
use proc_macro2::TokenStream;
use quote::quote;
use serde_json::Value;

use crate::{
    ast::{Leaf, Request, RequestThen, Scope, Severity},
    schema::{
        grammar::{SettingInfo, generate_value_expr},
        rule::NormalizedRule,
    },
};

pub fn check_requests(requests: &[Request], rules: &[NormalizedRule]) -> anyhow::Result<()> {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for request in requests {
        ensure!(
            seen.insert((&request.r#ref, &request.value)),
            "`{} = {}` has more than one request",
            request.r#ref,
            request.value
        );
    }
    for request in requests {
        if let RequestThen::Becomes(target) = &request.then {
            ensure!(
                !seen.contains(&(request.r#ref.as_str(), target.as_str())),
                "`{} = {}` becomes `{target}`, which is itself a request",
                request.r#ref,
                request.value
            );
        }
    }

    for rule in rules.iter().filter(|r| r.severity == Severity::Error) {
        for leaf in rule.when.iter().flat_map(|c| c.iter()) {
            if leaf.scope() != Scope::Current {
                continue;
            }
            let target = match leaf {
                Leaf::NotEquals(l) => Some(&l.equals),
                Leaf::NotIn(l) => l.values.first(),
                _ => None,
            };
            if let Some(Value::String(value)) = target
                && seen.contains(&(leaf.r#ref(), value.as_str()))
            {
                bail!(
                    "rule {:?} repairs `{}` to `{value}`, which is a request, not a value",
                    rule.message,
                    leaf.r#ref()
                );
            }
        }
    }
    Ok(())
}

pub fn generate_requests(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    requests: &[Request],
    buf_ty: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let rewrite = |request: &Request, to: TokenStream| -> anyhow::Result<TokenStream> {
        let info = &settings[request.r#ref.as_str()];
        let ident = info.field_ident();
        let from = generate_value_expr(info, &Value::String(request.value.clone()))?;
        Ok(quote! {
            if buf.#ident == Some(#from) {
                buf.#ident = #to;
            }
        })
    };

    let keeps = requests
        .iter()
        .filter(|r| r.then == RequestThen::Keep)
        .map(|r| rewrite(r, quote! { None }))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let becomes = requests
        .iter()
        .filter_map(|r| match &r.then {
            RequestThen::Becomes(target) => Some((r, target)),
            RequestThen::Keep => None,
        })
        .map(|(r, target)| {
            let info = &settings[r.r#ref.as_str()];
            let to = generate_value_expr(info, &Value::String(target.clone()))?;
            rewrite(r, quote! { Some(#to) })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let function = |name: TokenStream, body: Vec<TokenStream>| {
        if body.is_empty() {
            quote! {
                #[allow(clippy::needless_pass_by_ref_mut)]
                pub const fn #name(_buf: &mut #buf_ty) {}
            }
        } else {
            quote! {
                pub fn #name(buf: &mut #buf_ty) {
                    #( #body )*
                }
            }
        }
    };
    let drop_keeps = function(quote! { drop_keeps }, keeps);
    let apply_becomes = function(quote! { apply_becomes }, becomes);
    Ok(quote! {
        #drop_keeps
        #apply_becomes
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ast::{FujiOption, LeafIn, PredNot, Predicate, SpecKind};

    fn request(value: &str, then: RequestThen) -> Request {
        Request {
            r#ref: "dr".into(),
            value: value.into(),
            then,
        }
    }

    fn pin(values: &[&str]) -> NormalizedRule {
        let when: Predicate = PredNot {
            not: Box::new(
                LeafIn {
                    r#ref: "dr".into(),
                    scope: Scope::Current,
                    values: values.iter().map(|v| json!(v)).collect(),
                }
                .into(),
            ),
        }
        .into();
        NormalizedRule::new(Severity::Error, "pin", when.into())
    }

    #[test]
    fn keeps_clear_the_value_and_becomes_rewrite_it() {
        let option: FujiOption = serde_json::from_value(json!({
            "id": "dr",
            "spec": {
                "name": "DR", "kind": "enum",
                "rules": { "variants": [{"id": "auto", "name": "Auto"}, {"id": "x", "name": "X"}, {"id": "y", "name": "Y"}] },
                "encoding": { "kind": "lookup", "spec": { "values": {"auto": 0, "x": 1, "y": 2} } }
            }
        }))
        .unwrap();
        let settings = BTreeMap::from([(
            "dr",
            SettingInfo {
                id: "dr",
                kind: SpecKind::Enum,
                option: Some(&option),
                is_copy: true,
            },
        )]);
        let requests = [
            request("auto", RequestThen::Keep),
            request("x", RequestThen::Becomes("y".into())),
        ];
        let out = generate_requests(&settings, &requests, &quote! { Buf })
            .unwrap()
            .to_string();
        assert!(
            out.contains("pub fn drop_keeps (buf : & mut Buf) { if buf . dr == Some (crate :: generated :: options :: Dr :: Auto) { buf . dr = None ; } }"),
            "{out}"
        );
        assert!(
            out.contains("buf . dr = Some (crate :: generated :: options :: Dr :: Y)"),
            "{out}"
        );
        assert!(
            generate_requests(&settings, &[], &quote! { Buf })
                .unwrap()
                .to_string()
                .contains("pub const fn drop_keeps (_buf : & mut Buf) { }")
        );
    }

    #[test]
    fn rejects_duplicate_and_chained_requests() {
        let keep = request("auto", RequestThen::Keep);
        assert!(check_requests(&[keep.clone(), keep.clone()], &[]).is_err());
        let chained = request("x", RequestThen::Becomes("auto".into()));
        assert!(check_requests(&[keep, chained], &[]).is_err());
    }

    #[test]
    fn rejects_a_rule_that_repairs_to_a_request() {
        let keep = request("auto", RequestThen::Keep);
        assert!(check_requests(std::slice::from_ref(&keep), &[pin(&["auto", "hdr100"])]).is_err());
        assert!(check_requests(&[keep], &[pin(&["hdr100", "auto"])]).is_ok());
    }
}
