pub mod render;
pub mod simulation;

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::{Camera, FujiOption};

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> TokenStream {
    let simulation = simulation::generate(options, cameras);
    let render = render::generate(options, cameras);

    quote! {
        //! Generated CLI types. Do not edit.

        #simulation
        #render
    }
}
