use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::EnumRules, common::descriptors::common::generate_descriptor, snake_case_ident,
    upper_camel_case_ident,
};

pub fn generate(id: &str, name: &str, rules: &EnumRules) -> TokenStream {
    let type_ident = upper_camel_case_ident!("{}", id);
    let field_ident = snake_case_ident!("{}", id);

    let variants = rules.variants.iter().map(|v| v.name.as_str());

    let ops = quote! {
        crate::features::simulation::OptionOps::Enum(
            crate::features::simulation::EnumOps {
                variants: &[#(#variants),*],
                cycle: |base, dir, validator| {
                    let ::std::option::Option::Some(current) = base.#field_ident else {
                        return ::std::result::Result::Err(
                            crate::features::simulation::BumpError::Unset,
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
                            crate::features::simulation::Direction::Next => (start + i) % n,
                            crate::features::simulation::Direction::Prev => (start + n - i) % n,
                        };
                        let mut candidate = base.clone();
                        candidate.#field_ident = ::std::option::Option::Some(variants[idx]);
                        if let ::std::option::Option::Some(v) = validator(candidate) {
                            *base = v;
                            return ::std::result::Result::Ok(());
                        }
                    }
                    ::std::result::Result::Err(crate::features::simulation::BumpError::Exhausted)
                },
                set_by_index: |base, idx, validator| {
                    let variants: ::std::vec::Vec<_> = <
                        crate::generated::options::#type_ident as ::strum::IntoEnumIterator
                    >::iter().collect();
                    let ::std::option::Option::Some(value) = variants.get(idx) else {
                        return crate::features::simulation::SetOutcome::Rejected;
                    };
                    let mut candidate = base.clone();
                    candidate.#field_ident = ::std::option::Option::Some(*value);
                    validator(candidate).map_or(
                        crate::features::simulation::SetOutcome::Rejected,
                        |v| {
                            *base = v;
                            crate::features::simulation::SetOutcome::Set
                        },
                    )
                },
                set_default: |base| {
                    base.#field_ident = ::std::option::Option::Some(
                        <crate::generated::options::#type_ident as ::std::default::Default>::default(),
                    );
                },
            }
        )
    };

    generate_descriptor(id, name, &ops)
}
