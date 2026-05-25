pub mod base;
pub mod camera;

use std::collections::BTreeMap;

use anyhow::Context;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::{Camera, FujiOption};

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
    let base_struct = base::generate(options, cameras).context("generating RenderBase")?;
    let per_camera =
        camera::generate(options, cameras).context("generating per-camera render structs")?;

    Ok(quote! {
        //! Generated render profile types. Do not edit.

        #base_struct
        #per_camera
    })
}

pub fn path() -> TokenStream {
    quote! { crate::generated::renders }
}
