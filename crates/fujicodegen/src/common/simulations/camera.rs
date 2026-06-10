use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::{
    ast::{Camera, Dnf, FujiOption, Setting},
    common::{cameras, options},
    schema::{
        alias::{NormalizedRule, NormalizedTransformation},
        grammar::{
            Scopes, SettingInfo, build_settings, generate_apply_transformations, generate_dnf,
            generate_emit_warnings_and_infos,
        },
        presence::PresenceDag,
        repair::generate_solve,
    },
    snake_case_ident, upper_camel_case_ident, uppercase_ident,
    util::dag::Dag,
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

    let complete_ident = upper_camel_case_ident!("{}_simulation", camera.id);
    let draft_ident = upper_camel_case_ident!("{}_simulation_draft", camera.id);
    let mod_ident = snake_case_ident!("{}", camera.id);
    let camera_struct_ident = upper_camel_case_ident!("{}", camera.id);

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

    let optional_field_ids: BTreeSet<String> = presence_info.conditions.keys().cloned().collect();
    let flaky_field_ids: BTreeSet<String> = write_order
        .iter()
        .filter(|id| options.get(id.as_str()).is_some_and(|o| o.codegen.flaky))
        .cloned()
        .collect();
    let foreign_field_ids: Vec<&str> = base_union
        .iter()
        .map(String::as_str)
        .filter(|id| !settings.contains_key(*id))
        .collect();

    let draft_struct = generate_draft_struct(&settings, &simulation.settings, &draft_ident);
    let rule_module = generate_rule_module(
        &settings,
        simulation,
        &effective_rules,
        &mod_ident,
        &draft_ident,
        &optional_field_ids,
    )?;
    let complete_struct = generate_complete_struct(
        &settings,
        &simulation.settings,
        &optional_field_ids,
        &complete_ident,
    );
    let inherent_impl = generate_inherent_impl(
        &settings,
        &simulation.settings,
        &optional_field_ids,
        &complete_ident,
        &options_path,
        simulation.slots,
    );
    let from_complete_for_draft = generate_from_complete_for_draft(
        &settings,
        &simulation.settings,
        &optional_field_ids,
        &complete_ident,
        &draft_ident,
    );
    let from_complete_for_base = generate_from_complete_for_base(
        &settings,
        &simulation.settings,
        &optional_field_ids,
        &foreign_field_ids,
        &complete_ident,
    );
    let from_draft_for_base = generate_from_draft_for_base(
        &settings,
        &simulation.settings,
        &foreign_field_ids,
        &draft_ident,
    );
    let try_from_draft_for_complete = generate_try_from_draft_for_complete(
        &settings,
        &simulation.settings,
        &optional_field_ids,
        &complete_ident,
        &draft_ident,
        &mod_ident,
    );
    let try_from_base_for_draft = generate_try_from_base_for_draft(
        &settings,
        &simulation.settings,
        &foreign_field_ids,
        &complete_ident,
        &draft_ident,
    );
    let display_impl = generate_display_impl(
        &settings,
        &simulation.settings,
        &optional_field_ids,
        &complete_ident,
    );
    let deserialize_impl = generate_deserialize_impl(&complete_ident, &draft_ident);
    let simulation_impl = generate_simulation_impl(
        &settings,
        &optional_field_ids,
        &flaky_field_ids,
        &complete_ident,
        &draft_ident,
        &options_path,
        &read_order,
        &write_order,
        &presence_info.conditions,
    )?;
    let parser_impl = generate_parser_impl(&complete_ident, &camera_struct_path);
    let manager_impl = generate_manager_impl(
        &complete_ident,
        &draft_ident,
        &mod_ident,
        &camera_struct_path,
        &options_path,
    );

    Ok(quote! {
        #draft_struct
        #rule_module
        #complete_struct
        #inherent_impl
        #from_complete_for_draft
        #from_complete_for_base
        #from_draft_for_base
        #try_from_draft_for_complete
        #try_from_base_for_draft
        #display_impl
        #deserialize_impl
        #simulation_impl
        #parser_impl
        #manager_impl
    })
}

