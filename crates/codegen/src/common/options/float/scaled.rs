use anyhow::{Context, bail};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use super::super::common::{resolve_numeric_repr_signed, resolve_repr_type, resolve_repr_type_32};
use crate::{
    ast::{NumericEncoding, NumericRules},
    util::ident::safe_upper_camel_case_ident,
};

struct Bounds {
    min: f32,
    max: f32,
    step: f32,
    scale: i32,
}

impl Bounds {
    fn resolve(
        id: &str,
        rules: Option<&NumericRules<f32>>,
        encoding: &NumericEncoding,
    ) -> anyhow::Result<Self> {
        let min = rules.and_then(|r| r.min).unwrap_or(f32::MIN);
        let max = rules.and_then(|r| r.max).unwrap_or(f32::MAX);
        let step = rules.and_then(|r| r.step).unwrap_or(1.0);
        let scale = match encoding {
            NumericEncoding::Raw { .. } => 1,
            NumericEncoding::Scale { spec, .. } => spec.scale,
            NumericEncoding::Lookup { .. } => {
                bail!("float-lookup option `{id}` should use the lookup generator");
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

pub(crate) fn generate(
    id: &str,
    prop_code: Option<u16>,
    rules: Option<&NumericRules<f32>>,
    encoding: &NumericEncoding,
) -> anyhow::Result<TokenStream> {
    let bounds = Bounds::resolve(id, rules, encoding)
        .with_context(|| format!("resolving bounds for float option `{id}`"))?;

    let raw_min = (bounds.min * bounds.scale as f32).round() as i32;
    let raw_max = (bounds.max * bounds.scale as f32).round() as i32;
    let signed = resolve_numeric_repr_signed(raw_min, raw_max)
        .with_context(|| format!("determining representation type for float option `{id}`"))?;

    let repr_type = resolve_repr_type(signed);
    let repr_type_32 = resolve_repr_type_32(signed);

    let type_name = safe_upper_camel_case_ident(id);

    let struct_def = generate_struct_def(&type_name)
        .with_context(|| format!("generating struct definition for float option `{id}`"))?;
    let inherent_impl = generate_inherent_impl(&type_name, signed, &bounds)
        .with_context(|| format!("generating inherent impl for float option `{id}`"))?;
    let try_from_impl = generate_try_from_impl(&type_name)
        .with_context(|| format!("generating TryFrom<f32> impl for float option `{id}`"))?;
    let from_impl = generate_from_impl(&type_name).with_context(|| {
        format!("generating From<{type_name}> for f32 impl for float option `{id}`")
    })?;
    let display_impl = generate_display_impl(&type_name)
        .with_context(|| format!("generating Display impl for float option `{id}`"))?;
    let from_str_impl = generate_from_str_impl(&type_name)
        .with_context(|| format!("generating FromStr impl for float option `{id}`"))?;
    let serde_impls = generate_serde_impls(&type_name)
        .with_context(|| format!("generating Serde impls for float option `{id}`"))?;
    let simulation_setting_impl = if let Some(code) = prop_code {
        generate_simulation_setting_impl(&type_name, code)
            .with_context(|| format!("generating SimulationSetting impl for float option `{id}`"))?
    } else {
        quote! {}
    };
    let conversion_profile_impl =
        generate_conversion_profile_impl(&type_name, &repr_type, &repr_type_32).with_context(
            || format!("generating ConversionProfileField impl for float option `{id}`"),
        )?;
    Ok(quote! {
        #struct_def
        #inherent_impl
        #try_from_impl
        #from_impl
        #display_impl
        #from_str_impl
        #serde_impls
        #simulation_setting_impl
        #conversion_profile_impl
    })
}

fn generate_struct_def(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            ptp_macro::PtpSerialize,
            ptp_macro::PtpDeserialize,
        )]
        pub struct #type_name(i16);
    })
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
        pub const MIN: f32 = #min;
        pub const MAX: f32 = #max;
        pub const STEP: f32 = #step;
        pub const SCALE: i32 = #scale;
    };

    let raw = if signed {
        let raw_min: i16 = ((min * scale as f32) as i32).try_into()?;
        let raw_max: i16 = ((max * scale as f32) as i32).try_into()?;
        let raw_step: i16 = ((step * scale as f32) as i32).try_into()?;

        quote! {
            pub const RAW_MIN: i16 = #raw_min;
            pub const RAW_MAX: i16 = #raw_max;
            pub const RAW_STEP: i16 = #raw_step;
        }
    } else {
        let raw_min: u16 = ((min * scale as f32) as i32).try_into()?;
        let raw_max: u16 = ((max * scale as f32) as i32).try_into()?;
        let raw_step: u16 = ((step * scale as f32) as i32).try_into()?;

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

fn generate_try_from_impl(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::std::convert::TryFrom<f32> for #type_name {
            type Error = ::anyhow::Error;
            fn try_from(value: f32) -> ::anyhow::Result<Self> {
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    ::anyhow::bail!(
                        "{} value {} is out of range [{}, {}]",
                        stringify!(#type_name), value, Self::MIN, Self::MAX,
                    );
                }
                if (value - Self::MIN) % Self::STEP != 0.0 {
                    ::anyhow::bail!(
                        "{} value {} is not aligned to step {}",
                        stringify!(#type_name), value, Self::STEP,
                    );
                }
                let raw: i32 = (value * Self::SCALE as f32).round() as i32;
                let raw = raw.try_into()?;
                Ok(Self(raw))
            }
        }
    })
}

fn generate_from_impl(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::std::convert::From<#type_name> for f32 {
            fn from(value: #type_name) -> f32 {
                f32::from(value.0) / #type_name::SCALE as f32
            }
        }
    })
}

fn generate_display_impl(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::std::fmt::Display for #type_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", f32::from(*self))
            }
        }
    })
}

fn generate_from_str_impl(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::std::str::FromStr for #type_name {
            type Err = ::anyhow::Error;
            fn from_str(s: &str) -> ::anyhow::Result<Self> {
                let logical = crate::input::CleanAlphanumeric::clean(&s)
                    .parse::<f32>()
                    .map_err(|e| ::anyhow::anyhow!("Invalid numeric value '{}': {}", s, e))?;
                Self::try_from(logical)
            }
        }
    })
}

fn generate_serde_impls(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
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
                Self::try_from(logical).map_err(::serde::de::Error::custom)
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
    })
}
