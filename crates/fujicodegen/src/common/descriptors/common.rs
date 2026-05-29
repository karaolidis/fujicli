use proc_macro2::TokenStream;
use quote::quote;

use crate::{snake_case_ident, upper_camel_case_ident, uppercase_ident};

pub fn generate_descriptor(id: &str, name: &str, ops: &TokenStream) -> TokenStream {
    let type_ident = upper_camel_case_ident!("{}", id);
    let const_ident = uppercase_ident!("OPT_{}", id);
    let field_ident = snake_case_ident!("{}", id);

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
            ops: #ops,
        };

        impl crate::generated::options::#type_ident {
            pub const SIMULATION_DESCRIPTOR: &'static crate::features::simulation::OptionDescriptor<
                crate::generated::simulations::SimulationBase,
            > = &#const_ident;
        }
    }
}
