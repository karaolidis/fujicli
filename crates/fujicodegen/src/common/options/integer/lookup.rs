use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use super::super::common::{
    generate_try_from_wire_impl, resolve_enum_repr_signed, resolve_repr_type, resolve_repr_type_32,
    wire_literal,
};
use crate::{
    ast::{LookupSpec, LookupValue},
    util::ident::{numeric_variant_ident, safe_upper_camel_case_ident},
};

struct Resolved {
    ident: Ident,
    logical: i32,
    canonical: i32,
    alternates: Vec<i32>,
}

impl Resolved {
    fn from_spec_entry(key: &str, value: &LookupValue) -> anyhow::Result<Self> {
        let logical = key
            .parse()
            .with_context(|| format!("integer-lookup key `{key}` is not an integer"))?;

        let (canonical, alternates) = match value {
            LookupValue::Single(n) => (*n, Vec::new()),
            LookupValue::Multi(list) => {
                let (&canonical, rest) = list
                    .split_first()
                    .with_context(|| format!("empty multi-value lookup for key `{key}`"))?;
                (canonical, rest.to_vec())
            }
        };

        Ok(Self {
            ident: numeric_variant_ident(key),
            logical,
            canonical,
            alternates,
        })
    }
}

pub(crate) fn generate(
    id: &str,
    prop_code: Option<u16>,
    spec: &LookupSpec,
) -> anyhow::Result<TokenStream> {
    let mut resolved: Vec<_> = spec
        .values
        .iter()
        .map(|(key, value)| Resolved::from_spec_entry(key, value))
        .collect::<anyhow::Result<Vec<_>>>()
        .with_context(|| format!("resolving lookup entries for integer option `{id}`"))?;
    resolved.sort_by_key(|r| r.logical);

    let wire_values: Vec<_> = resolved
        .iter()
        .flat_map(|r| std::iter::once(&r.canonical).chain(&r.alternates))
        .copied()
        .collect();

    let signed = resolve_enum_repr_signed(&wire_values)
        .with_context(|| format!("determining representation type for integer option `{id}`"))?;
    let repr_type = resolve_repr_type(signed);
    let repr_type_32 = resolve_repr_type_32(signed);

    let type_name = safe_upper_camel_case_ident(id);

    let enum_def = generate_enum_def(&type_name, &repr_type, signed, &resolved)
        .with_context(|| format!("generating enum definition for integer option `{id}`"))?;
    let inherent_impl = generate_inherent_impl(&type_name, &resolved)
        .with_context(|| format!("generating inherent impl for integer option `{id}`"))?;
    let try_from_wire_impl = generate_try_from_wire_impl(
        &safe_upper_camel_case_ident(id),
        signed,
        &repr_type,
        &wire_items(&resolved),
    )
    .with_context(|| format!("generating try_from_wire impl for integer option `{id}`"))?;
    let try_from_logical_impl = generate_try_from_logical_impl(&type_name, &resolved)
        .with_context(|| format!("generating TryFrom<i32> impl for integer option `{id}`"))?;
    let to_logical_impl = generate_to_logical_impl(&type_name, &resolved).with_context(|| {
        format!("generating From<{type_name}> for i32 impl for integer option `{id}`")
    })?;
    let display_impl = generate_display_impl(&type_name)
        .with_context(|| format!("generating Display impl for integer option `{id}`"))?;
    let from_str_impl = generate_from_str_impl(&type_name)
        .with_context(|| format!("generating FromStr impl for integer option `{id}`"))?;
    let serde_impls = generate_serde_impls(&type_name)
        .with_context(|| format!("generating Serde impls for integer option `{id}`"))?;
    let (ptp_serde_impl, simulation_setting_impl) = if let Some(code) = prop_code {
        let serde = generate_ptp_serde_impl(&type_name, &repr_type).with_context(|| {
            format!("generating PtpSerialize/PtpDeserialize impls for integer option `{id}`")
        })?;
        let setting = generate_simulation_setting_impl(&type_name, code).with_context(|| {
            format!("generating SimulationSetting impl for integer option `{id}`")
        })?;
        (serde, setting)
    } else {
        (quote! {}, quote! {})
    };
    let conversion_profile_impl =
        generate_conversion_profile_impl(&type_name, &repr_type, &repr_type_32).with_context(
            || format!("generating ConversionProfileField impl for integer option `{id}`"),
        )?;

    Ok(quote! {
        #enum_def
        #inherent_impl
        #try_from_wire_impl
        #try_from_logical_impl
        #to_logical_impl
        #display_impl
        #from_str_impl
        #serde_impls
        #ptp_serde_impl
        #simulation_setting_impl
        #conversion_profile_impl
    })
}

