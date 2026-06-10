mod common;
mod r#enum;
mod float;
mod integer;
mod string;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::{
    ast::{FujiOption, NumericEncoding, OptionSpec},
    upper_camel_case_ident,
};

pub fn generate(options: &BTreeMap<String, FujiOption>) -> anyhow::Result<TokenStream> {
    let mut blocks = Vec::with_capacity(options.len() * 2);

    for (id, opt) in options {
        if opt.codegen.flaky {
            println!(
                "cargo:warning=option `{id}` is flaky: camera failures will be tolerated at runtime"
            );
        }

        let kind_block = match &opt.spec {
            OptionSpec::Enum {
                rules,
                encoding,
                default,
                ..
            } => r#enum::generate(id, rules, encoding, default.as_deref())
                .with_context(|| format!("generating enum option `{id}`"))?,
            OptionSpec::Integer {
                rules,
                encoding,
                default,
                ..
            } => match encoding {
                NumericEncoding::Lookup { spec, prop_code } => {
                    integer::lookup::generate(id, *prop_code, spec, *default)
                        .with_context(|| format!("generating integer lookup option `{id}`"))?
                }
                NumericEncoding::Raw { prop_code, .. }
                | NumericEncoding::Scale { prop_code, .. } => {
                    integer::scaled::generate(id, *prop_code, rules.as_ref(), encoding, *default)
                        .with_context(|| format!("generating integer option `{id}`"))?
                }
            },
            OptionSpec::Float {
                rules,
                encoding,
                default,
                ..
            } => match encoding {
                NumericEncoding::Lookup { spec, prop_code } => {
                    float::lookup::generate(id, *prop_code, spec, *default)
                        .with_context(|| format!("generating float lookup option `{id}`"))?
                }
                NumericEncoding::Raw { prop_code, .. }
                | NumericEncoding::Scale { prop_code, .. } => {
                    float::scaled::generate(id, *prop_code, rules.as_ref(), encoding, *default)
                        .with_context(|| format!("generating float option `{id}`"))?
                }
            },
            OptionSpec::String {
                rules,
                encoding,
                default,
                ..
            } => string::generate(id, rules.as_ref(), encoding, default.as_deref()),
        };
        blocks.push(kind_block);
        blocks.push(generate_option_category_const(id, opt.spec.category()));
    }

    let category_enum = generate_option_category_enum(options);

    let tokens = quote! {
        //! Generated option types. Do not edit.

        #(#blocks)*

        #category_enum
    };

    Ok(tokens)
}

pub fn path() -> TokenStream {
    quote! { crate::generated::options }
}

fn collect_categories(options: &BTreeMap<String, FujiOption>) -> BTreeMap<Ident, &str> {
    options
        .values()
        .filter_map(|o| o.spec.category())
        .collect::<BTreeSet<&str>>()
        .into_iter()
        .map(|cat| (upper_camel_case_ident!("{}", cat), cat))
        .collect()
}

fn generate_option_category_enum(options: &BTreeMap<String, FujiOption>) -> TokenStream {
    let by_ident = collect_categories(options);

    let variants = by_ident.keys().map(|ident| quote! { #ident, });
    let display_arms = by_ident.iter().map(|(ident, label)| {
        quote! { Self::#ident => f.write_str(#label), }
    });

    quote! {
        #[derive(
            ::std::fmt::Debug,
            ::std::clone::Clone,
            ::std::marker::Copy,
            ::std::cmp::PartialEq,
            ::std::cmp::Eq,
            ::std::hash::Hash,
        )]
        pub enum OptionCategory {
            #(#variants)*
        }

        impl ::std::fmt::Display for OptionCategory {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    #(#display_arms)*
                }
            }
        }
    }
}

fn generate_option_category_const(id: &str, category: Option<&str>) -> TokenStream {
    let type_name = upper_camel_case_ident!("{}", id);
    let value = category.map_or_else(
        || quote! { ::std::option::Option::None },
        |cat| {
            let variant = upper_camel_case_ident!("{}", cat);
            quote! { ::std::option::Option::Some(OptionCategory::#variant) }
        },
    );
    quote! {
        impl #type_name {
            pub const CATEGORY: ::std::option::Option<OptionCategory> = #value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enum_option(id: &str, category: Option<&str>) -> (String, FujiOption) {
        let json = serde_json::json!({
            "id": id,
            "spec": {
                "name": id,
                "kind": "enum",
                "category": category,
                "rules": { "variants": [{ "id": "a", "name": "A", "aliases": ["a"] }] },
                "encoding": { "kind": "lookup", "spec": { "values": { "a": 1 } } },
            },
        });
        let opt: FujiOption = serde_json::from_value(json).expect("fixture parses");
        (id.to_owned(), opt)
    }

    fn options(specs: &[(&str, Option<&str>)]) -> BTreeMap<String, FujiOption> {
        specs
            .iter()
            .map(|(id, cat)| enum_option(id, *cat))
            .collect()
    }

    #[test]
    fn collect_categories_dedupes_and_sorts() {
        let opts = options(&[
            ("a", Some("Tone")),
            ("b", Some("Detail")),
            ("c", Some("Tone")),
            ("d", None),
            ("e", Some("White Balance")),
        ]);
        let by_ident = collect_categories(&opts);
        let idents: Vec<String> = by_ident.keys().map(ToString::to_string).collect();
        assert_eq!(idents, vec!["Detail", "Tone", "WhiteBalance"]);
        assert_eq!(
            by_ident.values().copied().collect::<Vec<_>>(),
            vec!["Detail", "Tone", "White Balance"]
        );
    }
}
