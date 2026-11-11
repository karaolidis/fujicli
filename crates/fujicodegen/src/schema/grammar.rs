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
    upper_camel_case_ident,
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
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

impl CompareOp {
    fn tokens(self) -> TokenStream {
        match self {
            Self::Eq => quote! { == },
            Self::Ne => quote! { != },
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
                let type_ident = upper_camel_case_ident!("{}", option.id);
                quote! { #options_path::#type_ident }
            },
        )
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
                let option = &options[name];
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

/// Predicates follow Kleene's three-valued logic: a value test on an absent field is unknown,
/// and a predicate holds only when it is definitely true. Negations are pushed down to the
/// leaves, so a negated value test still requires its field to be present.
pub fn generate_predicate(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    pred: &Predicate,
    scopes: Scopes<'_>,
) -> anyhow::Result<TokenStream> {
    generate_polarized(settings, pred, scopes, true)
}

fn generate_polarized(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    pred: &Predicate,
    scopes: Scopes<'_>,
    positive: bool,
) -> anyhow::Result<TokenStream> {
    match pred {
        Predicate::Bool(b) => {
            let b = *b == positive;
            Ok(quote! { #b })
        }
        Predicate::All(p) => generate_junction(settings, &p.all, scopes, positive, positive),
        Predicate::Any(p) => generate_junction(settings, &p.any, scopes, positive, !positive),
        Predicate::Not(p) => generate_polarized(settings, &p.not, scopes, !positive),
        leaf => {
            let leaf = Leaf::try_from(leaf.clone())?;
            let leaf = if positive { leaf } else { leaf.negated() };
            generate_leaf(settings, &leaf, scopes)
        }
    }
}

fn generate_junction(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    preds: &[Predicate],
    scopes: Scopes<'_>,
    positive: bool,
    conjunctive: bool,
) -> anyhow::Result<TokenStream> {
    let mut parts = preds
        .iter()
        .map(|p| generate_polarized(settings, p, scopes, positive))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(match (parts.len(), conjunctive) {
        (0, true) => quote! { true },
        (0, false) => quote! { false },
        (1, _) => parts.pop().expect("parts has exactly one element"),
        (_, true) => quote! { #( ( #parts ) )&&* },
        (_, false) => quote! { #( ( #parts ) )||* },
    })
}

pub fn generate_assignment(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    assignment: &Assignment,
    accessor: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let info = &settings[assignment.r#ref.as_str()];
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
    accessor: &TokenStream,
    buf_ty: &TokenStream,
) -> anyhow::Result<TokenStream> {
    if transformations.is_empty() {
        return Ok(quote! {
            #[allow(clippy::needless_pass_by_ref_mut)]
            pub const fn apply_transformations(_buf: &mut #buf_ty) {}
        });
    }
    let parts = transformations
        .iter()
        .map(|t| generate_transformation(settings, t, accessor))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(quote! {
        #[allow(clippy::nonminimal_bool)]
        pub fn apply_transformations(#accessor: &mut #buf_ty) {
            #( #parts )*
        }
    })
}

pub fn generate_emit_warnings_and_infos(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    rules: &[NormalizedRule],
    scopes: Scopes<'_>,
    buf_ty: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let accessor = scopes.current;
    let original_param = match scopes.original {
        Some(_) => quote! { , original: &#buf_ty },
        None => quote! {},
    };

    let warning_rules: Vec<_> = rules
        .iter()
        .filter(|r| !matches!(r.severity, Severity::Error))
        .collect();
    if warning_rules.is_empty() {
        let empty_original_param = match scopes.original {
            Some(_) => quote! { , _original: &#buf_ty },
            None => quote! {},
        };
        return Ok(quote! {
            #[allow(clippy::unnecessary_wraps)]
            pub const fn emit_warnings_and_infos(
                _buf: &#buf_ty
                #empty_original_param,
            ) -> ::std::result::Result<(), crate::features::simulation::SimulationError> {
                Ok(())
            }
        });
    }
    let parts = warning_rules
        .iter()
        .map(|r| generate_normalized_rule(settings, r, scopes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(quote! {
        #[allow(unused_variables)]
        pub fn emit_warnings_and_infos(
            #accessor: &#buf_ty
            #original_param,
        ) -> ::std::result::Result<(), crate::features::simulation::SimulationError> {
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
    let info = &settings[leaf.r#ref()];
    let field = info.field_ident();
    let accessor = scopes.pick(leaf.scope());

    let (test, negated) = match leaf {
        Leaf::Present(p) => {
            return Ok(if p.present {
                quote! { #accessor.#field.is_some() }
            } else {
                quote! { #accessor.#field.is_none() }
            });
        }
        Leaf::Equals(l) => (generate_compare(info, &l.equals, CompareOp::Eq)?, false),
        Leaf::NotEquals(l) => (generate_compare(info, &l.equals, CompareOp::Ne)?, false),
        Leaf::In(l) if l.values.is_empty() => return Ok(quote! { false }),
        Leaf::NotIn(l) if l.values.is_empty() => return Ok(quote! { #accessor.#field.is_some() }),
        Leaf::In(l) => (generate_membership(info, &l.values)?, false),
        Leaf::NotIn(l) => (generate_membership(info, &l.values)?, true),
        Leaf::Between(l) => (generate_range(info, &l.min, &l.max)?, false),
        Leaf::NotBetween(l) => (generate_range(info, &l.min, &l.max)?, true),
        Leaf::LessThan(l) => (generate_compare(info, &l.lt, CompareOp::Lt)?, false),
        Leaf::NotLessThan(l) => (generate_compare(info, &l.lt, CompareOp::Gte)?, false),
        Leaf::LessThanOrEqual(l) => (generate_compare(info, &l.lte, CompareOp::Lte)?, false),
        Leaf::NotLessThanOrEqual(l) => (generate_compare(info, &l.lte, CompareOp::Gt)?, false),
        Leaf::GreaterThan(l) => (generate_compare(info, &l.gt, CompareOp::Gt)?, false),
        Leaf::NotGreaterThan(l) => (generate_compare(info, &l.gt, CompareOp::Lte)?, false),
        Leaf::GreaterThanOrEqual(l) => (generate_compare(info, &l.gte, CompareOp::Gte)?, false),
        Leaf::NotGreaterThanOrEqual(l) => (generate_compare(info, &l.gte, CompareOp::Lt)?, false),
    };

    let test = if negated {
        quote! { !( #test ) }
    } else {
        test
    };
    Ok(if info.kind == SpecKind::String {
        quote! { #accessor.#field.as_deref().is_some_and(|v| #test) }
    } else {
        quote! { #accessor.#field.is_some_and(|v| #test) }
    })
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
            let variant = upper_camel_case_ident!("{}", s);
            Ok(quote! { #path::#variant })
        }
    }
}

fn generate_compare(
    info: &SettingInfo<'_>,
    value: &Value,
    op: CompareOp,
) -> anyhow::Result<TokenStream> {
    let value_expr = generate_value_expr(info, value)?;

    Ok(match (info.kind, op) {
        (SpecKind::Enum | SpecKind::String, CompareOp::Eq) => quote! { v == #value_expr },
        (SpecKind::Enum | SpecKind::String, CompareOp::Ne) => quote! { v != #value_expr },
        (SpecKind::Enum | SpecKind::String, _) => bail!(
            "ordered comparison ({op:?}) on non-numeric setting `{}` is not supported",
            info.id
        ),
        (SpecKind::Integer, _) => {
            let op_tok = op.tokens();
            let v_expr = info.int(&quote! { v });
            quote! { #v_expr #op_tok (#value_expr) }
        }
        (SpecKind::Float, CompareOp::Eq) => {
            quote! { (f32::from(v) - (#value_expr)).abs() < f32::EPSILON }
        }
        (SpecKind::Float, CompareOp::Ne) => {
            quote! { (f32::from(v) - (#value_expr)).abs() >= f32::EPSILON }
        }
        (SpecKind::Float, _) => {
            let op_tok = op.tokens();
            quote! { f32::from(v) #op_tok (#value_expr) }
        }
    })
}

fn generate_membership(info: &SettingInfo<'_>, values: &[Value]) -> anyhow::Result<TokenStream> {
    let arms = values
        .iter()
        .map(|v| generate_value_expr(info, v))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(match info.kind {
        SpecKind::Enum | SpecKind::String => quote! { matches!(v, #( #arms )|*) },
        SpecKind::Integer => {
            let v_expr = info.int(&quote! { v });
            quote! { matches!(#v_expr, #( #arms )|*) }
        }
        SpecKind::Float => quote! {
            {
                let v = f32::from(v);
                #( (v - (#arms)).abs() < f32::EPSILON )||*
            }
        },
    })
}

fn generate_range(info: &SettingInfo<'_>, min: &Value, max: &Value) -> anyhow::Result<TokenStream> {
    let lo = generate_value_expr(info, min)?;
    let hi = generate_value_expr(info, max)?;

    Ok(match info.kind {
        SpecKind::Integer => {
            let v_expr = info.int(&quote! { v });
            quote! { ( (#lo) ..= (#hi) ).contains(&#v_expr) }
        }
        SpecKind::Float => quote! { ( (#lo) ..= (#hi) ).contains(&f32::from(v)) },
        other => bail!(
            "`between` predicate on non-numeric setting `{}` ({other:?})",
            info.id
        ),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ast::{
        Assignment, AssignmentEffect, LeafBetween, LeafEquals, LeafIn, PredAll, PredNot, Scope,
        Transformation,
    };

    fn integer_info(id: &'static str) -> SettingInfo<'static> {
        SettingInfo {
            id,
            kind: SpecKind::Integer,
            option: None,
            is_copy: true,
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

    #[test]
    fn negated_value_test_requires_presence() {
        let current = quote! { self };
        let pred = PredNot {
            not: Box::new(eq_leaf(Scope::Current)),
        }
        .into();
        let out = generate_predicate(&settings(), &pred, Scopes::new(&current))
            .unwrap()
            .to_string();
        assert_eq!(out, "self . x . is_some_and (| v | v != (1i32))");
    }

    #[test]
    fn negation_is_pushed_through_junctions() {
        let current = quote! { self };
        let pred = PredNot {
            not: Box::new(
                PredAll {
                    all: vec![eq_leaf(Scope::Current), Predicate::Bool(true)],
                }
                .into(),
            ),
        }
        .into();
        let out = generate_predicate(&settings(), &pred, Scopes::new(&current))
            .unwrap()
            .to_string();
        assert_eq!(
            out,
            "(self . x . is_some_and (| v | v != (1i32))) || (false)"
        );
    }

    fn leaf_tokens(leaf: &Leaf) -> String {
        let current = quote! { self };
        generate_leaf(&settings(), leaf, Scopes::new(&current))
            .unwrap()
            .to_string()
    }

    #[test]
    fn negated_range_requires_presence() {
        let out = leaf_tokens(&Leaf::NotBetween(LeafBetween {
            r#ref: "x".into(),
            scope: Scope::Current,
            min: json!(1),
            max: json!(2),
        }));
        assert_eq!(
            out,
            "self . x . is_some_and (| v | ! (((1i32) ..= (2i32)) . contains (& v)))"
        );
    }

    #[test]
    fn empty_membership_needs_no_value() {
        let empty = LeafIn {
            r#ref: "x".into(),
            scope: Scope::Current,
            values: vec![],
        };
        assert_eq!(leaf_tokens(&Leaf::In(empty.clone())), "false");
        assert_eq!(leaf_tokens(&Leaf::NotIn(empty)), "self . x . is_some ()");
    }

    #[test]
    fn negated_transformation_condition_skips_absent_fields() {
        let transformation = Transformation {
            when: Some(
                PredNot {
                    not: Box::new(eq_leaf(Scope::Current)),
                }
                .into(),
            ),
            apply: vec![Assignment {
                r#ref: "x".into(),
                effect: AssignmentEffect::Set(json!(2)),
            }],
        };
        let out = generate_transformation(&settings(), &transformation, &quote! { buf })
            .unwrap()
            .to_string();
        assert_eq!(
            out,
            "if buf . x . is_some_and (| v | v != (1i32)) { buf . x = Some (2i32) ; }"
        );
    }
}
