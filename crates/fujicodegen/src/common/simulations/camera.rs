use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::{
    ast::{Camera, Dnf, FujiOption, Leaf, Setting},
    common::{cameras, options},
    schema::{
        alias::{NormalizedRule, NormalizedTransformation},
        grammar::{
            Scopes, SettingInfo, build_settings, generate_apply_transformations, generate_dnf,
            generate_emit_warnings_and_infos,
        },
        presence::PresenceDag,
        repair::{generate_pin_set, generate_solve},
    },
    util::{dag::Dag, ident::safe_upper_camel_case_ident},
};

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
    let base_union = collect_base_union(cameras);
    let mut blocks = Vec::with_capacity(cameras.len());
    for camera in cameras.values() {
        let block = generate_one(options, camera, &base_union)
            .with_context(|| format!("generating simulation for camera `{}`", camera.id))?;
        blocks.push(block);
    }
    Ok(quote! { #( #blocks )* })
}

fn collect_base_union(cameras: &BTreeMap<String, Camera>) -> BTreeSet<String> {
    cameras
        .values()
        .filter_map(|c| c.spec.features.as_ref()?.simulation.as_ref())
        .flat_map(|s| s.settings.iter().map(|setting| setting.id.clone()))
        .collect()
}

fn generate_one(
    options: &BTreeMap<String, FujiOption>,
    camera: &Camera,
    base_union: &BTreeSet<String>,
) -> anyhow::Result<TokenStream> {
    let Some(simulation) = camera
        .spec
        .features
        .as_ref()
        .and_then(|f| f.simulation.as_ref())
    else {
        return Ok(quote! {});
    };

    let settings = build_settings(options, &simulation.settings);

    let aliases: Vec<NormalizedTransformation> = simulation
        .transformations
        .iter()
        .cloned()
        .filter_map(Option::from)
        .collect();
    let effective_rules: Vec<NormalizedRule> = simulation
        .rules
        .iter()
        .map(|r| NormalizedRule::from_rule(r, &aliases))
        .collect();

    let struct_ident = format_ident!("{}Simulation", safe_upper_camel_case_ident(&camera.id));
    let camera_struct_ident = safe_upper_camel_case_ident(&camera.id);
    let cameras_path = cameras::path();
    let camera_struct_path = quote! { #cameras_path::#camera_struct_ident };
    let options_path = options::path();

    let presence_info = PresenceDag::try_from_rules(&effective_rules)
        .with_context(|| format!("extracting presence DAG for `{}`", camera.id))?;

    let nodes: Vec<&str> = simulation.settings.iter().map(|s| s.id.as_str()).collect();
    let edges: Vec<(&str, &str)> = presence_info
        .edges
        .iter()
        .map(|(from, to)| (from.as_str(), to.as_str()))
        .collect();

    let write_order: Vec<String> = Dag::new(nodes, edges)
        .topological_order()?
        .into_iter()
        .map(str::to_owned)
        .collect();
    let read_order = write_order.clone();

    let struct_def = generate_struct_def(&settings, &simulation.settings, &struct_ident);
    let inherent_impl = generate_inherent_impl(
        &settings,
        simulation,
        &effective_rules,
        &struct_ident,
        &options_path,
    )?;
    let from_sim_impl =
        generate_from_sim_for_base_impl(&settings, &simulation.settings, &struct_ident, base_union);
    let try_from_base_impl = generate_try_from_base_impl(
        &settings,
        &simulation.settings,
        &effective_rules,
        &struct_ident,
    );
    let display_impl = generate_display_impl(&settings, &simulation.settings, &struct_ident);
    let simulation_impl = generate_simulation_impl(
        &settings,
        &struct_ident,
        &options_path,
        &read_order,
        &write_order,
        &presence_info.conditions,
    )?;
    let parser_impl = generate_parser_impl(&struct_ident, &camera_struct_path);
    let manager_impl = generate_manager_impl(&struct_ident, &camera_struct_path, &options_path);

    Ok(quote! {
        #struct_def
        #inherent_impl
        #from_sim_impl
        #try_from_base_impl
        #display_impl
        #simulation_impl
        #parser_impl
        #manager_impl
    })
}

fn generate_struct_def(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    struct_ident: &Ident,
) -> TokenStream {
    let field_defs = fields.iter().map(|s| {
        let info = settings.get(s.id.as_str()).expect("settings indexed");
        let ident = info.field_ident();
        let type_path = info.type_path();
        quote! {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub #ident: Option<#type_path>,
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
        pub struct #struct_ident {
            #( #field_defs )*
        }
    }
}

fn generate_inherent_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    simulation: &crate::ast::Simulation,
    effective_rules: &[NormalizedRule],
    struct_ident: &Ident,
    options_path: &TokenStream,
) -> anyhow::Result<TokenStream> {
    let slots = simulation.slots;

    let apply_transformations =
        generate_apply_transformations(settings, &simulation.transformations)?;
    let self_acc = quote! { self };
    let warnings_infos =
        generate_emit_warnings_and_infos(settings, effective_rules, Scopes::new(&self_acc))?;
    let solve = generate_solve(settings, effective_rules, false)?;
    let try_update_from = generate_try_update_from(settings, &simulation.settings);
    let name = generate_name(settings, options_path);

    Ok(quote! {
        impl #struct_ident {
            pub const SLOTS: u32 = #slots;

            #apply_transformations
            #warnings_infos
            #solve
            #try_update_from
            #name
        }
    })
}

fn generate_try_update_from(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
) -> TokenStream {
    let init_fields = fields.iter().map(|s| {
        let info = settings.get(s.id.as_str()).expect("settings indexed");
        let ident = info.field_ident();
        let value = if info.is_copy {
            quote! { partial.#ident }
        } else {
            quote! { partial.#ident.clone() }
        };
        quote! { #ident: #value, }
    });

    let merge_assigns = fields.iter().map(|s| {
        let info = settings.get(s.id.as_str()).expect("settings indexed");
        let ident = info.field_ident();
        quote! {
            if let Some(value) = partial_profile.#ident.take() {
                candidate.#ident = Some(value);
            }
        }
    });

    let pin_set_expr = generate_pin_set(settings, &quote! { partial_profile });

    quote! {
        pub fn try_update_from(
            &mut self,
            partial: &crate::generated::simulations::SimulationBase,
        ) -> ::std::result::Result<(), crate::features::simulation::SimulationError> {
            let mut partial_profile = Self {
                #( #init_fields )*
            };
            partial_profile.apply_transformations();

            let pin = #pin_set_expr;

            let mut candidate = self.clone();
            #( #merge_assigns )*
            candidate.apply_transformations();

            candidate.solve(&pin)?;
            candidate.emit_warnings_and_infos()?;

            *self = candidate;
            Ok(())
        }
    }
}

fn generate_name(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    options_path: &TokenStream,
) -> TokenStream {
    let body = if settings.contains_key("custom_setting_name") {
        quote! { self.custom_setting_name.clone() }
    } else {
        quote! { None }
    };
    quote! {
        #[must_use]
        pub fn name(&self) -> Option<#options_path::CustomSettingName> {
            #body
        }
    }
}

fn generate_from_sim_for_base_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    struct_ident: &Ident,
    base_union: &BTreeSet<String>,
) -> TokenStream {
    let init_fields = fields.iter().map(|s| {
        let info = settings.get(s.id.as_str()).expect("settings indexed");
        let ident = info.field_ident();
        let value = if matches!(info.kind, crate::ast::SpecKind::String) {
            quote! { simulation.#ident.clone() }
        } else {
            quote! { simulation.#ident }
        };
        quote! { #ident: #value, }
    });

    let tail = if fields.len() == base_union.len() {
        TokenStream::new()
    } else {
        quote! { ..::std::default::Default::default() }
    };

    quote! {
        impl ::std::convert::From<&#struct_ident>
            for crate::generated::simulations::SimulationBase
        {
            fn from(simulation: &#struct_ident) -> Self {
                Self {
                    #( #init_fields )*
                    #tail
                }
            }
        }
    }
}

fn generate_try_from_base_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    rules: &[NormalizedRule],
    struct_ident: &Ident,
) -> TokenStream {
    let mut optional_fields: BTreeSet<String> = BTreeSet::new();
    for rule in rules {
        for conj in &rule.when {
            for leaf in conj {
                if let Leaf::Present(p) = leaf
                    && !p.present
                {
                    optional_fields.insert(p.r#ref.clone());
                }
            }
        }
    }

    let struct_name = struct_ident.to_string();
    let required_checks =
        generate_required_field_checks(settings, fields, &optional_fields, &struct_name);

    quote! {
        impl ::std::convert::TryFrom<crate::generated::simulations::SimulationBase>
            for #struct_ident
        {
            type Error = crate::features::simulation::SimulationError;
            fn try_from(
                base: crate::generated::simulations::SimulationBase,
            ) -> ::std::result::Result<Self, crate::features::simulation::SimulationError> {
                let mut sim = Self::default();
                sim.try_update_from(&base)?;
                #required_checks
                Ok(sim)
            }
        }
    }
}

fn generate_required_field_checks(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    optional: &BTreeSet<String>,
    struct_name: &str,
) -> TokenStream {
    let parts = fields.iter().filter_map(|s| {
        let id = s.id.as_str();
        if optional.contains(id) {
            return None;
        }
        let info = settings.get(id).expect("settings indexed");
        let ident = info.field_ident();
        let id_str = id.to_string();
        Some(quote! {
            if sim.#ident.is_none() {
                return Err(crate::features::simulation::SimulationError::MissingField {
                    simulation: #struct_name,
                    field: #id_str,
                });
            }
        })
    });
    quote! { #( #parts )* }
}

fn generate_display_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    struct_ident: &Ident,
) -> TokenStream {
    let lines = fields.iter().map(|s| {
        let info = settings.get(s.id.as_str()).expect("settings indexed");
        let ident = info.field_ident();
        let label = info
            .option
            .map_or_else(|| info.id.to_string(), |o| o.spec.name().to_string());
        let escaped = label.replace('{', "{{").replace('}', "}}");
        let fmt = format!("{escaped}: {{value}}");
        quote! {
            if let Some(value) = self.#ident.as_ref() {
                writeln!(f, #fmt)?;
            }
        }
    });
    quote! {
        impl ::std::fmt::Display for #struct_ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                #( #lines )*
                Ok(())
            }
        }
    }
}

fn generate_simulation_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    struct_ident: &Ident,
    options_path: &TokenStream,
    read_order: &[String],
    write_order: &[String],
    presence_conditions: &BTreeMap<String, Dnf>,
) -> anyhow::Result<TokenStream> {
    let try_pull = generate_try_pull(settings, read_order, presence_conditions)?;
    let try_push = generate_try_push(settings, write_order);

    Ok(quote! {
        impl crate::features::simulation::Simulation for #struct_ident {
            fn as_any(&self) -> &dyn ::std::any::Any { self }

            fn name(&self) -> Option<#options_path::CustomSettingName> {
                <Self>::name(self)
            }

            fn try_update_from(
                &mut self,
                partial: &crate::generated::simulations::SimulationBase,
            ) -> crate::error::CoreResult<()> {
                <Self>::try_update_from(self, partial)?;
                Ok(())
            }

            #try_pull
            #try_push

            fn to_base(&self) -> crate::generated::simulations::SimulationBase {
                <crate::generated::simulations::SimulationBase as ::std::convert::From<&Self>>::from(self)
            }
        }
    })
}

