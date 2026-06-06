use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use super::common::{
    generate_try_from_wire_impl, resolve_enum_repr_signed, resolve_repr_type, resolve_repr_type_32,
    wire_literal,
};
use crate::{
    ast::{EnumEncoding, EnumRules, EnumVariant, LookupSpec, LookupValue},
    upper_camel_case_ident,
};

struct Resolved<'a> {
    variant: &'a EnumVariant,
    canonical: i32,
    alternates: Vec<i32>,
}

impl<'a> Resolved<'a> {
    fn from_variant(variant: &'a EnumVariant, spec: &LookupSpec) -> Self {
        let lookup_value = &spec.values[&variant.id];
        let (canonical, alternates) = match lookup_value {
            LookupValue::Single(n) => (*n, Vec::new()),
            LookupValue::Multi(list) => {
                let (&canonical, rest) = list.split_first().expect("multi-value non-empty by CUE");
                (canonical, rest.to_vec())
            }
        };

        Self {
            variant,
            canonical,
            alternates,
        }
    }
}

pub fn generate(
    id: &str,
    rules: &EnumRules,
    encoding: &EnumEncoding,
    default: Option<&str>,
) -> anyhow::Result<TokenStream> {
    let EnumEncoding::Lookup { spec, prop_code } = encoding;

    let resolved: Vec<_> = rules
        .variants
        .iter()
        .map(|v| Resolved::from_variant(v, spec))
        .collect();

    let wire_values: Vec<_> = resolved
        .iter()
        .flat_map(|r| std::iter::once(&r.canonical).chain(&r.alternates))
        .copied()
        .collect();

    let signed = resolve_enum_repr_signed(&wire_values)
        .with_context(|| format!("determining representation type for enum option `{id}`"))?;
    let repr_type = resolve_repr_type(signed);
    let repr_type_32 = resolve_repr_type_32(signed);

    let enum_def = generate_enum_def(
        &upper_camel_case_ident!("{}", id),
        &repr_type,
        signed,
        &resolved,
    )
    .with_context(|| format!("generating enum definition for enum option `{id}`"))?;
    let try_from_wire_impl = generate_try_from_wire_impl(
        &upper_camel_case_ident!("{}", id),
        signed,
        &repr_type,
        &wire_items(&resolved),
    )
    .with_context(|| format!("generating try_from_wire impl for enum option `{id}`"))?;
    let display_impl = generate_display_impl(&upper_camel_case_ident!("{}", id), &resolved);
    let from_str_impl = generate_from_str_impl(&upper_camel_case_ident!("{}", id), &resolved);
    let (ptp_serde_impl, simulation_setting_impl) = prop_code.as_ref().map_or_else(
        || (quote! {}, quote! {}),
        |prop_code| {
            let serde = generate_ptp_serde_impl(&upper_camel_case_ident!("{}", id), &repr_type);
            let setting =
                generate_simulation_setting_impl(&upper_camel_case_ident!("{}", id), *prop_code);
            (serde, setting)
        },
    );

    let conversion_profile_impl = generate_conversion_profile_impl(
        &upper_camel_case_ident!("{}", id),
        &repr_type,
        &repr_type_32,
    );

    let default_impl =
        generate_default_impl(&upper_camel_case_ident!("{}", id), &resolved, default);

    Ok(quote! {
        #enum_def
        #try_from_wire_impl
        #display_impl
        #from_str_impl
        #ptp_serde_impl
        #simulation_setting_impl
        #conversion_profile_impl
        #default_impl
    })
}

fn generate_default_impl(
    type_name: &Ident,
    resolved: &[Resolved<'_>],
    default: Option<&str>,
) -> TokenStream {
    let chosen = default.map_or_else(
        || &resolved[0],
        |want| {
            resolved
                .iter()
                .find(|r| r.variant.id == want)
                .expect("default is a known variant id")
        },
    );
    let variant = upper_camel_case_ident!("{}", chosen.variant.id);
    quote! {
        #[allow(clippy::derivable_impls)]
        impl ::std::default::Default for #type_name {
            fn default() -> Self {
                Self::#variant
            }
        }
    }
}

fn wire_items(resolved: &[Resolved<'_>]) -> Vec<(Ident, Vec<i32>)> {
    resolved
        .iter()
        .map(|r| {
            let mut wires = Vec::with_capacity(1 + r.alternates.len());
            wires.push(r.canonical);
            wires.extend(r.alternates.iter().copied());
            (upper_camel_case_ident!("{}", r.variant.id), wires)
        })
        .collect()
}

fn generate_enum_def(
    type_name: &Ident,
    repr_type: &Ident,
    signed: bool,
    resolved: &[Resolved<'_>],
) -> anyhow::Result<TokenStream> {
    let defs = resolved
        .iter()
        .map(|r| {
            let v = upper_camel_case_ident!("{}", r.variant.id);
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
            ::std::cmp::PartialOrd,
            ::std::cmp::Ord,
            ::std::hash::Hash,
            ::strum_macros::EnumIter,
            ::serde_with::SerializeDisplay,
            ::serde_with::DeserializeFromStr,
        )]
        pub enum #type_name {
            #(#defs)*
        }
    })
}

fn generate_display_impl(type_name: &Ident, resolved: &[Resolved<'_>]) -> TokenStream {
    let arms = resolved
        .iter()
        .map(|r| {
            let v = upper_camel_case_ident!("{}", r.variant.id);
            let display = &r.variant.name;
            quote! { Self::#v => write!(f, #display), }
        })
        .collect::<Vec<_>>();

    quote! {
        impl ::std::fmt::Display for #type_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    #(#arms)*
                }
            }
        }
    }
}

fn generate_from_str_impl(type_name: &Ident, resolved: &[Resolved<'_>]) -> TokenStream {
    let arms = resolved
        .iter()
        .map(|r| {
            let v = upper_camel_case_ident!("{}", r.variant.id);
            let aliases = &r.variant.aliases;
            quote! {
                #(#aliases)|* => return Ok(Self::#v),
            }
        })
        .collect::<Vec<_>>();

    quote! {
        impl ::std::str::FromStr for #type_name {
            type Err = crate::input::OptionError;
            fn from_str(s: &str) -> ::std::result::Result<Self, crate::input::OptionError> {
                match crate::input::CleanAlphanumeric::clean(&s).as_str() {
                    #(#arms)*
                    _ => {}
                }
                if let Some(best) = <Self as crate::input::Choices>::closest(s) {
                    return Err(crate::input::OptionError::UnknownWithSuggestion {
                        type_name: stringify!(#type_name),
                        input: s.to_string(),
                        suggestion: best,
                    });
                }
                Err(crate::input::OptionError::Unknown {
                    type_name: stringify!(#type_name),
                    input: s.to_string(),
                })
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
