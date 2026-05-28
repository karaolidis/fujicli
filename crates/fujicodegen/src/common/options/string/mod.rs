use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::{
    ast::{StringEncoding, StringRules},
    upper_camel_case_ident,
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

pub fn generate(id: &str, rules: Option<&StringRules>, encoding: &StringEncoding) -> TokenStream {
    let StringEncoding::Raw { prop_code } = encoding;

    let bounds = Bounds::resolve(rules);
    let type_name = upper_camel_case_ident!("{}", id);

    let struct_def = generate_struct_def(&type_name);
    let inherent_impl = generate_inherent_impl(&type_name, &bounds);
    let from_str_impl = generate_from_str_impl(&type_name, &bounds);
    let display_impl = generate_display_impl(&type_name);
    let simulation_setting_impl = prop_code.as_ref().map_or_else(
        || quote! {},
        |code| generate_simulation_setting_impl(&type_name, *code),
    );

    quote! {
        #struct_def
        #inherent_impl
        #from_str_impl
        #display_impl
        #simulation_setting_impl
    }
}

fn generate_struct_def(type_name: &Ident) -> TokenStream {
    quote! {
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
    }
}

fn generate_inherent_impl(type_name: &Ident, bounds: &Bounds) -> TokenStream {
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

    quote! {
        impl #type_name {
            #const_block

            #[must_use]
            pub fn as_str(&self) -> &str { &self.0 }
        }
    }
}

fn generate_from_str_impl(type_name: &Ident, bounds: &Bounds) -> TokenStream {
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

    quote! {
        impl ::std::str::FromStr for #type_name {
            type Err = crate::input::OptionError;
            fn from_str(s: &str) -> ::std::result::Result<Self, crate::input::OptionError> {
                #validate_min
                #validate_max
                Ok(Self(s.to_string()))
            }
        }
    }
}

fn generate_display_impl(type_name: &Ident) -> TokenStream {
    quote! {
        impl ::std::fmt::Display for #type_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(&self.0)
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
