use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, ensure};
use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use serde_json::Value;

use crate::{
    ast::{
        Dnf, EnumEncoding, Leaf, LookupValue, NumericEncoding, OptionSpec, Predicate, RawValue,
        Scope, Transformation,
    },
    schema::grammar::{
        Scopes, SettingInfo, generate_dnf, generate_field_value, generate_predicate,
    },
};

#[derive(Debug)]
pub struct Encoding {
    pub when: Option<Predicate>,
    pub point: Option<Vec<(String, Value)>>,
    pub writes: Vec<(String, i32)>,
    pub witness: Option<(String, i32)>,
}

impl Encoding {
    fn describe_point(&self) -> String {
        self.point
            .iter()
            .flatten()
            .map(|(id, value)| match value {
                Value::String(s) => format!("{id} = {s}"),
                other => format!("{id} = {other}"),
            })
            .collect::<Vec<_>>()
            .join(" with ")
    }

    fn point_value(&self, id: &str) -> Option<&Value> {
        self.point
            .iter()
            .flatten()
            .find_map(|(s, v)| (s == id).then_some(v))
    }
}

pub fn analyse(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    inline: &[&str],
    transformations: &[Transformation],
) -> anyhow::Result<Vec<Encoding>> {
    let mut encodings = transformations
        .iter()
        .map(|t| {
            let writes = t
                .apply
                .iter()
                .map(|w| Ok((w.r#ref.clone(), resolve(settings, &w.r#ref, &w.value)?)))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(Encoding {
                when: t.when.clone(),
                point: t.when.as_ref().and_then(point),
                writes,
                witness: None,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    for field in inline {
        ensure!(
            encodings
                .iter()
                .any(|e| e.writes.iter().any(|(f, _)| f == field)),
            "inline field `{field}` is listed but no transformation writes it"
        );
    }

    for (i, encoding) in encodings.iter().enumerate() {
        for (field, _) in &encoding.writes {
            if settings.contains_key(field.as_str()) {
                ensure!(
                    encoding.point_value(field).is_some(),
                    "transformation {i} writes setting `{field}`, which its `when` does not fix to one value"
                );
            }
        }
    }

    let witnesses = (0..encodings.len())
        .map(|i| witness(settings, &encodings, i))
        .collect::<Vec<_>>();
    for (encoding, witness) in encodings.iter_mut().zip(witnesses) {
        encoding.witness = witness;
    }

    for (i, encoding) in encodings.iter().enumerate() {
        let writes_setting = encoding
            .writes
            .iter()
            .any(|(f, _)| settings.contains_key(f.as_str()));
        ensure!(
            !writes_setting || encoding.witness.is_some(),
            "transformation {i} writes a setting, but no value it writes is unique to it, so it cannot be undone"
        );
    }

    for (i, undone) in encodings.iter().enumerate() {
        if undone.witness.is_none() {
            continue;
        }
        for (j, later) in encodings.iter().enumerate().skip(i + 1) {
            let overlaps = later
                .writes
                .iter()
                .any(|(f, _)| undone.writes.iter().any(|(g, _)| f == g));
            if overlaps {
                ensure!(
                    excluded_at_point(later, undone),
                    "transformation {j} overwrites the pattern of transformation {i} and may fire at its point"
                );
            }
        }
    }

    for (i, a) in encodings.iter().enumerate() {
        for (j, b) in encodings.iter().enumerate().skip(i + 1) {
            if a.witness.is_none() || b.witness.is_none() {
                continue;
            }
            let disagree = a
                .point
                .iter()
                .flatten()
                .any(|(s, v)| b.point_value(s).is_some_and(|w| !same(v, w)));
            let conflict = a
                .writes
                .iter()
                .any(|(f, c)| b.writes.iter().any(|(g, d)| f == g && c != d));
            ensure!(
                !disagree || conflict,
                "transformations {i} and {j} undo to different values, but their patterns can both be on the wire"
            );
        }
    }

    Ok(encodings)
}

fn resolve(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    field: &str,
    value: &RawValue,
) -> anyhow::Result<i32> {
    match value {
        RawValue::Int(n) => Ok(i32::try_from(*n)?),
        RawValue::Variant(variant) => {
            let Some(OptionSpec::Enum {
                encoding: EnumEncoding::Lookup { spec, .. },
                ..
            }) = settings
                .get(field)
                .and_then(|info| info.option)
                .map(|o| &o.spec)
            else {
                bail!("`{field}` has no variants, so `{variant}` cannot be written to it");
            };
            match spec.values.get(variant) {
                Some(LookupValue::Single(n)) => Ok(*n),
                Some(LookupValue::Multi(list)) => Ok(list[0]),
                None => bail!("`{variant}` is not a variant of `{field}`"),
            }
        }
    }
}

fn point(when: &Predicate) -> Option<Vec<(String, Value)>> {
    let dnf = Dnf::from(when.clone());
    let [conjunction] = dnf.0.as_slice() else {
        return None;
    };
    let mut point: Vec<(String, Value)> = Vec::new();
    for leaf in conjunction {
        let Leaf::Equals(l) = leaf else {
            return None;
        };
        if l.scope != Scope::Current || point.iter().any(|(s, _)| *s == l.r#ref) {
            return None;
        }
        point.push((l.r#ref.clone(), l.equals.clone()));
    }
    (!point.is_empty()).then_some(point)
}

fn plain_image(settings: &BTreeMap<&str, SettingInfo<'_>>, field: &str) -> Option<BTreeSet<i32>> {
    let Some(info) = settings.get(field) else {
        return Some(BTreeSet::from([0]));
    };
    let values = match &info.option?.spec {
        OptionSpec::Enum {
            encoding: EnumEncoding::Lookup { spec, .. },
            ..
        }
        | OptionSpec::Integer {
            encoding: NumericEncoding::Lookup { spec, .. },
            ..
        }
        | OptionSpec::Float {
            encoding: NumericEncoding::Lookup { spec, .. },
            ..
        } => &spec.values,
        _ => return None,
    };
    let image = values
        .values()
        .flat_map(|v| match v {
            LookupValue::Single(n) => vec![*n],
            LookupValue::Multi(list) => list.clone(),
        })
        .chain([0])
        .collect();
    Some(image)
}

fn witness(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    encodings: &[Encoding],
    index: usize,
) -> Option<(String, i32)> {
    encodings[index].point.as_ref()?;
    encodings[index]
        .writes
        .iter()
        .find(|(field, c)| {
            plain_image(settings, field).is_some_and(|image| !image.contains(c))
                && encodings.iter().enumerate().all(|(j, other)| {
                    j == index || !other.writes.iter().any(|(f, d)| f == field && d == c)
                })
        })
        .cloned()
}

fn excluded_at_point(later: &Encoding, undone: &Encoding) -> bool {
    let Some(when) = later.when.as_ref() else {
        return false;
    };
    Dnf::from(when.clone()).iter().all(|conjunction| {
        conjunction.iter().any(|leaf| {
            leaf.scope() == Scope::Current
                && undone
                    .point_value(leaf.r#ref())
                    .and_then(|value| holds(leaf, value))
                    == Some(false)
        })
    })
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < f64::EPSILON,
        _ => a == b,
    }
}

fn compare(value: &Value, bound: &Value) -> Option<std::cmp::Ordering> {
    value.as_f64()?.partial_cmp(&bound.as_f64()?)
}

fn holds(leaf: &Leaf, value: &Value) -> Option<bool> {
    use std::cmp::Ordering::{Equal, Greater, Less};

    let ordered = |bound: &Value, accept: &[std::cmp::Ordering]| {
        compare(value, bound).map(|o| accept.contains(&o))
    };
    let between = |min: &Value, max: &Value| {
        Some(compare(value, min)? != Less && compare(value, max)? != Greater)
    };
    match leaf {
        Leaf::Present(p) => Some(p.present),
        Leaf::Equals(l) => Some(same(value, &l.equals)),
        Leaf::NotEquals(l) => Some(!same(value, &l.equals)),
        Leaf::In(l) => Some(l.values.iter().any(|v| same(value, v))),
        Leaf::NotIn(l) => Some(!l.values.iter().any(|v| same(value, v))),
        Leaf::Between(l) => between(&l.min, &l.max),
        Leaf::NotBetween(l) => between(&l.min, &l.max).map(|b| !b),
        Leaf::LessThan(l) => ordered(&l.lt, &[Less]),
        Leaf::NotLessThan(l) => ordered(&l.lt, &[Equal, Greater]),
        Leaf::LessThanOrEqual(l) => ordered(&l.lte, &[Less, Equal]),
        Leaf::NotLessThanOrEqual(l) => ordered(&l.lte, &[Greater]),
        Leaf::GreaterThan(l) => ordered(&l.gt, &[Greater]),
        Leaf::NotGreaterThan(l) => ordered(&l.gt, &[Less, Equal]),
        Leaf::GreaterThanOrEqual(l) => ordered(&l.gte, &[Equal, Greater]),
        Leaf::NotGreaterThanOrEqual(l) => ordered(&l.gte, &[Less]),
    }
}

pub fn generate_encode(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    encodings: &[Encoding],
    positions: &BTreeMap<&str, usize>,
    draft: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let blocks = encodings
        .iter()
        .map(|e| {
            let writes = e.writes.iter().map(|(field, c)| {
                let position = positions[field.as_str()];
                let c = Literal::i32_suffixed(*c);
                quote! { values[#position] = #c; }
            });
            let Some(when) = &e.when else {
                return Ok(quote! { #( #writes )* });
            };
            let cond = generate_predicate(settings, when, Scopes::new(draft))?;
            Ok(quote! { if #cond { #( #writes )* } })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(quote! { #( #blocks )* })
}

pub fn undo_ident(index: usize) -> proc_macro2::Ident {
    format_ident!("undo_{}", index)
}

pub fn raw_ident(field: &str) -> proc_macro2::Ident {
    format_ident!("raw_{}", field)
}

pub fn generate_undo(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    encodings: &[Encoding],
    positions: &BTreeMap<&str, usize>,
    staged: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let blocks = encodings
        .iter()
        .enumerate()
        .filter_map(|(i, e)| Some((i, e, e.witness.as_ref()?)))
        .map(|(i, e, (witness_field, witness_value))| {
            let flag = undo_ident(i);
            let matches = e.writes.iter().map(|(field, c)| {
                let raw = raw_ident(field);
                let c = Literal::i32_suffixed(*c);
                quote! { #raw == #c }
            });
            let lone_witness = (e.writes.len() > 1).then(|| {
                let witness_raw = raw_ident(witness_field);
                let witness_lit = Literal::i32_suffixed(*witness_value);
                let message = format!(
                    "position {} holds {}, which only {} writes, but the rest of its pattern is not on the wire",
                    positions[witness_field.as_str()],
                    witness_value,
                    e.describe_point(),
                );
                quote! {
                    if !#flag && #witness_raw == #witness_lit {
                        return Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData, #message));
                    }
                }
            });
            let assigns = e
                .point
                .iter()
                .flatten()
                .map(|(id, value)| {
                    let info = &settings[id.as_str()];
                    let ident = info.field_ident();
                    let value = generate_field_value(info, value)?;
                    Ok(quote! { #staged.#ident = Some(#value); })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(quote! {
                let #flag = #( #matches )&&*;
                #lone_witness
                if #flag {
                    #( #assigns )*
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(quote! { #( #blocks )* })
}

pub fn settled_by(encodings: &[Encoding], id: &str) -> Vec<proc_macro2::Ident> {
    encodings
        .iter()
        .enumerate()
        .filter(|(_, e)| e.witness.is_some() && e.point_value(id).is_some())
        .map(|(i, _)| undo_ident(i))
        .collect()
}

pub fn generate_settled_check(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    encodings: &[Encoding],
    conditions: &BTreeMap<String, Dnf>,
    staged: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let checks = encodings
        .iter()
        .enumerate()
        .filter(|(_, e)| e.witness.is_some())
        .flat_map(|(i, e)| {
            e.point
                .iter()
                .flatten()
                .filter_map(move |(id, _)| Some((i, e, id, conditions.get(id)?)))
        })
        .map(|(i, e, id, condition)| {
            let flag = undo_ident(i);
            let cond = generate_dnf(settings, condition, Scopes::new(staged))?;
            let message = format!(
                "the profile holds {}, but `{id}` does not apply alongside the other settings",
                e.describe_point()
            );
            Ok(quote! {
                if #flag {
                    #[allow(clippy::manual_is_variant_and)]
                    let allowed = #cond;
                    if !allowed {
                        return Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData, #message));
                    }
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(quote! { #( #checks )* })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ast::{FujiOption, LeafEquals, PredAll, PredNot, RawWrite, SpecKind};

    fn option(id: &str, values: &Value) -> FujiOption {
        serde_json::from_value(json!({
            "id": id,
            "spec": {
                "name": id, "kind": "enum",
                "rules": { "variants": values.as_object().unwrap().keys().map(|k| json!({"id": k, "name": k})).collect::<Vec<_>>() },
                "encoding": { "kind": "lookup", "spec": { "values": values } }
            }
        }))
        .unwrap()
    }

    struct Fixture {
        dr: FujiOption,
        drp: FujiOption,
        wb: FujiOption,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                dr: option(
                    "dr",
                    &json!({"hdr400": 0x190, "hdr800": 0x320, "hdr800_plus": 0x640}),
                ),
                drp: option("drp", &json!({"off": 0, "weak": 1, "strong": 2})),
                wb: option("wb", &json!({"as_shot": 0, "auto": 2})),
            }
        }

        fn settings(&self) -> BTreeMap<&str, SettingInfo<'_>> {
            [("dr", &self.dr), ("drp", &self.drp), ("wb", &self.wb)]
                .into_iter()
                .map(|(id, option)| {
                    (
                        id,
                        SettingInfo {
                            id,
                            kind: SpecKind::Enum,
                            option: Some(option),
                            is_copy: true,
                        },
                    )
                })
                .collect()
        }
    }

    fn eq(r: &str, v: Value) -> Predicate {
        LeafEquals {
            r#ref: r.into(),
            scope: Scope::Current,
            equals: v,
        }
        .into()
    }

    fn write(r: &str, v: RawValue) -> RawWrite {
        RawWrite {
            r#ref: r.into(),
            value: v,
        }
    }

    fn hdr800_plus() -> Transformation {
        Transformation {
            when: Some(
                PredAll {
                    all: vec![eq("dr", json!("hdr800_plus")), eq("drp", json!("off"))],
                }
                .into(),
            ),
            apply: vec![
                write("dr", RawValue::Variant("hdr800".into())),
                write("drp", RawValue::Int(3)),
            ],
        }
    }

    fn as_shot(value: i64, negated: bool) -> Transformation {
        let when = eq("wb", json!("as_shot"));
        Transformation {
            when: Some(if negated {
                PredNot {
                    not: Box::new(when),
                }
                .into()
            } else {
                when
            }),
            apply: vec![write("wb_as_shot", RawValue::Int(value))],
        }
    }

    #[test]
    fn a_point_with_a_unique_value_is_undone_by_that_value() {
        let fixture = Fixture::new();
        let encodings = analyse(&fixture.settings(), &[], &[hdr800_plus()]).unwrap();
        assert_eq!(
            encodings[0].writes,
            [("dr".into(), 0x320), ("drp".into(), 3)]
        );
        assert_eq!(encodings[0].witness, Some(("drp".into(), 3)));
    }

    #[test]
    fn writing_a_setting_needs_a_unique_value() {
        let fixture = Fixture::new();
        let mut t = hdr800_plus();
        t.apply.pop();
        let err = analyse(&fixture.settings(), &[], &[t]).unwrap_err();
        assert!(err.to_string().contains("cannot be undone"), "{err}");
    }

    #[test]
    fn writing_a_setting_outside_the_point_is_rejected() {
        let fixture = Fixture::new();
        let mut t = hdr800_plus();
        t.apply.push(write("wb", RawValue::Int(9)));
        let err = analyse(&fixture.settings(), &[], &[t]).unwrap_err();
        assert!(err.to_string().contains("does not fix"), "{err}");
    }

    #[test]
    fn a_shared_value_is_no_witness() {
        let fixture = Fixture::new();
        let encodings = analyse(
            &fixture.settings(),
            &["wb_as_shot"],
            &[as_shot(1, false), as_shot(1, true)],
        )
        .unwrap();
        assert!(encodings.iter().all(|e| e.witness.is_none()));
    }

    #[test]
    fn inline_zero_is_no_witness() {
        let fixture = Fixture::new();
        let encodings =
            analyse(&fixture.settings(), &["wb_as_shot"], &[as_shot(0, false)]).unwrap();
        assert!(encodings[0].witness.is_none());
    }

    #[test]
    fn an_unwritten_inline_field_is_rejected() {
        let fixture = Fixture::new();
        let err = analyse(&fixture.settings(), &["wb_as_shot"], &[]).unwrap_err();
        assert!(
            err.to_string().contains("no transformation writes it"),
            "{err}"
        );
    }

    #[test]
    fn a_later_write_must_be_false_at_the_point() {
        let fixture = Fixture::new();
        let settings = fixture.settings();
        assert!(
            analyse(
                &settings,
                &["wb_as_shot"],
                &[as_shot(1, false), as_shot(2, true)]
            )
            .is_ok()
        );

        let unconditional = Transformation {
            when: None,
            apply: vec![write("wb_as_shot", RawValue::Int(2))],
        };
        let err = analyse(
            &settings,
            &["wb_as_shot"],
            &[as_shot(1, false), unconditional],
        )
        .unwrap_err();
        assert!(err.to_string().contains("may fire at its point"), "{err}");
    }

    #[test]
    fn disagreeing_points_need_conflicting_patterns() {
        let fixture = Fixture::new();
        let other = Transformation {
            when: Some(eq("dr", json!("hdr400"))),
            apply: vec![write("wb_as_shot", RawValue::Int(7))],
        };
        let err = analyse(
            &fixture.settings(),
            &["wb_as_shot"],
            &[hdr800_plus(), other],
        )
        .unwrap_err();
        assert!(err.to_string().contains("can both be on the wire"), "{err}");
    }

    #[test]
    fn undo_sets_the_point_and_rejects_a_lone_witness() {
        let fixture = Fixture::new();
        let settings = fixture.settings();
        let encodings = analyse(&settings, &[], &[hdr800_plus()]).unwrap();
        let positions = BTreeMap::from([("dr", 5), ("drp", 6)]);
        let out = generate_undo(&settings, &encodings, &positions, &quote! { staged })
            .unwrap()
            .to_string();
        assert!(
            out.contains("let undo_0 = raw_dr == 800i32 && raw_drp == 3i32"),
            "{out}"
        );
        assert!(out.contains("if ! undo_0 && raw_drp == 3i32"), "{out}");
        assert!(out.contains("staged . dr = Some"), "{out}");
        assert!(
            out.contains("position 6 holds 3, which only dr = hdr800_plus with drp = off writes"),
            "{out}"
        );
    }

    #[test]
    fn encode_writes_raw_values_when_the_point_holds() {
        let fixture = Fixture::new();
        let settings = fixture.settings();
        let encodings = analyse(&settings, &[], &[hdr800_plus()]).unwrap();
        let positions = BTreeMap::from([("dr", 5), ("drp", 6)]);
        let out = generate_encode(&settings, &encodings, &positions, &quote! { d })
            .unwrap()
            .to_string();
        assert!(
            out.contains("values [5usize] = 800i32 ; values [6usize] = 3i32 ;"),
            "{out}"
        );
    }
}