fn generate_draft_struct(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    draft_ident: &Ident,
) -> TokenStream {
    let field_defs = fields.iter().map(|s| {
        let info = &settings[s.id.as_str()];
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
        pub struct #draft_ident {
            #( #field_defs )*
        }
    }
}

fn generate_rule_module(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    simulation: &crate::ast::Simulation,
    effective_rules: &[NormalizedRule],
    mod_ident: &Ident,
    draft_ident: &Ident,
    optional: &BTreeSet<String>,
) -> anyhow::Result<TokenStream> {
    let buf_ty = quote! { super::#draft_ident };
    let buf_acc = quote! { buf };
    let scopes = Scopes::new(&buf_acc);
    let apply_transformations =
        generate_apply_transformations(settings, &simulation.transformations, &buf_acc, &buf_ty)?;
    let emit_warnings_and_infos =
        generate_emit_warnings_and_infos(settings, effective_rules, scopes, &buf_ty)?;
    let solve = generate_solve(settings, effective_rules, scopes, &buf_ty, optional)?;
    let try_update_from = generate_try_update_from(&simulation.settings, settings, &buf_ty);

    Ok(quote! {
        pub mod #mod_ident {
            #apply_transformations
            #emit_warnings_and_infos
            #solve
            #try_update_from
        }
    })
}

fn generate_try_update_from(
    fields: &[Setting],
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    buf_ty: &TokenStream,
) -> TokenStream {
    let merge_assigns = fields.iter().map(|s| {
        let info = &settings[s.id.as_str()];
        let ident = info.field_ident();
        let read = if info.is_copy {
            quote! { partial_normalized.#ident }
        } else {
            quote! { partial_normalized.#ident.clone() }
        };
        quote! {
            if let Some(value) = #read {
                candidate.#ident = Some(value);
            }
        }
    });

    quote! {
        pub fn try_update_from(
            buf: &mut #buf_ty,
            partial: &#buf_ty,
        ) -> ::std::result::Result<(), crate::features::simulation::SimulationError> {
            let mut partial_normalized = partial.clone();
            apply_transformations(&mut partial_normalized);

            let mut candidate = buf.clone();
            #( #merge_assigns )*
            apply_transformations(&mut candidate);

            solve(&mut candidate, &partial_normalized)?;
            emit_warnings_and_infos(&candidate)?;

            *buf = candidate;
            Ok(())
        }
    }
}

fn generate_complete_struct(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
) -> TokenStream {
    let field_defs = fields.iter().map(|s| {
        let info = &settings[s.id.as_str()];
        let ident = info.field_ident();
        let type_path = info.type_path();
        if optional.contains(s.id.as_str()) {
            quote! {
                #[serde(skip_serializing_if = "Option::is_none")]
                pub #ident: Option<#type_path>,
            }
        } else {
            quote! {
                pub #ident: #type_path,
            }
        }
    });

    quote! {
        #[derive(::std::fmt::Debug, ::std::clone::Clone, ::serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct #complete_ident {
            #( #field_defs )*
        }
    }
}

fn generate_inherent_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
    options_path: &TokenStream,
    slots: u32,
) -> TokenStream {
    let slots_lit = proc_macro2::Literal::u32_suffixed(slots);

    let name_body = if settings.contains_key("custom_setting_name") {
        if optional.contains("custom_setting_name") {
            quote! { self.custom_setting_name.clone() }
        } else {
            quote! { Some(self.custom_setting_name.clone()) }
        }
    } else {
        quote! { None }
    };

    let field_refs = fields.iter().map(|s| {
        let const_ident = uppercase_ident!("SIMULATION_OPT_{}", s.id);
        quote! { &crate::generated::descriptors::#const_ident }
    });

    quote! {
        impl #complete_ident {
            pub const SLOTS: u32 = #slots_lit;

            pub const FIELDS: &'static [&'static crate::features::descriptor::OptionDescriptor<
                crate::generated::simulations::SimulationBase,
            >] = &[ #( #field_refs ),* ];

            #[must_use]
            pub fn name(&self) -> Option<#options_path::CustomSettingName> {
                #name_body
            }
        }
    }
}

