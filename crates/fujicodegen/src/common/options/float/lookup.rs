use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use super::super::common::{
    generate_try_from_wire_impl, resolve_enum_repr_signed, resolve_repr_type, resolve_repr_type_32,
    wire_literal,
};
use crate::{
    ast::{LookupSpec, LookupValue},
    upper_camel_case_ident,
    util::ident::numeric_variant_ident,
};

struct Resolved {
    ident: Ident,
    logical: f32,
    canonical: i32,
    alternates: Vec<i32>,
}

impl Resolved {
    fn from_spec_entry(key: &str, value: &LookupValue) -> Self {
        let logical: f32 = key.parse().expect("float-lookup key validated by CUE");
        let (canonical, alternates) = match value {
            LookupValue::Single(n) => (*n, Vec::new()),
            LookupValue::Multi(list) => {
                let (&canonical, rest) = list.split_first().expect("multi-value non-empty by CUE");
                (canonical, rest.to_vec())
            }
        };

        Self {
            ident: numeric_variant_ident(key),
            logical,
            canonical,
            alternates,
        }
    }
}

pub fn generate(
    id: &str,
    prop_code: Option<u16>,
    spec: &LookupSpec,
    default: Option<f32>,
) -> anyhow::Result<TokenStream> {
    let mut resolved: Vec<_> = spec
        .values
        .iter()
        .map(|(key, value)| Resolved::from_spec_entry(key, value))
        .collect();
    resolved.sort_by(|a, b| {
        a.logical
            .partial_cmp(&b.logical)
            .unwrap_or(::std::cmp::Ordering::Equal)
    });

    let wire_values: Vec<_> = resolved
        .iter()
        .flat_map(|r| std::iter::once(&r.canonical).chain(&r.alternates))
        .copied()
        .collect();

    let signed = resolve_enum_repr_signed(&wire_values)
        .with_context(|| format!("determining representation type for float option `{id}`"))?;
    let repr_type = resolve_repr_type(signed);
    let repr_type_32 = resolve_repr_type_32(signed);

    let type_name = upper_camel_case_ident!("{}", id);

    let enum_def = generate_enum_def(&type_name, &repr_type, signed, &resolved)
        .with_context(|| format!("generating enum definition for float option `{id}`"))?;
    let inherent_impl = generate_inherent_impl(&type_name, &resolved);
    let try_from_wire_impl = generate_try_from_wire_impl(
        &upper_camel_case_ident!("{}", id),
        signed,
        &repr_type,
        &wire_items(&resolved),
    )
    .with_context(|| format!("generating try_from_wire impl for float option `{id}`"))?;
    let try_from_logical_impl = generate_try_from_logical_impl(&type_name);
    let to_logical_impl = generate_to_logical_impl(&type_name, &resolved);
    let display_impl = generate_display_impl(&type_name);
    let from_str_impl = generate_from_str_impl(&type_name);
    let serde_impls = generate_serde_impls(&type_name);
    let (ptp_serde_impl, simulation_setting_impl) = prop_code.map_or_else(
        || (quote! {}, quote! {}),
        |code| {
            let serde = generate_ptp_serde_impl(&type_name, &repr_type);
            let setting = generate_simulation_setting_impl(&type_name, code);
            (serde, setting)
        },
    );
    let conversion_profile_impl =
        generate_conversion_profile_impl(&type_name, &repr_type, &repr_type_32);
    let default_impl = generate_default_impl(&type_name, &resolved, default);

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
        #default_impl
    })
}

fn generate_default_impl(
    type_name: &Ident,
    resolved: &[Resolved],
    default: Option<f32>,
) -> TokenStream {
    let chosen = default.map_or_else(
        || {
            resolved
                .iter()
                .min_by(|a, b| {
                    a.logical
                        .abs()
                        .partial_cmp(&b.logical.abs())
                        .unwrap_or(::std::cmp::Ordering::Equal)
                })
                .expect("at least one lookup entry")
        },
        |want| {
            #[allow(clippy::float_cmp)]
            resolved
                .iter()
                .find(|r| r.logical == want)
                .expect("default is one of the lookup keys")
        },
    );
    let variant = &chosen.ident;
    quote! {
        #[allow(clippy::derivable_impls)]
        impl ::std::default::Default for #type_name {
            fn default() -> Self {
                Self::#variant
            }
        }
    }
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
            ::std::fmt::Debug,
            ::std::clone::Clone,
            ::std::marker::Copy,
            ::std::cmp::PartialEq,
            ::std::cmp::Eq,
            ::strum_macros::EnumIter,
        )]
        pub enum #type_name {
            #(#defs)*
        }
    })
}

