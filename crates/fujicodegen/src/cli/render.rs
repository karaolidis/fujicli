use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    ast::{Camera, Field, FujiOption},
    common::{options, renders},
    util::ident::safe_upper_camel_case_ident,
};

struct Entry {
    ident: proc_macro2::Ident,
    type_path: TokenStream,
}

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
    let render_options = collect_render_option_ids(cameras);
    let render_inline_ids = collect_render_inline_ids(cameras);
    let entries = build_entries(options, &render_options, &render_inline_ids);

    let struct_def = generate_struct(&entries);
    let from_impl = generate_from_impl(&entries);

    Ok(quote! {
        #struct_def
        #from_impl
    })
}

fn collect_render_option_ids(cameras: &BTreeMap<String, Camera>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for camera in cameras.values() {
        let Some(render) = camera
            .spec
            .features
            .as_ref()
            .and_then(|f| f.render.as_ref())
        else {
            continue;
        };
        for field in &render.fields {
            if let Field::Ref(r) = field {
                out.insert(r.r#ref.clone());
            }
        }
    }
    out
}

fn collect_render_inline_ids(cameras: &BTreeMap<String, Camera>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for camera in cameras.values() {
        let Some(render) = camera
            .spec
            .features
            .as_ref()
            .and_then(|f| f.render.as_ref())
        else {
            continue;
        };
        for field in &render.fields {
            if let Field::Inline(i) = field {
                out.insert(i.id.clone());
            }
        }
    }
    out
}

fn build_entries(
    options: &BTreeMap<String, FujiOption>,
    render_options: &BTreeSet<String>,
    render_inline_ids: &BTreeSet<String>,
) -> Vec<Entry> {
    options
        .values()
        .filter(|opt| !opt.codegen.skip_args)
        .filter(|opt| render_options.contains(&opt.id) && !render_inline_ids.contains(&opt.id))
        .map(|opt| {
            let ident = format_ident!("{}", opt.id);
            let type_ident = safe_upper_camel_case_ident(&opt.id);
            let options_path = options::path();
            let type_path = quote! { #options_path::#type_ident };
            Entry { ident, type_path }
        })
        .collect()
}

fn generate_struct(entries: &[Entry]) -> TokenStream {
    let fields = entries.iter().map(|entry| {
        let ident = &entry.ident;
        let ty = &entry.type_path;
        let attrs = quote! { #[clap(long, allow_hyphen_values(true))] };
        quote! {
            #attrs
            pub #ident: Option<#ty>,
        }
    });

    quote! {
        #[derive(::clap::Args, Debug, Default, Clone)]
        pub struct RenderArgs {
            #( #fields )*
        }
    }
}

fn generate_from_impl(entries: &[Entry]) -> TokenStream {
    let renders_path = renders::path();
    let fields = entries.iter().map(|entry| {
        let ident = &entry.ident;
        quote! { #ident: args.#ident, }
    });

    quote! {
        impl ::std::convert::From<RenderArgs> for #renders_path::RenderBase {
            fn from(args: RenderArgs) -> Self {
                Self {
                    #( #fields )*
                    ..::std::default::Default::default()
                }
            }
        }
    }
}