fn lift_field(info: &SettingInfo<'_>, source: &TokenStream) -> TokenStream {
    let ident = info.field_ident();
    if info.is_copy {
        quote! { Some(#source.#ident) }
    } else {
        quote! { Some(#source.#ident.clone()) }
    }
}

fn copy_field(info: &SettingInfo<'_>, source: &TokenStream) -> TokenStream {
    let ident = info.field_ident();
    if info.is_copy {
        quote! { #source.#ident }
    } else {
        quote! { #source.#ident.clone() }
    }
}

fn generate_from_complete_for_draft(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
    draft_ident: &Ident,
) -> TokenStream {
    let inits = fields.iter().map(|s| {
        let info = &settings[s.id.as_str()];
        let ident = info.field_ident();
        let value = if optional.contains(s.id.as_str()) {
            copy_field(info, &quote! { simulation })
        } else {
            lift_field(info, &quote! { simulation })
        };
        quote! { #ident: #value, }
    });

    quote! {
        impl ::std::convert::From<&#complete_ident> for #draft_ident {
            fn from(simulation: &#complete_ident) -> Self {
                Self {
                    #( #inits )*
                }
            }
        }
    }
}

fn generate_from_draft_for_base(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    foreign_field_ids: &[&str],
    draft_ident: &Ident,
) -> TokenStream {
    let inits = fields.iter().map(|s| {
        let info = &settings[s.id.as_str()];
        let ident = info.field_ident();
        let value = copy_field(info, &quote! { draft });
        quote! { #ident: #value, }
    });
    let tail = if foreign_field_ids.is_empty() {
        quote! {}
    } else {
        quote! { ..::std::default::Default::default() }
    };

    quote! {
        impl ::std::convert::From<&#draft_ident>
            for crate::generated::simulations::SimulationBase
        {
            fn from(draft: &#draft_ident) -> Self {
                Self {
                    #( #inits )*
                    #tail
                }
            }
        }
    }
}

fn generate_from_complete_for_base(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    optional: &BTreeSet<String>,
    foreign_field_ids: &[&str],
    complete_ident: &Ident,
) -> TokenStream {
    let inits = fields.iter().map(|s| {
        let info = &settings[s.id.as_str()];
        let ident = info.field_ident();
        let value = if optional.contains(s.id.as_str()) {
            copy_field(info, &quote! { simulation })
        } else {
            lift_field(info, &quote! { simulation })
        };
        quote! { #ident: #value, }
    });
    let tail = if foreign_field_ids.is_empty() {
        quote! {}
    } else {
        quote! { ..::std::default::Default::default() }
    };

    quote! {
        impl ::std::convert::From<&#complete_ident>
            for crate::generated::simulations::SimulationBase
        {
            fn from(simulation: &#complete_ident) -> Self {
                Self {
                    #( #inits )*
                    #tail
                }
            }
        }
    }
}

fn generate_try_from_draft_for_complete(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
    draft_ident: &Ident,
    mod_ident: &Ident,
) -> TokenStream {
    let complete_name = complete_ident.to_string();
    let inits = fields.iter().map(|s| {
        let id = s.id.as_str();
        let info = &settings[id];
        let ident = info.field_ident();
        if optional.contains(id) {
            quote! { #ident: candidate.#ident, }
        } else {
            let id_str = id.to_string();
            quote! {
                #ident: candidate.#ident.ok_or(
                    crate::features::simulation::SimulationError::MissingField {
                        simulation: #complete_name,
                        field: #id_str,
                    },
                )?,
            }
        }
    });

    quote! {
        impl ::std::convert::TryFrom<#draft_ident> for #complete_ident {
            type Error = crate::features::simulation::SimulationError;
            fn try_from(
                draft: #draft_ident,
            ) -> ::std::result::Result<Self, crate::features::simulation::SimulationError> {
                let mut candidate = draft;
                #mod_ident::try_update_from(&mut candidate, &#draft_ident::default())?;
                Ok(Self {
                    #( #inits )*
                })
            }
        }
    }
}

fn generate_try_from_base_for_draft(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    foreign_field_ids: &[&str],
    complete_ident: &Ident,
    draft_ident: &Ident,
) -> TokenStream {
    let complete_name = complete_ident.to_string();
    let foreign_checks = foreign_field_ids.iter().map(|id| {
        let ident = snake_case_ident!("{}", id);
        let id_str = (*id).to_string();
        quote! {
            if base.#ident.is_some() {
                return Err(crate::features::simulation::SimulationError::ForeignField {
                    simulation: #complete_name,
                    field: #id_str,
                });
            }
        }
    });
    let inits = fields.iter().map(|s| {
        let info = &settings[s.id.as_str()];
        let ident = info.field_ident();
        let value = copy_field(info, &quote! { base });
        quote! { #ident: #value, }
    });

    quote! {
        impl ::std::convert::TryFrom<crate::generated::simulations::SimulationBase>
            for #draft_ident
        {
            type Error = crate::features::simulation::SimulationError;
            fn try_from(
                base: crate::generated::simulations::SimulationBase,
            ) -> ::std::result::Result<Self, crate::features::simulation::SimulationError> {
                #( #foreign_checks )*
                Ok(Self {
                    #( #inits )*
                })
            }
        }
    }
}

fn generate_display_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Setting],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
) -> TokenStream {
    let lines = fields.iter().map(|s| {
        let id = s.id.as_str();
        let info = &settings[id];
        let ident = info.field_ident();
        let label = info
            .option
            .map_or_else(|| info.id.to_string(), |o| o.spec.name().to_string());
        let escaped = label.replace('{', "{{").replace('}', "}}");
        let fmt = format!("{escaped}: {{value}}");
        if optional.contains(id) {
            quote! {
                if let Some(value) = self.#ident.as_ref() {
                    writeln!(f, #fmt)?;
                }
            }
        } else {
            quote! {
                {
                    let value = &self.#ident;
                    writeln!(f, #fmt)?;
                }
            }
        }
    });
    quote! {
        impl ::std::fmt::Display for #complete_ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                #( #lines )*
                Ok(())
            }
        }
    }
}

fn generate_deserialize_impl(complete_ident: &Ident, draft_ident: &Ident) -> TokenStream {
    quote! {
        impl<'de> ::serde::Deserialize<'de> for #complete_ident {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::std::result::Result<Self, D::Error> {
                let draft = <#draft_ident as ::serde::Deserialize<'de>>::deserialize(deserializer)?;
                <Self as ::std::convert::TryFrom<#draft_ident>>::try_from(draft)
                    .map_err(<D::Error as ::serde::de::Error>::custom)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_simulation_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    optional: &BTreeSet<String>,
    flaky: &BTreeSet<String>,
    complete_ident: &Ident,
    draft_ident: &Ident,
    options_path: &TokenStream,
    read_order: &[String],
    write_order: &[String],
    presence_conditions: &BTreeMap<String, Dnf>,
) -> anyhow::Result<TokenStream> {
    let try_pull = generate_try_pull(settings, draft_ident, read_order, presence_conditions)?;
    let try_push = generate_try_push(settings, optional, flaky, write_order);

    Ok(quote! {
        impl crate::features::simulation::Simulation for #complete_ident {
            fn as_any(&self) -> &dyn ::std::any::Any { self }

            fn name(&self) -> Option<#options_path::CustomSettingName> {
                <Self>::name(self)
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
    draft_ident: &Ident,
    read_order: &[String],
    presence_conditions: &BTreeMap<String, Dnf>,
) -> anyhow::Result<TokenStream> {
    let reads = read_order
        .iter()
        .map(|id| {
            let info = &settings[id.as_str()];
            let ident = info.field_ident();
            let type_path = info.type_path();

            let read_call = quote! {
                Some(<#type_path as crate::ptp::option::SimulationSetting>::try_pull(ptp)?)
            };

            let body = if let Some(dnf) = presence_conditions.get(id) {
                let staged_accessor = quote! { staged };
                let cond = generate_dnf(settings, dnf, Scopes::new(&staged_accessor))?;
                quote! {
                    {
                        let present = #cond;
                        if present { #read_call } else { None }
                    }
                }
            } else {
                read_call
            };

            Ok(quote! { staged.#ident = #body; })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        fn try_pull(
            ptp: &mut crate::ptp::Ptp,
        ) -> crate::error::CoreResult<Self> {
            let mut staged = #draft_ident::default();
            #( #reads )*
            Ok(<Self as ::std::convert::TryFrom<#draft_ident>>::try_from(staged)?)
        }
    })
}

fn generate_try_push(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    optional: &BTreeSet<String>,
    flaky: &BTreeSet<String>,
    write_order: &[String],
) -> TokenStream {
    let writes = write_order.iter().map(|id| {
        let info = &settings[id.as_str()];
        let ident = info.field_ident();
        let value = if optional.contains(id.as_str()) {
            quote! { value }
        } else {
            quote! { &self.#ident }
        };

        let push = if flaky.contains(id.as_str()) {
            quote! {
                if let Err(error) = crate::ptp::option::SimulationSetting::try_push(#value, ptp) {
                    if error.is_disconnect() {
                        return Err(error);
                    }
                    log::warn!("flaky field `{}` rejected by camera ({error}); skipping", #id);
                }
            }
        } else {
            quote! {
                crate::ptp::option::SimulationSetting::try_push(#value, ptp)?;
            }
        };

        if optional.contains(id.as_str()) {
            quote! {
                if let Some(value) = self.#ident.as_ref() {
                    #push
                }
            }
        } else {
            push
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

fn generate_parser_impl(complete_ident: &Ident, camera_struct_path: &TokenStream) -> TokenStream {
    quote! {
        impl crate::features::simulation::CameraSimulationParser for #camera_struct_path {
            fn deserialize_simulation(
                &self,
                data: &[u8],
            ) -> crate::error::CoreResult<
                Box<dyn crate::features::simulation::Simulation>,
            > {
                let sim: #complete_ident = ::serde_json::from_slice(data)
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
    complete_ident: &Ident,
    draft_ident: &Ident,
    mod_ident: &Ident,
    camera_struct_path: &TokenStream,
    options_path: &TokenStream,
) -> TokenStream {
    let complete_name = complete_ident.to_string();
    quote! {
        impl crate::features::simulation::CameraSimulationManager for #camera_struct_path {
            fn custom_settings_slots(&self) -> Vec<#options_path::CustomSetting> {
                <#options_path::CustomSetting as ::strum::IntoEnumIterator>::iter()
                    .take(#complete_ident::SLOTS as usize)
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
                    <#complete_ident as crate::features::simulation::Simulation>::try_pull(ptp)?,
                ))
            }

            fn update_simulation(
                &self,
                ptp: &mut crate::ptp::Ptp,
                slot: #options_path::CustomSetting,
                partial: crate::generated::simulations::SimulationBase,
            ) -> crate::error::CoreResult<()> {
                let partial_draft = <#draft_ident as ::std::convert::TryFrom<
                    crate::generated::simulations::SimulationBase,
                >>::try_from(partial)?;
                crate::ptp::option::SimulationSetting::try_push(&slot, ptp)?;
                let current =
                    <#complete_ident as crate::features::simulation::Simulation>::try_pull(ptp)?;
                let mut draft = #draft_ident::from(&current);
                #mod_ident::try_update_from(&mut draft, &partial_draft)?;
                let next =
                    <#complete_ident as ::std::convert::TryFrom<#draft_ident>>::try_from(draft)?;
                <#complete_ident as crate::features::simulation::Simulation>::try_push(&next, ptp)
            }

            fn set_simulation(
                &self,
                ptp: &mut crate::ptp::Ptp,
                slot: #options_path::CustomSetting,
                simulation: &dyn crate::features::simulation::Simulation,
            ) -> crate::error::CoreResult<()> {
                let sim = simulation
                    .as_any()
                    .downcast_ref::<#complete_ident>()
                    .ok_or(crate::features::simulation::SimulationError::TypeMismatch {
                        expected: #complete_name,
                    })?;
                crate::ptp::option::SimulationSetting::try_push(&slot, ptp)?;
                <#complete_ident as crate::features::simulation::Simulation>::try_push(sim, ptp)
            }
        }
    }
}
