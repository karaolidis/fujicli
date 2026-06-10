use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::EnumRules,
    common::descriptors::{Target, common::generate_descriptor},
    snake_case_ident, upper_camel_case_ident,
};

pub fn generate(id: &str, name: &str, rules: &EnumRules, target: &Target) -> TokenStream {
    let type_ident = upper_camel_case_ident!("{}", id);
    let field_ident = snake_case_ident!("{}", id);

    let variant_entries = rules.variants.iter().map(|v| {
        let id = v.id.as_str();
        let name = v.name.as_str();
        quote! {
            crate::features::descriptor::VariantInfo { id: #id, name: #name }
        }
    });
    let variant_match_arms = rules.variants.iter().map(|v| {
        let id = v.id.as_str();
        let variant_ident = upper_camel_case_ident!("{}", v.id);
        quote! {
            #id => crate::generated::options::#type_ident::#variant_ident
        }
    });

    let ops = quote! {
        crate::features::descriptor::OptionOps::Enum(
            crate::features::descriptor::EnumOps {
                variants: &[#(#variant_entries),*],
                cycle: |base, dir, validator| {
                    let ::std::option::Option::Some(current) = base.#field_ident else {
                        return ::std::result::Result::Err(
                            crate::features::descriptor::BumpError::Unset,
                        );
                    };
                    let variants: ::std::vec::Vec<_> = <
                        crate::generated::options::#type_ident as ::strum::IntoEnumIterator
                    >::iter().collect();
                    let n = variants.len();
                    let start = variants
                        .iter()
                        .position(|v| *v == current)
                        .expect("current value is a variant");
                    for i in 1..n {
                        let idx = match dir {
                            crate::features::descriptor::Direction::Next => (start + i) % n,
                            crate::features::descriptor::Direction::Prev => (start + n - i) % n,
                        };
                        let want = variants[idx];
                        let mut candidate = base.clone();
                        candidate.#field_ident = ::std::option::Option::Some(want);
                        if let ::std::option::Option::Some(v) = validator(candidate)
                            && v.#field_ident == ::std::option::Option::Some(want)
                        {
                            *base = v;
                            return ::std::result::Result::Ok(());
                        }
                    }
                    ::std::result::Result::Err(crate::features::descriptor::BumpError::Exhausted)
                },
                set_by_id: |base, id, validator| {
                    let want = match id {
                        #( #variant_match_arms, )*
                        _ => return crate::features::descriptor::SetOutcome::Rejected,
                    };
                    let mut candidate = base.clone();
                    candidate.#field_ident = ::std::option::Option::Some(want);
                    if let ::std::option::Option::Some(v) = validator(candidate)
                        && v.#field_ident == ::std::option::Option::Some(want)
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

    generate_descriptor(id, name, &ops, true, target)
}