fn generate_try_pull(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    read_order: &[String],
    presence_conditions: &BTreeMap<String, Dnf>,
) -> anyhow::Result<TokenStream> {
    let staging_accessor = quote! { staged };
    let reads = read_order
        .iter()
        .map(|id| {
            let info = settings
                .get(id.as_str())
                .expect("read order references known setting");
            let ident = info.field_ident();
            let type_path = info.type_path();

            let read_call = quote! {
                Some(<#type_path as crate::ptp::option::SimulationSetting>::try_pull(ptp)?)
            };

            let body = if let Some(dnf) = presence_conditions.get(id) {
                let cond = generate_dnf(settings, dnf, Scopes::new(&staging_accessor))?;
                quote! {
                    if #cond { #read_call } else { None }
                }
            } else {
                read_call
            };

            Ok(quote! {
                staged.#ident = #body;
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        fn try_pull(
            ptp: &mut crate::ptp::Ptp,
        ) -> crate::error::CoreResult<Self> {
            let mut staged = Self::default();
            #( #reads )*
            Ok(staged)
        }
    })
}

fn generate_try_push(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    write_order: &[String],
) -> TokenStream {
    let writes = write_order.iter().map(|id| {
        let info = settings
            .get(id.as_str())
            .expect("write order references known setting");
        let ident = info.field_ident();

        quote! {
            if let Some(value) = self.#ident.as_ref() {
                crate::ptp::option::SimulationSetting::try_push(value, ptp)?;
            }
        }
    });

    quote! {
        fn try_push(
            &self,
            ptp: &mut crate::ptp::Ptp,
        ) -> crate::error::CoreResult<()> {
            #( #writes )*
            Ok(())
        }
    }
}

fn generate_parser_impl(struct_ident: &Ident, camera_struct_path: &TokenStream) -> TokenStream {
    quote! {
        impl crate::features::simulation::CameraSimulationParser for #camera_struct_path {
            fn deserialize_simulation(
                &self,
                data: &[u8],
            ) -> crate::error::CoreResult<
                Box<dyn crate::features::simulation::Simulation>,
            > {
                let sim: #struct_ident = ::serde_json::from_slice(data)
                    .map_err(crate::features::simulation::SimulationError::from)?;
                Ok(Box::new(sim))
            }

            fn serialize_simulation(
                &self,
                simulation: &dyn crate::features::simulation::Simulation,
            ) -> crate::error::CoreResult<Vec<u8>> {
                let bytes = ::serde_json::to_vec(simulation)
                    .map_err(crate::features::simulation::SimulationError::from)?;
                Ok(bytes)
            }
        }
    }
}

fn generate_manager_impl(
    struct_ident: &Ident,
    camera_struct_path: &TokenStream,
    options_path: &TokenStream,
) -> TokenStream {
    let struct_name = struct_ident.to_string();
    quote! {
        impl crate::features::simulation::CameraSimulationManager for #camera_struct_path {
            fn custom_settings_slots(&self) -> Vec<#options_path::CustomSetting> {
                <#options_path::CustomSetting as ::strum::IntoEnumIterator>::iter()
                    .take(#struct_ident::SLOTS as usize)
                    .collect()
            }

            fn get_simulation(
                &self,
                ptp: &mut crate::ptp::Ptp,
                slot: #options_path::CustomSetting,
            ) -> crate::error::CoreResult<
                Box<dyn crate::features::simulation::Simulation>,
            > {
                crate::ptp::option::SimulationSetting::try_push(&slot, ptp)?;
                Ok(Box::new(
                    <#struct_ident as crate::features::simulation::Simulation>::try_pull(ptp)?,
                ))
            }

            fn update_simulation(
                &self,
                ptp: &mut crate::ptp::Ptp,
                slot: #options_path::CustomSetting,
                partial: crate::generated::simulations::SimulationBase,
            ) -> crate::error::CoreResult<()> {
                crate::ptp::option::SimulationSetting::try_push(&slot, ptp)?;
                let mut current =
                    <#struct_ident as crate::features::simulation::Simulation>::try_pull(ptp)?;
                current.try_update_from(&partial)?;
                <#struct_ident as crate::features::simulation::Simulation>::try_push(&current, ptp)
            }

            fn set_simulation(
                &self,
                ptp: &mut crate::ptp::Ptp,
                slot: #options_path::CustomSetting,
                simulation: &dyn crate::features::simulation::Simulation,
            ) -> crate::error::CoreResult<()> {
                let sim = simulation
                    .as_any()
                    .downcast_ref::<#struct_ident>()
                    .ok_or(crate::features::simulation::SimulationError::TypeMismatch {
                        expected: #struct_name,
                    })?;
                crate::ptp::option::SimulationSetting::try_push(&slot, ptp)?;
                <#struct_ident as crate::features::simulation::Simulation>::try_push(sim, ptp)
            }
        }
    }
}
