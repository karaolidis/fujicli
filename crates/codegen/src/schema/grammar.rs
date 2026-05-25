use std::collections::BTreeMap;

use anyhow::bail;
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
use serde_json::Value;

use crate::{
    ast::{
        Assignment, AssignmentEffect, Conjunction, Dnf, Field, FujiOption, Leaf, Predicate,
        Setting, Severity, SpecKind, Transformation,
    },
    common::options,
    schema::alias::NormalizedRule,
    util::ident::safe_upper_camel_case_ident,
};

pub trait OptionLike {
    fn id(&self) -> &str;
    fn option_ref(&self) -> Option<&str>;
}

impl OptionLike for Setting {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn option_ref(&self) -> Option<&str> {
        Some(&self.r#ref)
    }
}

impl OptionLike for Field {
    fn id(&self) -> &str {
        Field::id(self)
    }

    fn option_ref(&self) -> Option<&str> {
        match self {
            Field::Ref(r) => Some(&r.r#ref),
            Field::Inline(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CompareOp {
    Eq,
    Lt,
    Lte,
    Gt,
    Gte,
}

impl CompareOp {
    fn tokens(self) -> TokenStream {
        match self {
            Self::Eq => quote! { == },
            Self::Lt => quote! { < },
            Self::Lte => quote! { <= },
            Self::Gt => quote! { > },
            Self::Gte => quote! { >= },
        }
    }
}

#[derive(Clone)]
pub struct SettingInfo<'a> {
    pub id: &'a str,
    pub kind: SpecKind,
    pub option: Option<&'a FujiOption>,
}

impl<'a> SettingInfo<'a> {
    pub fn field_ident(&self) -> Ident {
        Ident::new(self.id, proc_macro2::Span::call_site())
    }

    pub fn type_path(&self) -> TokenStream {
        match self.option {
            Some(option) => {
                let options_path = options::path();
                let type_ident = safe_upper_camel_case_ident(&option.id);
                quote! { #options_path::#type_ident }
            }
            None => {
                debug_assert!(matches!(self.kind, SpecKind::Integer));
                quote! { i32 }
            }
        }
    }

    fn int(&self, accessor: &TokenStream) -> TokenStream {
        if self.option.is_some() {
            quote! { i32::from(#accessor) }
        } else {
            quote! { #accessor }
        }
    }
}

pub fn build_settings<'a, F: OptionLike + 'a>(
    options: &'a BTreeMap<String, FujiOption>,
    items: &'a [F],
) -> anyhow::Result<BTreeMap<&'a str, SettingInfo<'a>>> {
    let mut table = BTreeMap::new();
    for item in items {
        let info = if let Some(name) = item.option_ref() {
            let option = options.get(name).expect("ref validated during cue export");
            SettingInfo {
                id: item.id(),
                kind: option.spec.kind(),
                option: Some(option),
            }
        } else {
            SettingInfo {
                id: item.id(),
                kind: SpecKind::Integer,
                option: None,
            }
        };
        let _ = table.insert(info.id, info);
    }
    Ok(table)
}

pub fn generate_predicate(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    pred: &Predicate,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    Ok(match pred {
        Predicate::Bool(b) => {
            if *b {
                quote! { true }
            } else {
                quote! { false }
            }
        }
        Predicate::All(p) => {
            if p.all.is_empty() {
                quote! { true }
            } else if p.all.len() == 1 {
                generate_predicate(settings, &p.all[0], accessor)?
            } else {
                let parts = p
                    .all
                    .iter()
                    .map(|c| generate_predicate(settings, c, accessor))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                quote! { #( ( #parts ) )&&* }
            }
        }
        Predicate::Any(p) => {
            if p.any.is_empty() {
                quote! { false }
            } else if p.any.len() == 1 {
                generate_predicate(settings, &p.any[0], accessor)?
            } else {
                let parts = p
                    .any
                    .iter()
                    .map(|c| generate_predicate(settings, c, accessor))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                quote! { #( ( #parts ) )||* }
            }
        }
        Predicate::Not(p) => {
            let inner = generate_predicate(settings, &p.not, accessor)?;
            quote! { !( #inner ) }
        }
        Predicate::Present(p) => {
            let info = settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export");
            let field = info.field_ident();
            if p.present {
                quote! { #accessor.#field.is_some() }
            } else {
                quote! { #accessor.#field.is_none() }
            }
        }
        Predicate::Equals(p) => {
            let info = settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export");
            generate_compare(info, accessor, &p.equals, CompareOp::Eq)?
        }
        Predicate::In(p) => {
            let info = settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export");
            let field = info.field_ident();
            let arms = p
                .values
                .iter()
                .map(|v| generate_value_expr(info, v))
                .collect::<anyhow::Result<Vec<_>>>()?;
            match info.kind {
                SpecKind::Enum => {
                    quote! { #accessor.#field.is_some_and(|v| matches!(v, #( #arms )|*)) }
                }
                SpecKind::Integer => {
                    let v_expr = info.int(&quote! { v });
                    quote! { #accessor.#field.is_some_and(|v| matches!(#v_expr, #( #arms )|*)) }
                }
                SpecKind::Float => {
                    quote! {
                        #accessor.#field.is_some_and(|v| {
                            let v = f32::from(v);
                            #( (v - (#arms)).abs() < f32::EPSILON )||*
                        })
                    }
                }
                SpecKind::String => {
                    quote! { #accessor.#field.as_deref().is_some_and(|v| matches!(v, #( #arms )|*)) }
                }
            }
        }
        Predicate::Between(p) => {
            let info = settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export");
            let field = info.field_ident();
            let lo = generate_value_expr(info, &p.min)?;
            let hi = generate_value_expr(info, &p.max)?;
            match info.kind {
                SpecKind::Integer => {
                    let v_expr = info.int(&quote! { v });
                    quote! { #accessor.#field.is_some_and(|v| ( (#lo) ..= (#hi) ).contains(&#v_expr)) }
                }
                SpecKind::Float => {
                    quote! { #accessor.#field.is_some_and(|v| ( (#lo) ..= (#hi) ).contains(&f32::from(v))) }
                }
                other => bail!(
                    "`between` predicate on non-numeric setting `{}` ({other:?})",
                    p.r#ref
                ),
            }
        }
        Predicate::LessThan(p) => generate_compare(
            settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export"),
            accessor,
            &p.lt,
            CompareOp::Lt,
        )?,
        Predicate::LessThanOrEqual(p) => generate_compare(
            settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export"),
            accessor,
            &p.lte,
            CompareOp::Lte,
        )?,
        Predicate::GreaterThan(p) => generate_compare(
            settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export"),
            accessor,
            &p.gt,
            CompareOp::Gt,
        )?,
        Predicate::GreaterThanOrEqual(p) => generate_compare(
            settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export"),
            accessor,
            &p.gte,
            CompareOp::Gte,
        )?,
    })
}

pub fn generate_assignment(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    assignment: &Assignment,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let info = settings
        .get(assignment.r#ref.as_str())
        .expect("ref validated during cue export");
    let field = info.field_ident();
    Ok(match &assignment.effect {
        AssignmentEffect::Set(value) => {
            let value = generate_value_expr(info, value)?;
            quote! { #accessor.#field = Some(#value); }
        }
        AssignmentEffect::Clear => quote! { #accessor.#field = None; },
    })
}

pub fn generate_transformation(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    transformation: &Transformation,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let assignments = transformation
        .apply
        .iter()
        .map(|a| generate_assignment(settings, a, accessor))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let body = quote! { #( #assignments )* };
    Ok(match &transformation.when {
        Some(pred) => {
            let cond = generate_predicate(settings, pred, accessor)?;
            quote! { if #cond { #body } }
        }
        None => body,
    })
}

pub fn generate_apply_transformations(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    transformations: &[Transformation],
) -> anyhow::Result<TokenStream> {
    let accessor = quote! { self };
    let parts = transformations
        .iter()
        .map(|t| generate_transformation(settings, t, &accessor))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(quote! {
        fn apply_transformations(&mut self) {
            #( #parts )*
        }
    })
}

pub fn generate_emit_warnings_and_infos(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    rules: &[NormalizedRule],
) -> anyhow::Result<TokenStream> {
    let accessor = quote! { self };
    let parts = rules
        .iter()
        .filter(|r| !matches!(r.severity, Severity::Error))
        .map(|r| generate_normalized_rule(settings, r, &accessor))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(quote! {
        fn emit_warnings_and_infos(&self) -> ::anyhow::Result<()> {
            #( #parts )*
            Ok(())
        }
    })
}

pub fn generate_normalized_rule(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    rule: &NormalizedRule,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let cond = generate_dnf(settings, &rule.when, accessor)?;
    Ok({
        let severity = rule.severity;
        let message: &str = &rule.message;
        let action = match severity {
            Severity::Error => quote! { ::anyhow::bail!(#message); },
            Severity::Warning => quote! { ::log::warn!(#message); },
            Severity::Info => quote! { ::log::info!(#message); },
        };
        quote! { if #cond { #action } }
    })
}

pub fn generate_dnf(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    dnf: &Dnf,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    if dnf.is_contradiction() {
        return Ok(quote! { false });
    }
    if dnf.is_tautology() {
        return Ok(quote! { true });
    }
    let parts = dnf
        .iter()
        .map(|c| generate_conjunction(settings, c, accessor))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(if parts.len() == 1 {
        parts
            .into_iter()
            .next()
            .expect("parts has exactly one element")
    } else {
        quote! { #( ( #parts ) )||* }
    })
}

pub fn generate_conjunction(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    conj: &Conjunction,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    if conj.0.is_empty() {
        return Ok(quote! { true });
    }
    let parts = conj
        .iter()
        .map(|l| generate_leaf(settings, l, accessor))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(if parts.len() == 1 {
        parts
            .into_iter()
            .next()
            .expect("parts has exactly one element")
    } else {
        quote! { #( ( #parts ) )&&* }
    })
}

pub fn generate_leaf(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    leaf: &Leaf,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let pred: Predicate = leaf.clone().into();
    generate_predicate(settings, &pred, accessor)
}

pub fn generate_value_expr(info: &SettingInfo<'_>, value: &Value) -> anyhow::Result<TokenStream> {
    match info.kind {
        SpecKind::Integer => {
            let n = value
                .as_i64()
                .expect("integer predicate value validated during cue export");
            let lit = Literal::i32_suffixed(i32::try_from(n)?);
            Ok(quote! { #lit })
        }
        SpecKind::Float => {
            let n = value
                .as_f64()
                .expect("float predicate value validated during cue export");
            let lit = Literal::f32_suffixed(n as f32);
            Ok(quote! { #lit })
        }
        SpecKind::String => {
            let s = value
                .as_str()
                .expect("string predicate value validated during cue export");
            Ok(quote! { #s })
        }
        SpecKind::Enum => {
            let s = value
                .as_str()
                .expect("enum predicate value validated during cue export");
            let path = info.type_path();
            let variant = safe_upper_camel_case_ident(s);
            Ok(quote! { #path::#variant })
        }
    }
}

fn generate_compare(
    info: &SettingInfo<'_>,
    accessor: &TokenStream,
    value: &Value,
    op: CompareOp,
) -> anyhow::Result<TokenStream> {
    let field = info.field_ident();
    let value_expr = generate_value_expr(info, value)?;

    Ok(match info.kind {
        SpecKind::Enum => match op {
            CompareOp::Eq => quote! { #accessor.#field.is_some_and(|v| v == #value_expr) },
            _ => bail!(
                "ordered comparison ({op:?}) on enum setting `{}` is not supported",
                info.id
            ),
        },
        SpecKind::Integer => {
            let op_tok = op.tokens();
            let v_expr = info.int(&quote! { v });
            quote! { #accessor.#field.is_some_and(|v| #v_expr #op_tok (#value_expr)) }
        }
        SpecKind::Float => {
            if matches!(op, CompareOp::Eq) {
                quote! {
                    #accessor.#field.is_some_and(|v| (f32::from(v) - (#value_expr)).abs() < f32::EPSILON)
                }
            } else {
                let op_tok = op.tokens();
                quote! { #accessor.#field.is_some_and(|v| f32::from(v) #op_tok (#value_expr)) }
            }
        }
        SpecKind::String => match op {
            CompareOp::Eq => {
                quote! { #accessor.#field.as_deref().is_some_and(|v| v == #value_expr) }
            }
            _ => bail!(
                "ordered comparison ({op:?}) on string setting `{}` is not supported",
                info.id
            ),
        },
    })
}