fn wire_items(resolved: &[Resolved]) -> Vec<(Ident, Vec<i32>)> {
    resolved
        .iter()
        .map(|r| {
            let mut wires = Vec::with_capacity(1 + r.alternates.len());
            wires.push(r.canonical);
            wires.extend(r.alternates.iter().copied());
            (r.ident.clone(), wires)
        })
        .collect()
}

fn generate_enum_def(
    type_name: &Ident,
    repr_type: &Ident,
    signed: bool,
    resolved: &[Resolved],
) -> anyhow::Result<TokenStream> {
    let defs = resolved
        .iter()
        .map(|r| {
            let v = &r.ident;
            let canonical = wire_literal(r.canonical, signed)?;
            Ok(quote! { #v = #canonical, })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        #[repr(#repr_type)]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            strum_macros::EnumIter,
        )]
        pub enum #type_name {
            #(#defs)*
        }
    })
}

fn generate_inherent_impl(type_name: &Ident, resolved: &[Resolved]) -> anyhow::Result<TokenStream> {
    let values_const: Vec<_> = resolved
        .iter()
        .map(|r| {
            let v = &r.ident;
            let logical = r.logical as f32;
            quote! { (#logical, Self::#v), }
        })
        .collect();

    let logical_min = resolved.first().map(|r| r.logical).unwrap_or(0);
    let logical_max = resolved.last().map(|r| r.logical).unwrap_or(0);

    Ok(quote! {
        impl #type_name {
            const VALUES: &'static [(f32, Self)] = &[
                #(#values_const)*
            ];

            pub const LOGICAL_MIN: i32 = #logical_min;
            pub const LOGICAL_MAX: i32 = #logical_max;

            pub fn from_nearest_i32(value: i32) -> Self {
                Self::from_nearest_f32(value as f32)
            }

            fn from_nearest_f32(value: f32) -> Self {
                Self::VALUES
                    .iter()
                    .min_by(|a, b| {
                        let da = (a.0 - value).abs();
                        let db = (b.0 - value).abs();
                        da.partial_cmp(&db).unwrap_or(::std::cmp::Ordering::Equal)
                    })
                    .map(|(_, v)| *v)
                    .unwrap_or(Self::VALUES[0].1)
            }
        }
    })
}

fn generate_try_from_logical_impl(
    type_name: &Ident,
    resolved: &[Resolved],
) -> anyhow::Result<TokenStream> {
    let arms = resolved
        .iter()
        .map(|r| {
            let v = &r.ident;
            let logical = r.logical;
            Ok(quote! { #logical => Ok(Self::#v), })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        impl ::std::convert::TryFrom<i32> for #type_name {
            type Error = ::anyhow::Error;
            fn try_from(value: i32) -> ::anyhow::Result<Self> {
                match value {
                    #(#arms)*
                    _ => ::anyhow::bail!(
                        "Value {} is not a valid {}",
                        value, stringify!(#type_name),
                    ),
                }
            }
        }
    })
}

fn generate_to_logical_impl(
    type_name: &Ident,
    resolved: &[Resolved],
) -> anyhow::Result<TokenStream> {
    let arms = resolved
        .iter()
        .map(|r| {
            let v = &r.ident;
            let logical = r.logical;
            Ok(quote! { #type_name::#v => #logical, })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        impl ::std::convert::From<#type_name> for i32 {
            fn from(value: #type_name) -> i32 {
                match value {
                    #(#arms)*
                }
            }
        }
    })
}

fn generate_display_impl(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::std::fmt::Display for #type_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let n = i32::from(*self);
                if n == 0 { write!(f, "0") } else { write!(f, "{n:+}") }
            }
        }
    })
}

