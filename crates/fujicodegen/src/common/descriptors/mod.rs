mod common;
mod r#enum;
mod float;
mod integer;
mod string;

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::{Camera, FujiOption, NumericEncoding, OptionSpec};

pub struct Target {
    pub base: TokenStream,
    pub const_prefix: &'static str,
    pub inherent: &'static str,
}

impl Target {
    fn simulation() -> Self {
        Self {
            base: quote! { crate::generated::simulations::SimulationBase },
            const_prefix: "SIMULATION_OPT",
            inherent: "SIMULATION_DESCRIPTOR",
        }
    }

    fn render() -> Self {
        Self {
            base: quote! { crate::generated::renders::RenderBase },
            const_prefix: "RENDER_OPT",
            inherent: "RENDER_DESCRIPTOR",
        }
    }
}

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> TokenStream {
    let simulation_blocks = generate_blocks(
        options,
        &collect_simulation_field_ids(cameras),
        &Target::simulation(),
    );
    let render_blocks = generate_blocks(
        options,
        &collect_render_field_ids(cameras),
        &Target::render(),
    );

    quote! {
        //! Generated option descriptor tables. Do not edit.

        #(#simulation_blocks)*
        #(#render_blocks)*
    }
}

fn generate_blocks(
    options: &BTreeMap<String, FujiOption>,
    field_ids: &BTreeSet<&str>,
    target: &Target,
) -> Vec<TokenStream> {
    let mut blocks = Vec::with_capacity(field_ids.len());
    for (id, opt) in options {
        if !field_ids.contains(id.as_str()) {
            continue;
        }
        let name = opt.spec.name();
        let block = match &opt.spec {
            OptionSpec::Enum { rules, .. } => r#enum::generate(id, name, rules, target),
            OptionSpec::Integer { encoding, .. } => match encoding {
                NumericEncoding::Lookup { .. } => integer::lookup::generate(id, name, target),
                NumericEncoding::Raw { .. } | NumericEncoding::Scale { .. } => {
                    integer::scaled::generate(id, name, target)
                }
            },
            OptionSpec::Float { encoding, .. } => match encoding {
                NumericEncoding::Lookup { .. } => float::lookup::generate(id, name, target),
                NumericEncoding::Raw { .. } | NumericEncoding::Scale { .. } => {
                    float::scaled::generate(id, name, target)
                }
            },
            OptionSpec::String { rules, .. } => string::generate(id, name, rules.as_ref(), target),
        };
        blocks.push(block);
    }
    blocks
}

#[allow(dead_code)]
pub fn path() -> TokenStream {
    quote! { crate::generated::descriptors }
}

fn collect_simulation_field_ids(cameras: &BTreeMap<String, Camera>) -> BTreeSet<&str> {
    cameras
        .values()
        .filter_map(|c| c.spec.features.as_ref()?.simulation.as_ref())
        .flat_map(|s| s.settings.iter().map(|setting| setting.id.as_str()))
        .collect()
}

fn collect_render_field_ids(cameras: &BTreeMap<String, Camera>) -> BTreeSet<&str> {
    cameras
        .values()
        .filter_map(|c| c.spec.features.as_ref()?.render.as_ref())
        .flat_map(|r| r.fields.iter().map(crate::ast::Field::id))
        .collect()
}
