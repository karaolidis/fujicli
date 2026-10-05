use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    ast::{Camera, FujiOption},
    common::{options, simulations},
    util::ident::safe_upper_camel_case_ident,
};

struct Entry {
    id: String,
    ident: proc_macro2::Ident,
    type_path: TokenStream,
}

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> TokenStream {
    let simulation_options = collect_simulation_option_ids(cameras);
    let entries = build_entries(options);

    let struct_def = generate_struct(&entries);
    let from_impl = generate_from_impl(&entries, &simulation_options);
    let prop_codes_const = generate_prop_codes_const(options, &simulation_options);

    quote! {
        #struct_def
        #from_impl
        #prop_codes_const
    }
}

fn collect_simulation_option_ids(cameras: &BTreeMap<String, Camera>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for camera in cameras.values() {
        let Some(simulation) = camera
            .spec
            .features
            .as_ref()
            .and_then(|f| f.simulation.as_ref())
        else {
            continue;
        };
        for setting in &simulation.settings {
            out.insert(setting.r#ref.clone());
        }
    }
    out
}

fn build_entries(options: &BTreeMap<String, FujiOption>) -> Vec<Entry> {
    options
        .values()
        .filter(|opt| !opt.codegen.skip_args)
        .map(|opt| {
            let ident = format_ident!("{}", opt.id);
            let type_ident = safe_upper_camel_case_ident(&opt.id);
            let options_path = options::path();
            let type_path = quote! { #options_path::#type_ident };
            Entry {
                id: opt.id.clone(),
                ident,
                type_path,
            }
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
        #[derive(::clap::Args, ::std::fmt::Debug, ::std::default::Default, ::std::clone::Clone)]
        pub struct SimulationArgs {
            #( #fields )*
        }
    }
}

fn generate_from_impl(entries: &[Entry], simulation_options: &BTreeSet<String>) -> TokenStream {
    let simulations_path = simulations::path();
    let mut fields: Vec<TokenStream> = Vec::new();
    let mut covered = 0usize;

    for entry in entries {
        if !simulation_options.contains(&entry.id) {
            continue;
        }
        let ident = &entry.ident;
        fields.push(quote! { #ident: args.#ident, });
        covered += 1;
    }

    let tail = if covered == simulation_options.len() {
        quote! {}
    } else {
        quote! { ..::std::default::Default::default() }
    };

    quote! {
        impl ::std::convert::From<SimulationArgs> for #simulations_path::SimulationBase {
            fn from(args: SimulationArgs) -> Self {
                Self {
                    #( #fields )*
                    #tail
                }
            }
        }
    }
}

fn generate_prop_codes_const(
    options: &BTreeMap<String, FujiOption>,
    simulation_options: &BTreeSet<String>,
) -> TokenStream {
    let prop_codes = collect_simulation_prop_codes(options, simulation_options);
    let prop_code_lits = prop_codes
        .iter()
        .map(|c| proc_macro2::Literal::u16_suffixed(*c));

    quote! {
        pub const SIMULATION_PROP_CODES: &[u16] = &[
            #( #prop_code_lits ),*
        ];
    }
}

fn collect_simulation_prop_codes(
    options: &BTreeMap<String, FujiOption>,
    simulation_options: &BTreeSet<String>,
) -> Vec<u16> {
    let mut codes: BTreeSet<u16> = BTreeSet::new();
    for id in simulation_options {
        let Some(option) = options.get(id) else {
            continue;
        };
        if let Some(code) = option.spec.prop_code() {
            codes.insert(code);
        }
    }
    codes.into_iter().collect()
}
