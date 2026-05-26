use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::{
    ast::{StringEncoding, StringRules},
    util::ident::safe_upper_camel_case_ident,
};

struct Bounds {
    min_len: Option<u32>,
    max_len: Option<u32>,
}

impl Bounds {
    fn resolve(rules: Option<&StringRules>) -> Self {
        Self {
            min_len: rules.and_then(|r| r.min_length),
            max_len: rules.and_then(|r| r.max_length),
        }
    }
}

pub fn generate(
    id: &str,
    rules: Option<&StringRules>,
    encoding: &StringEncoding,
) -> anyhow::Result<TokenStream> {
    let StringEncoding::Raw { prop_code } = encoding;

    let bounds = Bounds::resolve(rules);
    let type_name = safe_upper_camel_case_ident(id);

    let struct_def = generate_struct_def(&type_name)
        .with_context(|| format!("generating struct definition for string option `{id}`"))?;
    let inherent_impl = generate_inherent_impl(&type_name, &bounds)
        .with_context(|| format!("generating inherent impl for string option `{id}`"))?;
    let from_str_impl = generate_from_str_impl(&type_name, &bounds)
        .with_context(|| format!("generating FromStr impl for string option `{id}`"))?;
    let display_impl = generate_display_impl(&type_name)
        .with_context(|| format!("generating Display impl for string option `{id}`"))?;
    let simulation_setting_impl = if let Some(code) = prop_code {
        generate_simulation_setting_impl(&type_name, *code).with_context(|| {
            format!("generating SimulationSetting impl for string option `{id}`")
        })?
    } else {
        quote! {}
    };

    Ok(quote! {
        #struct_def
        #inherent_impl
        #from_str_impl
        #display_impl
        #simulation_setting_impl
    })
}

#[allow(clippy::unnecessary_wraps)]
fn generate_struct_def(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        #[derive(
            ::std::fmt::Debug,
            ::std::clone::Clone,
            ::std::cmp::PartialEq,
            ::std::cmp::Eq,
            ::ptp_macro::PtpSerialize,
            ::ptp_macro::PtpDeserialize,
            ::serde_with::SerializeDisplay,
            ::serde_with::DeserializeFromStr,
        )]
        pub struct #type_name(String);
    })
}

#[allow(clippy::unnecessary_wraps)]
fn generate_inherent_impl(type_name: &Ident, bounds: &Bounds) -> anyhow::Result<TokenStream> {
    let const_block = match (bounds.min_len, bounds.max_len) {
        (Some(min), Some(max)) => quote! {
            pub const MIN_LEN: usize = #min as usize;
            pub const MAX_LEN: usize = #max as usize;
        },
        (None, Some(max)) => quote! {
            pub const MAX_LEN: usize = #max as usize;
        },
        (Some(min), None) => quote! {
            pub const MIN_LEN: usize = #min as usize;
        },
        (None, None) => quote! {},
    };

    Ok(quote! {
        impl #type_name {
            #const_block

            #[must_use]
            pub fn as_str(&self) -> &str { &self.0 }
        }
    })
}

#[allow(clippy::unnecessary_wraps)]
fn generate_from_str_impl(type_name: &Ident, bounds: &Bounds) -> anyhow::Result<TokenStream> {
    let validate_min = bounds.min_len.map(|min| {
        quote! {
            if s.chars().count() < #min as usize {
                return Err(crate::input::OptionError::TooShort {
                    type_name: stringify!(#type_name),
                    input: s.to_string(),
                    min: #min,
                });
            }
        }
    });

    let validate_max = bounds.max_len.map(|max| {
        quote! {
            if s.chars().count() > #max as usize {
                return Err(crate::input::OptionError::TooLong {
                    type_name: stringify!(#type_name),
                    input: s.to_string(),
                    max: #max,
                });
            }
        }
    });

    Ok(quote! {
        impl ::std::str::FromStr for #type_name {
            type Err = crate::input::OptionError;
            fn from_str(s: &str) -> ::std::result::Result<Self, crate::input::OptionError> {
                #validate_min
                #validate_max
                Ok(Self(s.to_string()))
            }
        }
    })
}

#[allow(clippy::unnecessary_wraps)]
fn generate_display_impl(type_name: &Ident) -> anyhow::Result<TokenStream> {
    Ok(quote! {
        impl ::std::fmt::Display for #type_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    })
}

#[allow(clippy::unnecessary_wraps)]
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
