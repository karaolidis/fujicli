use proc_macro2::TokenStream;
use quote::quote;

use crate::{snake_case_ident, upper_camel_case_ident, uppercase_ident};

pub fn generate_descriptor(id: &str, name: &str, ops: &TokenStream, is_copy: bool) -> TokenStream {
    let type_ident = upper_camel_case_ident!("{}", id);
    let const_ident = uppercase_ident!("OPT_{}", id);
    let field_ident = snake_case_ident!("{}", id);

    let copy_from_body = if is_copy {
        quote! { |dst, src| { dst.#field_ident = src.#field_ident; } }
    } else {
        quote! { |dst, src| { dst.#field_ident.clone_from(&src.#field_ident); } }
    };

    quote! {
        pub const #const_ident: crate::features::simulation::OptionDescriptor<
            crate::generated::simulations::SimulationBase,
        > = crate::features::simulation::OptionDescriptor {
            name: #name,
            category: crate::generated::options::#type_ident::CATEGORY,
            display: |base| {
                base.#field_ident
                    .as_ref()
                    .map(::std::string::ToString::to_string)
            },
            copy_from: #copy_from_body,
            eq: |a, b| a.#field_ident == b.#field_ident,
            ops: #ops,
        };

        impl crate::generated::options::#type_ident {
            pub const SIMULATION_DESCRIPTOR: &'static crate::features::simulation::OptionDescriptor<
                crate::generated::simulations::SimulationBase,
            > = &#const_ident;
        }
    }
}