fn generate_from_str_impl(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::std::str::FromStr for #type_name {
            type Err = ::anyhow::Error;
            fn from_str(s: &str) -> ::anyhow::Result<Self> {
                let value = crate::input::CleanAlphanumeric::clean(&s)
                    .parse::<f32>()
                    .map_err(|e| ::anyhow::anyhow!("Invalid numeric value '{}': {}", s, e))?;
                let min = Self::LOGICAL_MIN as f32;
                let max = Self::LOGICAL_MAX as f32;
                if !(min..=max).contains(&value) {
                    ::anyhow::bail!(
                        "{} value {} is out of range [{}, {}]",
                        stringify!(#type_name), value, min, max,
                    );
                }
                Ok(Self::from_nearest_f32(value))
            }
        }
    })
}

fn generate_serde_impls(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::serde::Serialize for #type_name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_i32(i32::from(*self))
            }
        }

        impl<'de> ::serde::Deserialize<'de> for #type_name {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let logical = i32::deserialize(deserializer)?;
                <Self as ::std::convert::TryFrom<i32>>::try_from(logical)
                    .map_err(::serde::de::Error::custom)
            }
        }
    })
}

fn generate_ptp_serde_impl(type_name: &Ident, repr_type: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::ptp_cursor::PtpSerialize for #type_name {
            fn try_into_ptp(&self) -> ::std::io::Result<Vec<u8>> {
                let mut buf = Vec::new();
                <Self as ::ptp_cursor::PtpSerialize>::try_write_ptp(self, &mut buf)?;
                Ok(buf)
            }

            fn try_write_ptp(&self, buf: &mut Vec<u8>) -> ::std::io::Result<()> {
                let raw: #repr_type = *self as #repr_type;
                ::ptp_cursor::PtpSerialize::try_write_ptp(&raw, buf)
            }
        }

        impl ::ptp_cursor::PtpDeserialize for #type_name {
            fn try_from_ptp(buf: &[u8]) -> ::std::io::Result<Self> {
                let mut cur = ::std::io::Cursor::new(buf);
                let val = <Self as ::ptp_cursor::PtpDeserialize>::try_read_ptp(&mut cur)?;
                ::ptp_cursor::Read::expect_end(&mut cur)?;
                Ok(val)
            }

            fn try_read_ptp<R: ::ptp_cursor::Read>(cur: &mut R) -> ::std::io::Result<Self> {
                let raw = <#repr_type as ::ptp_cursor::PtpDeserialize>::try_read_ptp(cur)?;
                Self::try_from_wire(raw)
            }
        }
    })
}

fn generate_simulation_setting_impl(
    type_name: &Ident,
    prop_code: u16,
) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl crate::ptp::option::SimulationSetting for #type_name {
            fn prop_code() -> u16 { #prop_code }
        }
    })
}

fn generate_conversion_profile_impl(
    type_name: &Ident,
    repr_type: &Ident,
    repr_type_32: &Ident,
) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl crate::ptp::option::ConversionProfileField for #type_name {
            fn try_write_conversion_profile_field_ptp(
                &self, buf: &mut Vec<u8>,
            ) -> ::std::io::Result<()> {
                let raw: #repr_type = *self as #repr_type;
                ::ptp_cursor::PtpSerialize::try_write_ptp(&#repr_type_32::from(raw), buf)
            }

            fn try_read_conversion_profile_field_ptp<R: ::ptp_cursor::Read>(
                cur: &mut R,
            ) -> ::std::io::Result<Self> {
                let extended = <#repr_type_32 as ::ptp_cursor::PtpDeserialize>::try_read_ptp(cur)?;
                let raw: #repr_type = extended.try_into().map_err(|_| {
                    ::std::io::Error::new(
                        ::std::io::ErrorKind::InvalidData,
                        format!(
                            "{} value {} doesn't fit in {}",
                            stringify!(#type_name),
                            extended,
                            stringify!(#repr_type),
                        ),
                    )
                })?;
                Self::try_from_wire(raw)
            }
        }
    })
}
