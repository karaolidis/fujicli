use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{Camera, FujiOption},
    schema::grammar::build_settings,
    snake_case_ident,
};

struct UnionEntry {
    id: String,
    type_path: TokenStream,
}

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
    let union = build_union(options, cameras)?;
    Ok(generate_struct_def(&union))
}

fn build_union(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<Vec<UnionEntry>> {
    let by_id = cameras
        .values()
        .filter_map(|camera| camera.spec.features.as_ref()?.simulation.as_ref())
        .try_fold(
            BTreeMap::<String, UnionEntry>::new(),
            |mut by_id, simulation| -> anyhow::Result<_> {
                let settings = build_settings(options, &simulation.settings);
                simulation.settings.iter().for_each(|setting| {
                    let id = setting.id.clone();
                    let info = &settings[id.as_str()];
                    by_id.entry(id.clone()).or_insert_with(|| UnionEntry {
                        id,
                        type_path: info.type_path(),
                    });
                });
                Ok(by_id)
            },
        )?;
    Ok(by_id.into_values().collect())
}

fn generate_struct_def(union: &[UnionEntry]) -> TokenStream {
    let base_fields = union.iter().map(|entry| {
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
        pub struct SimulationBase {
            #( #base_fields )*
        }
    }
}
