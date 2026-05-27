mod common;
mod r#enum;
mod float;
mod integer;
mod string;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{Camera, Field, FujiOption, NumericEncoding, OptionSpec},
    util::ident::safe_upper_camel_case_ident,
};

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
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
            } => string::generate(id, rules.as_ref(), encoding),
        };
        blocks.push(block);
    }

    let discriminant = generate_discriminant(options, cameras);

    let tokens = quote! {
        //! Generated option types. Do not edit.

        #(#blocks)*

        #discriminant
    };

    Ok(tokens)
}

fn generate_discriminant(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> TokenStream {
    let inline_ids: BTreeSet<String> = cameras
        .values()
        .filter_map(|c| c.spec.features.as_ref()?.render.as_ref())
        .flat_map(|r| r.fields.iter())
        .filter_map(|f| match f {
            Field::Inline(i) => Some(i.id.clone()),
            Field::Ref(_) => None,
        })
        .collect();

    let option_variants = options.keys().map(|id| {
        let ident = safe_upper_camel_case_ident(id);
        quote! { #ident, }
    });

    let inline_variants = inline_ids.iter().map(|id| {
        let ident = safe_upper_camel_case_ident(id);
        quote! { #ident, }
    });

    let option_impls = options.keys().map(|id| {
        let ident = safe_upper_camel_case_ident(id);
        quote! {
            impl #ident {
                pub const DISCRIMINANT: OptionDiscriminant = OptionDiscriminant::#ident;
            }
        }
    });

    quote! {
        #[derive(::std::fmt::Debug, ::std::clone::Clone, ::std::marker::Copy, ::std::cmp::PartialEq, ::std::cmp::Eq, ::std::hash::Hash)]
        pub enum OptionDiscriminant {
            #(#option_variants)*
            #(#inline_variants)*
        }

        #(#option_impls)*
    }
}

pub fn path() -> TokenStream {
    quote! { crate::generated::options }
}
