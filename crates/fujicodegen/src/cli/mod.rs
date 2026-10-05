pub mod render;
pub mod simulation;

use std::collections::BTreeMap;

use anyhow::Context;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::{Camera, FujiOption};

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
    let simulation = simulation::generate(options, cameras).context("generating SimulationArgs")?;
    let render = render::generate(options, cameras).context("generating RenderArgs")?;

    Ok(quote! {
        //! Generated CLI types. Do not edit.

        #simulation
        #render
    })
}
