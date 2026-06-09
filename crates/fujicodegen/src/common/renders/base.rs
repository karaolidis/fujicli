use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{Camera, FujiOption, SpecKind},
    common::simulations,
    schema::grammar::build_settings,
    snake_case_ident,
};

struct UnionEntry {
    id: String,
    type_path: TokenStream,
    is_copy: bool,
}

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
    let render_union = build_union(options, cameras)?;
    let simulation_field_ids = collect_simulation_field_ids(cameras);

    let struct_def = generate_struct_def(&render_union);
    let merge_impl = generate_merge_impl(&render_union);
    let apply_simulation_impl =
        generate_apply_simulation_impl(&render_union, &simulation_field_ids);

    Ok(quote! {
        #struct_def
        #merge_impl
        #apply_simulation_impl
    })
}

fn build_union(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<Vec<UnionEntry>> {
    let by_id = cameras
        .values()
        .filter_map(|camera| camera.spec.features.as_ref()?.render.as_ref())
        .try_fold(
            BTreeMap::<String, UnionEntry>::new(),
            |mut by_id, render| -> anyhow::Result<_> {
                let settings = build_settings(options, &render.fields);
                render.fields.iter().for_each(|field| {
                    let id = field.id().to_string();
                    let info = &settings[id.as_str()];
                    by_id.entry(id.clone()).or_insert_with(|| UnionEntry {
                        id,
                        type_path: info.type_path(),
                        is_copy: !matches!(info.kind, SpecKind::String),
                    });
                });
                Ok(by_id)
            },
        )?;
    Ok(by_id.into_values().collect())
}

fn collect_simulation_field_ids(cameras: &BTreeMap<String, Camera>) -> BTreeSet<String> {
    cameras
        .values()
        .filter_map(|camera| camera.spec.features.as_ref()?.simulation.as_ref())
        .flat_map(|simulation| simulation.settings.iter().map(|s| s.id.clone()))
        .collect()
}

fn generate_struct_def(union: &[UnionEntry]) -> TokenStream {
    let fields = union.iter().map(|entry| {
        let ident = snake_case_ident!("{}", entry.id);
        let ty = &entry.type_path;
        quote! {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub #ident: Option<#ty>,
        }
    });

    quote! {
        #[derive(
            ::std::fmt::Debug,
            ::std::clone::Clone,
            ::std::default::Default,
            ::serde::Serialize,
            ::serde::Deserialize,
        )]
        #[serde(default, rename_all = "camelCase")]
        pub struct RenderBase {
            #( #fields )*
        }
    }
}

fn generate_merge_impl(union: &[UnionEntry]) -> TokenStream {
    let assigns = union.iter().map(|entry| {
        let ident = snake_case_ident!("{}", entry.id);
        let access = if entry.is_copy {
            quote! { overlay.#ident }
        } else {
            quote! { overlay.#ident.clone() }
        };
        quote! {
            if let Some(value) = #access {
                self.#ident = Some(value);
            }
        }
    });
    let r#const = if union.iter().all(|entry| entry.is_copy) {
        quote! { const }
    } else {
        quote! {}
    };

    quote! {
        impl RenderBase {
            pub #r#const fn merge(&mut self, overlay: &Self) {
                #( #assigns )*
            }
        }
    }
}

fn generate_apply_simulation_impl(
    union: &[UnionEntry],
    simulation_field_ids: &BTreeSet<String>,
) -> TokenStream {
    let simulations_path = simulations::path();
    let entries: Vec<&UnionEntry> = union
        .iter()
        .filter(|entry| simulation_field_ids.contains(&entry.id))
        .collect();
    let assigns = entries.iter().map(|entry| {
        let ident = snake_case_ident!("{}", entry.id);
        let access = if entry.is_copy {
            quote! { simulation.#ident }
        } else {
            quote! { simulation.#ident.clone() }
        };

        quote! {
            if let Some(value) = #access {
                self.#ident = Some(value);
            }
        }
    });
    let r#const = if entries.iter().all(|entry| entry.is_copy) {
        quote! { const }
    } else {
        quote! {}
    };

    quote! {
        impl RenderBase {
            pub #r#const fn try_update_from(
                &mut self,
                simulation: &#simulations_path::SimulationBase,
            ) {
                #( #assigns )*
            }
        }
    }
}
