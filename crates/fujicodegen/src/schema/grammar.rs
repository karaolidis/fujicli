use std::collections::BTreeMap;

use anyhow::bail;
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
use serde_json::Value;

use crate::{
    ast::{
        Assignment, AssignmentEffect, Conjunction, Dnf, Field, FujiOption, Leaf, Predicate, Scope,
        Setting, Severity, SpecKind, Transformation,
    },
    common::options,
    schema::alias::NormalizedRule,
    util::ident::safe_upper_camel_case_ident,
};

#[derive(Clone, Copy)]
pub struct Scopes<'a> {
    pub current: &'a TokenStream,
    pub original: Option<&'a TokenStream>,
}

impl<'a> Scopes<'a> {
    pub const fn new(accessor: &'a TokenStream) -> Self {
        Self {
            current: accessor,
            original: None,
        }
    }

    pub const fn with_original(current: &'a TokenStream, original: &'a TokenStream) -> Self {
        Self {
            current,
            original: Some(original),
        }
    }

    const fn pick(&self, scope: Scope) -> &'a TokenStream {
        match scope {
            Scope::Current => self.current,
            Scope::Original => self
                .original
                .expect("original-scope leaf reached a context with no original accessor"),
        }
    }
}

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
        Self::id(self)
    }

    fn option_ref(&self) -> Option<&str> {
        match self {
            Self::Ref(r) => Some(&r.r#ref),
            Self::Inline(_) => None,
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
    pub is_copy: bool,
}

impl SettingInfo<'_> {
    pub fn field_ident(&self) -> Ident {
        Ident::new(self.id, proc_macro2::Span::call_site())
    }

    pub fn type_path(&self) -> TokenStream {
        self.option.map_or_else(
            || {
                debug_assert!(matches!(self.kind, SpecKind::Integer));
                quote! { i32 }
            },
            |option| {
                let options_path = options::path();
                let type_ident = safe_upper_camel_case_ident(&option.id);
                quote! { #options_path::#type_ident }
            },
        )
    }

    pub fn discriminant_expr(&self) -> TokenStream {
        let options_path = options::path();
        let variant = self.option.map_or_else(
            || safe_upper_camel_case_ident(self.id),
            |option| safe_upper_camel_case_ident(&option.id),
        );
        quote! { #options_path::OptionDiscriminant::#variant }
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
) -> BTreeMap<&'a str, SettingInfo<'a>> {
    let mut table = BTreeMap::new();
    for item in items {
        let info = item.option_ref().map_or_else(
            || SettingInfo {
                id: item.id(),
                kind: SpecKind::Integer,
                option: None,
                is_copy: true,
            },
            |name| {
                let option = options.get(name).expect("ref validated during cue export");
                SettingInfo {
                    id: item.id(),
                    kind: option.spec.kind(),
                    option: Some(option),
                    is_copy: !matches!(option.spec.kind(), SpecKind::String),
                }
            },
        );
        let _ = table.insert(info.id, info);
    }
    table
}

pub fn generate_predicate(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    pred: &Predicate,
    scopes: Scopes<'_>,
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
                generate_predicate(settings, &p.all[0], scopes)?
            } else {
                let parts = p
                    .all
                    .iter()
                    .map(|c| generate_predicate(settings, c, scopes))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                quote! { #( ( #parts ) )&&* }
            }
        }
        Predicate::Any(p) => {
            if p.any.is_empty() {
                quote! { false }
            } else if p.any.len() == 1 {
                generate_predicate(settings, &p.any[0], scopes)?
            } else {
                let parts = p
                    .any
                    .iter()
                    .map(|c| generate_predicate(settings, c, scopes))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                quote! { #( ( #parts ) )||* }
            }
        }
        Predicate::Not(p) => {
            let inner = generate_predicate(settings, &p.not, scopes)?;
            quote! { !( #inner ) }
        }
        Predicate::Present(p) => {
            let info = settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export");
            let field = info.field_ident();
            let accessor = scopes.pick(p.scope);
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
            generate_compare(info, scopes.pick(p.scope), &p.equals, CompareOp::Eq)?
        }
        Predicate::In(p) => {
            let info = settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export");
            let field = info.field_ident();
            let accessor = scopes.pick(p.scope);
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
            let accessor = scopes.pick(p.scope);
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
            scopes.pick(p.scope),
            &p.lt,
            CompareOp::Lt,
        )?,
        Predicate::LessThanOrEqual(p) => generate_compare(
            settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export"),
            scopes.pick(p.scope),
            &p.lte,
            CompareOp::Lte,
        )?,
        Predicate::GreaterThan(p) => generate_compare(
            settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export"),
            scopes.pick(p.scope),
            &p.gt,
            CompareOp::Gt,
        )?,
        Predicate::GreaterThanOrEqual(p) => generate_compare(
            settings
                .get(p.r#ref.as_str())
                .expect("ref validated during cue export"),
            scopes.pick(p.scope),
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
            let scopes = Scopes::new(accessor);
            let cond = generate_predicate(settings, pred, scopes)?;
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
    let head = if transformations.is_empty() {
        quote! {
            #[allow(
                clippy::missing_const_for_fn,
                clippy::needless_pass_by_ref_mut,
                clippy::unused_self,
            )]
        }
    } else {
        quote! {}
    };
    Ok(quote! {
        #head
        fn apply_transformations(&mut self) {
            #( #parts )*
        }
    })
}

pub fn generate_emit_warnings_and_infos(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    rules: &[NormalizedRule],
    scopes: Scopes<'_>,
) -> anyhow::Result<TokenStream> {
    let warning_rules: Vec<_> = rules
        .iter()
        .filter(|r| !matches!(r.severity, Severity::Error))
        .collect();
    let parts = warning_rules
        .iter()
        .map(|r| generate_normalized_rule(settings, r, scopes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let original_param = match scopes.original {
        Some(_) => quote! { , original: &Self },
        None => quote! {},
    };
    let head = if warning_rules.is_empty() {
        quote! {
            #[allow(
                clippy::missing_const_for_fn,
                clippy::unnecessary_wraps,
                clippy::unused_self,
            )]
        }
    } else {
        quote! {}
    };
    Ok(quote! {
        #head
        #[allow(unused_variables)]
        fn emit_warnings_and_infos(&self #original_param) -> ::std::result::Result<(), crate::features::simulation::SimulationError> {
            #( #parts )*
            Ok(())
        }
    })
}

pub fn generate_normalized_rule(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    rule: &NormalizedRule,
    scopes: Scopes<'_>,
) -> anyhow::Result<TokenStream> {
    let cond = generate_dnf(settings, &rule.when, scopes)?;
    Ok({
        let severity = rule.severity;
        let message: &str = &rule.message;
        let action = match severity {
            Severity::Error => {
                quote! { return Err(crate::features::simulation::SimulationError::RuleViolation(#message)); }
            }
            Severity::Warning => quote! { ::log::warn!(#message); },
            Severity::Info => quote! { ::log::info!(#message); },
        };
        quote! { if #cond { #action } }
    })
}

pub fn generate_dnf(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    dnf: &Dnf,
    scopes: Scopes<'_>,
) -> anyhow::Result<TokenStream> {
    if dnf.is_contradiction() {
        return Ok(quote! { false });
    }
    if dnf.is_tautology() {
        return Ok(quote! { true });
    }
    let parts = dnf
        .iter()
        .map(|c| generate_conjunction(settings, c, scopes))
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
    scopes: Scopes<'_>,
) -> anyhow::Result<TokenStream> {
    if conj.0.is_empty() {
        return Ok(quote! { true });
    }
    let parts = conj
        .iter()
        .map(|l| generate_leaf(settings, l, scopes))
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
    scopes: Scopes<'_>,
) -> anyhow::Result<TokenStream> {
    let pred: Predicate = leaf.clone().into();
    generate_predicate(settings, &pred, scopes)
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
            #[allow(clippy::cast_possible_truncation)]
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ast::{LeafEquals, Scope};

    fn integer_info(id: &'static str) -> SettingInfo<'static> {
        SettingInfo {
            id,
            kind: SpecKind::Integer,
            option: None,
        }
    }

    fn settings() -> BTreeMap<&'static str, SettingInfo<'static>> {
        let mut settings = BTreeMap::new();
        let _ = settings.insert("x", integer_info("x"));
        settings
    }

    fn eq_leaf(scope: Scope) -> Predicate {
        LeafEquals {
            r#ref: "x".into(),
            scope,
            equals: json!(1),
        }
        .into()
    }

    #[test]
    fn current_scope_reads_the_current_accessor() {
        let current = quote! { self };
        let original = quote! { original };
        let out = generate_predicate(
            &settings(),
            &eq_leaf(Scope::Current),
            Scopes::with_original(&current, &original),
        )
        .unwrap()
        .to_string();
        assert!(out.contains("self . x"), "got: {out}");
        assert!(!out.contains("original . x"), "got: {out}");
    }

    #[test]
    fn original_scope_reads_the_original_accessor() {
        let current = quote! { self };
        let original = quote! { original };
        let out = generate_predicate(
            &settings(),
            &eq_leaf(Scope::Original),
            Scopes::with_original(&current, &original),
        )
        .unwrap()
        .to_string();
        assert!(out.contains("original . x"), "got: {out}");
    }
}
