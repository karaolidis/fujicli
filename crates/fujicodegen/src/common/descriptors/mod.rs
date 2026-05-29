mod common;
mod r#enum;
mod float;
mod integer;
mod string;

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::{Camera, FujiOption, NumericEncoding, OptionSpec};

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> TokenStream {
    let simulation_fields = collect_simulation_field_ids(cameras);
    let mut blocks = Vec::with_capacity(simulation_fields.len());

    for (id, opt) in options {
        if !simulation_fields.contains(id.as_str()) {
            continue;
        }
        let name = opt.spec.name();
        let block = match &opt.spec {
            OptionSpec::Enum { rules, .. } => r#enum::generate(id, name, rules),
            OptionSpec::Integer { encoding, .. } => match encoding {
                NumericEncoding::Lookup { .. } => integer::lookup::generate(id, name),
                NumericEncoding::Raw { .. } | NumericEncoding::Scale { .. } => {
                    integer::scaled::generate(id, name)
                }
            },
            OptionSpec::Float { encoding, .. } => match encoding {
                NumericEncoding::Lookup { .. } => float::lookup::generate(id, name),
                NumericEncoding::Raw { .. } | NumericEncoding::Scale { .. } => {
                    float::scaled::generate(id, name)
                }
            },
            OptionSpec::String { rules, .. } => string::generate(id, name, rules.as_ref()),
        };
        blocks.push(block);
    }

    quote! {
        //! Generated option descriptor tables. Do not edit.

        #(#blocks)*
    }
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
