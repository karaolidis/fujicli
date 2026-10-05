use anyhow::{Context, bail};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use super::super::common::{resolve_numeric_repr_signed, resolve_repr_type, resolve_repr_type_32};
use crate::{
    ast::{NumericEncoding, NumericRules},
    util::ident::safe_upper_camel_case_ident,
};

struct Bounds {
    min: i32,
    max: i32,
    step: i32,
    scale: i32,
}

impl Bounds {
    fn resolve(
        id: &str,
        rules: Option<&NumericRules<i32>>,
        encoding: &NumericEncoding,
    ) -> anyhow::Result<Self> {
        let min = rules.and_then(|r| r.min).unwrap_or(i32::MIN);
        let max = rules.and_then(|r| r.max).unwrap_or(i32::MAX);
        let step = rules.and_then(|r| r.step).unwrap_or(1);
        let scale: i32 = match encoding {
            NumericEncoding::Raw { .. } => 1,
            NumericEncoding::Scale { spec, .. } => spec.scale,
            NumericEncoding::Lookup { .. } => {
                bail!("integer-lookup option `{id}` should use the lookup generator");
            }
        };

        Ok(Self {
            min,
            max,
            step,
            scale,
        })
    }
}

pub fn generate(
    id: &str,
    prop_code: Option<u16>,
    rules: Option<&NumericRules<i32>>,
    encoding: &NumericEncoding,
) -> anyhow::Result<TokenStream> {
    let bounds = Bounds::resolve(id, rules, encoding)
        .with_context(|| format!("resolving bounds for integer option `{id}`"))?;

    let signed = resolve_numeric_repr_signed(bounds.min, bounds.max)
        .with_context(|| format!("determining representation type for integer option `{id}`"))?;

    let repr_type = resolve_repr_type(signed);
    let repr_type_32 = resolve_repr_type_32(signed);

    let type_name = safe_upper_camel_case_ident(id);

    let struct_def = generate_struct_def(&type_name, &repr_type);
    let inherent_impl = generate_inherent_impl(&type_name, signed, &bounds)
        .with_context(|| format!("generating inherent impl for integer option `{id}`"))?;
    let try_from_impl = generate_try_from_impl(&type_name, &repr_type, bounds.step);
    let to_impl = generate_to_impl(&type_name);
    let display_impl = generate_display_impl(&type_name);
    let from_str_impl = generate_from_str_impl(&type_name);
    let serde_impls = generate_serde_impls(&type_name);
    let simulation_setting_impl = prop_code.map_or_else(
        || quote! {},
        |code| generate_simulation_setting_impl(&type_name, code),
    );
    let conversion_profile_impl =
        generate_conversion_profile_impl(&type_name, &repr_type, &repr_type_32);

    Ok(quote! {
        #struct_def
        #inherent_impl
        #try_from_impl
        #to_impl
        #display_impl
        #from_str_impl
        #serde_impls
        #simulation_setting_impl
        #conversion_profile_impl
    })
}

fn generate_struct_def(type_name: &Ident, repr_type: &Ident) -> TokenStream {
    quote! {
        #[derive(
            ::std::fmt::Debug,
            ::std::clone::Clone,
            ::std::marker::Copy,
            ::std::cmp::PartialEq,
            ::std::cmp::Eq,
            ::ptp_macro::PtpSerialize,
            ::ptp_macro::PtpDeserialize,
        )]
        pub struct #type_name(#repr_type);
    }
}

fn generate_inherent_impl(
    type_name: &Ident,
    signed: bool,
    bounds: &Bounds,
) -> anyhow::Result<TokenStream> {
    let Bounds {
        min,
        max,
        step,
        scale,
    } = *bounds;

    let logical = quote! {
        pub const MIN: i32 = #min;
        pub const MAX: i32 = #max;
        pub const STEP: i32 = #step;
        pub const SCALE: i32 = #scale;
    };

    let raw = if signed {
        let raw_min: i16 = (min * scale).try_into()?;
        let raw_max: i16 = (max * scale).try_into()?;
        let raw_step: i16 = (step * scale).try_into()?;

        quote! {
            pub const RAW_MIN: i16 = #raw_min;
            pub const RAW_MAX: i16 = #raw_max;
            pub const RAW_STEP: i16 = #raw_step;
        }
    } else {
        let raw_min: u16 = (min * scale).try_into()?;
        let raw_max: u16 = (max * scale).try_into()?;
        let raw_step: u16 = (step * scale).try_into()?;

        quote! {
            pub const RAW_MIN: u16 = #raw_min;
            pub const RAW_MAX: u16 = #raw_max;
            pub const RAW_STEP: u16 = #raw_step;
        }
    };

    Ok(quote! {
        impl #type_name {
            #logical
            #raw
        }
    })
}

fn generate_try_from_impl(type_name: &Ident, repr_type: &Ident, step: i32) -> TokenStream {
    let step_check = if step == 1 {
        quote! {}
    } else {
        quote! {
            if (value - Self::MIN) % Self::STEP != 0 {
                return Err(crate::input::OptionError::StepMisaligned {
                    type_name: stringify!(#type_name),
                    value: value.to_string(),
                    step: Self::STEP.to_string(),
                });
            }
        }
    };

    quote! {
        impl ::std::convert::TryFrom<i32> for #type_name {
            type Error = crate::input::OptionError;
            fn try_from(value: i32) -> ::std::result::Result<Self, crate::input::OptionError> {
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    return Err(crate::input::OptionError::OutOfRange {
                        type_name: stringify!(#type_name),
                        value: value.to_string(),
                        min: Self::MIN.to_string(),
                        max: Self::MAX.to_string(),
                    });
                }
                #step_check
                let raw = value * Self::SCALE;
                let raw = raw.try_into().map_err(|_| {
                    crate::input::OptionError::WireOverflow {
                        type_name: stringify!(#type_name),
                        raw: raw.to_string(),
                        repr: stringify!(#repr_type),
                    }
                })?;
                Ok(Self(raw))
            }
        }
    }
}

fn generate_to_impl(type_name: &Ident) -> TokenStream {
    quote! {
        impl ::std::convert::From<#type_name> for i32 {
            fn from(value: #type_name) -> Self {
                Self::from(value.0) / #type_name::SCALE
            }
        }
    }
}

fn generate_display_impl(type_name: &Ident) -> TokenStream {
    quote! {
        impl ::std::fmt::Display for #type_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", i32::from(*self))
            }
        }
    }
}

fn generate_from_str_impl(type_name: &Ident) -> TokenStream {
    quote! {
        impl ::std::str::FromStr for #type_name {
            type Err = crate::input::OptionError;
            fn from_str(s: &str) -> ::std::result::Result<Self, crate::input::OptionError> {
                let logical = crate::input::CleanAlphanumeric::clean(&s)
                    .parse::<i32>()
                    .map_err(|e: ::std::num::ParseIntError| crate::input::OptionError::InvalidValue {
                        type_name: stringify!(#type_name),
                        input: s.to_string(),
                        reason: e.to_string(),
                    })?;
                Self::try_from(logical)
            }
        }
    }
}

fn generate_serde_impls(type_name: &Ident) -> TokenStream {
    quote! {
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
                Self::try_from(logical).map_err(::serde::de::Error::custom)
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
                ::ptp_cursor::PtpSerialize::try_write_ptp(&#repr_type_32::from(self.0), buf)
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
                Ok(Self(raw))
            }
        }
    }
}