fn generate_inherent_impl(type_name: &Ident, resolved: &[Resolved]) -> TokenStream {
    let values_const: Vec<TokenStream> = resolved
        .iter()
        .map(|r| {
            let v = &r.ident;
            let logical = r.logical;
            quote! { (#logical, Self::#v), }
        })
        .collect();

    let logical_min = resolved.first().map_or(0.0, |r| r.logical);
    let logical_max = resolved.last().map_or(0.0, |r| r.logical);

    quote! {
        impl #type_name {
            const VALUES: &'static [(f32, Self)] = &[
                #(#values_const)*
            ];

            pub const LOGICAL_MIN: f32 = #logical_min;
            pub const LOGICAL_MAX: f32 = #logical_max;

            #[must_use]
            pub fn from_nearest_f32(value: f32) -> Self {
                Self::VALUES
                    .iter()
                    .min_by(|a, b| {
                        let da = (a.0 - value).abs();
                        let db = (b.0 - value).abs();
                        da.partial_cmp(&db).unwrap_or(::std::cmp::Ordering::Equal)
                    })
                    .map_or(Self::VALUES[0].1, |(_, v)| *v)
            }
        }
    }
}

fn generate_try_from_logical_impl(type_name: &Ident) -> TokenStream {
    quote! {
        impl ::std::convert::TryFrom<f32> for #type_name {
            type Error = crate::input::OptionError;
            fn try_from(value: f32) -> ::std::result::Result<Self, crate::input::OptionError> {
                Self::VALUES
                    .iter()
                    .find(|(v, _)| (*v - value).abs() < f32::EPSILON)
                    .map(|(_, variant)| *variant)
                    .ok_or_else(|| crate::input::OptionError::Unknown {
                        type_name: stringify!(#type_name),
                        input: value.to_string(),
                    })
            }
        }
    }
}

fn generate_to_logical_impl(type_name: &Ident, resolved: &[Resolved]) -> TokenStream {
    let arms: Vec<_> = resolved
        .iter()
        .map(|r| {
            let v = &r.ident;
            let logical = r.logical;
            quote! { #type_name::#v => #logical, }
        })
        .collect();

    quote! {
        impl ::std::convert::From<#type_name> for f32 {
            fn from(value: #type_name) -> Self {
                match value {
                    #(#arms)*
                }
            }
        }
    }
}

fn generate_display_impl(type_name: &Ident) -> TokenStream {
    quote! {
        impl ::std::fmt::Display for #type_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let n = f32::from(*self);
                if n == 0.0 { write!(f, "0") } else { write!(f, "{n:+}") }
            }
        }
    }
}

fn generate_from_str_impl(type_name: &Ident) -> TokenStream {
    quote! {
        impl ::std::str::FromStr for #type_name {
            type Err = crate::input::OptionError;
            fn from_str(s: &str) -> ::std::result::Result<Self, crate::input::OptionError> {
                let value = crate::input::CleanAlphanumeric::clean(&s)
                    .parse::<f32>()
                    .map_err(|e: ::std::num::ParseFloatError| {
                        crate::input::OptionError::InvalidValue {
                            type_name: stringify!(#type_name),
                            input: s.to_string(),
                            reason: e.to_string(),
                        }
                    })?;
                if !(Self::LOGICAL_MIN..=Self::LOGICAL_MAX).contains(&value) {
                    return Err(crate::input::OptionError::OutOfRange {
                        type_name: stringify!(#type_name),
                        value: value.to_string(),
                        min: Self::LOGICAL_MIN.to_string(),
                        max: Self::LOGICAL_MAX.to_string(),
                    });
                }
                Ok(Self::from_nearest_f32(value))
            }
        }
    }
}

fn generate_serde_impls(type_name: &Ident) -> TokenStream {
    quote! {
        impl ::serde::Serialize for #type_name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_f32(f32::from(*self))
            }
        }

        impl<'de> ::serde::Deserialize<'de> for #type_name {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let logical = f32::deserialize(deserializer)?;
                <Self as ::std::convert::TryFrom<f32>>::try_from(logical)
                    .map_err(::serde::de::Error::custom)
            }
        }
    }
}

fn generate_ptp_serde_impl(type_name: &Ident, repr_type: &Ident) -> TokenStream {
    quote! {
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
    }
}

fn generate_simulation_setting_impl(type_name: &Ident, prop_code: u16) -> TokenStream {
    quote! {
        impl crate::ptp::option::SimulationSetting for #type_name {
            fn prop_code() -> u16 { #prop_code }
        }
    }
}

fn generate_conversion_profile_impl(
    type_name: &Ident,
    repr_type: &Ident,
    repr_type_32: &Ident,
) -> TokenStream {
    quote! {
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
    }
}
