mod common;
mod r#enum;
mod float;
mod integer;
mod string;

use std::collections::BTreeMap;

use anyhow::Context;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::{FujiOption, NumericEncoding, OptionSpec};

pub fn generate(options: &BTreeMap<String, FujiOption>) -> anyhow::Result<TokenStream> {
    let mut blocks = Vec::with_capacity(options.len());

    for (id, opt) in options {
        let block = match &opt.spec {
            OptionSpec::Enum {
                rules, encoding, ..
            } => r#enum::generate(id, rules, encoding)
                .with_context(|| format!("generating enum option `{id}`"))?,
            OptionSpec::Integer {
                rules, encoding, ..
            } => match encoding {
                NumericEncoding::Lookup { spec, prop_code } => {
                    integer::lookup::generate(id, *prop_code, spec)
                        .with_context(|| format!("generating integer lookup option `{id}`"))?
                }
                NumericEncoding::Raw { prop_code, .. }
                | NumericEncoding::Scale { prop_code, .. } => {
                    integer::scaled::generate(id, *prop_code, rules.as_ref(), encoding)
                        .with_context(|| format!("generating integer option `{id}`"))?
                }
            },
            OptionSpec::Float {
                rules, encoding, ..
            } => match encoding {
                NumericEncoding::Lookup { spec, prop_code } => {
                    float::lookup::generate(id, *prop_code, spec)
                        .with_context(|| format!("generating float lookup option `{id}`"))?
                }
                NumericEncoding::Raw { prop_code, .. }
                | NumericEncoding::Scale { prop_code, .. } => {
                    float::scaled::generate(id, *prop_code, rules.as_ref(), encoding)
                        .with_context(|| format!("generating float option `{id}`"))?
                }
            },
            OptionSpec::String {
                rules, encoding, ..
            } => string::generate(id, rules.as_ref(), encoding)
                .with_context(|| format!("generating string option `{id}`"))?,
        };
        blocks.push(block);
    }

    let tokens = quote! {
        //! Generated option types. Do not edit.

        #(#blocks)*
    };

    Ok(tokens)
}

pub fn path() -> TokenStream {
    quote! { crate::generated::options }
}
