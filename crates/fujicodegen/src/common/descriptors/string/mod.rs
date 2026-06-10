use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::StringRules,
    common::descriptors::{Target, common::generate_descriptor},
    snake_case_ident, upper_camel_case_ident,
};

pub fn generate(id: &str, name: &str, rules: Option<&StringRules>, target: &Target) -> TokenStream {
    let type_ident = upper_camel_case_ident!("{}", id);
    let field_ident = snake_case_ident!("{}", id);

    let max_len = if rules.is_some_and(|r| r.max_length.is_some()) {
        quote! {
            ::std::option::Option::Some(crate::generated::options::#type_ident::MAX_LEN)
        }
    } else {
        quote! { ::std::option::Option::None }
    };

    let ops = quote! {
        crate::features::descriptor::OptionOps::String(
            crate::features::descriptor::StringOps {
                max_len: #max_len,
                set_by_text: |base, text, validator| {
                    let parsed = match <
                        crate::generated::options::#type_ident as ::std::str::FromStr
                    >::from_str(text) {
                        ::std::result::Result::Ok(v) => v,
                        ::std::result::Result::Err(e) =>
                            return crate::features::descriptor::SetOutcome::InvalidInput(e),
                    };
                    let mut candidate = base.clone();
                    candidate.#field_ident = ::std::option::Option::Some(parsed);
                    let want = candidate.#field_ident.clone();
                    if let ::std::option::Option::Some(v) = validator(candidate)
                        && v.#field_ident == want
                    {
                        *base = v;
                        crate::features::descriptor::SetOutcome::Set
                    } else {
                        crate::features::descriptor::SetOutcome::Rejected
                    }
                },
                set_default: |base| {
                    base.#field_ident = ::std::option::Option::Some(
                        <crate::generated::options::#type_ident as ::std::default::Default>::default(),
                    );
                },
            }
        )
    };

    generate_descriptor(id, name, &ops, false, target)
}
