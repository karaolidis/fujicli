use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::{
    ast::{Camera, Dnf, Field, FujiOption, Profile, Render},
    common::{cameras, renders},
    schema::{
        grammar::{
            Scopes, SettingInfo, build_settings, generate_dnf, generate_emit_warnings_and_infos,
            generate_report,
        },
        presence::PresenceDag,
        repair::generate_solve,
        request::{check_requests, generate_requests},
        rule::NormalizedRule,
        transformation::{
            Encoding, analyse, generate_encode, generate_settled_check, generate_undo, raw_ident,
            settled_by,
        },
    },
    snake_case_ident, upper_camel_case_ident,
    util::dag::Dag,
};

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    profile: &Profile,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
    let base_union = collect_base_union(profile, cameras);
    let mut blocks = Vec::with_capacity(cameras.len());
    for camera in cameras.values() {
        let block = generate_one(options, profile, camera, &base_union)
            .with_context(|| format!("generating render profile for camera `{}`", camera.id))?;
        blocks.push(block);
    }
    Ok(quote! { #( #blocks )* })
}

fn collect_base_union(profile: &Profile, cameras: &BTreeMap<String, Camera>) -> BTreeSet<String> {
    cameras
        .values()
        .filter_map(|c| c.spec.features.as_ref()?.render.as_ref())
        .flat_map(|r| r.fields.iter().cloned())
        .filter(|id| profile.get(id).is_some_and(Field::is_setting))
        .collect()
}

fn generate_one(
    options: &BTreeMap<String, FujiOption>,
    profile: &Profile,
    camera: &Camera,
    base_union: &BTreeSet<String>,
) -> anyhow::Result<TokenStream> {
    let Some(render) = camera
        .spec
        .features
        .as_ref()
        .and_then(|f| f.render.as_ref())
    else {
        return Ok(quote! {});
    };

    let listed = profile.select(&render.fields)?;
    let fields: Vec<&Field> = listed.iter().copied().filter(|f| f.is_setting()).collect();
    let inline: Vec<&str> = listed
        .iter()
        .filter(|f| !f.is_setting())
        .map(|f| f.id())
        .collect();
    let settings = build_settings(options, &fields);
    let positions: BTreeMap<&str, usize> = profile
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| render.fields.iter().any(|id| id == f.id()))
        .map(|(position, f)| (f.id(), position))
        .collect();

    let effective_rules: Vec<NormalizedRule> =
        render.rules.iter().map(NormalizedRule::from).collect();
    check_requests(&render.requests, &effective_rules)?;
    let encodings = analyse(&settings, &inline, &render.transformations)?;

    let complete_ident = upper_camel_case_ident!("{}_render_profile", camera.id);
    let draft_ident = upper_camel_case_ident!("{}_render_profile_draft", camera.id);
    let mod_ident = snake_case_ident!("{}", camera.id);
    let camera_struct_ident = upper_camel_case_ident!("{}", camera.id);
    let cameras_path = cameras::path();
    let camera_struct_path = quote! { #cameras_path::#camera_struct_ident };
    let renders_path = renders::path();

    let presence_info = PresenceDag::from_rules(&effective_rules);

    let nodes: Vec<&str> = fields.iter().map(|f| f.id()).collect();
    let edges: Vec<(&str, &str)> = presence_info
        .edges
        .iter()
        .map(|(from, to)| (from.as_str(), to.as_str()))
        .collect();

    let convert_order: Vec<String> = Dag::new(nodes, edges)
        .topological_order()?
        .into_iter()
        .map(str::to_owned)
        .collect();

    let optional_field_ids: BTreeSet<String> = presence_info.conditions.keys().cloned().collect();
    let foreign_field_ids: Vec<&str> = base_union
        .iter()
        .map(String::as_str)
        .filter(|id| !settings.contains_key(*id))
        .collect();

    let draft_struct = generate_draft_struct(&settings, &fields, &draft_ident);
    let rule_module = generate_rule_module(
        &settings,
        &fields,
        render,
        &effective_rules,
        &mod_ident,
        &draft_ident,
        &optional_field_ids,
    )?;
    let complete_struct =
        generate_complete_struct(&settings, &fields, &optional_field_ids, &complete_ident);
    let from_complete_for_draft = generate_from_complete_for_draft(
        &settings,
        &fields,
        &optional_field_ids,
        &complete_ident,
        &draft_ident,
    );
    let from_complete_for_base = generate_from_complete_for_base(
        &settings,
        &fields,
        &optional_field_ids,
        &foreign_field_ids,
        &complete_ident,
        &renders_path,
    );
    let try_from_draft_for_complete = generate_try_from_draft_for_complete(
        &settings,
        &fields,
        &optional_field_ids,
        &complete_ident,
        &draft_ident,
    );
    let try_from_base_for_draft = generate_try_from_base_for_draft(
        &settings,
        &fields,
        &foreign_field_ids,
        &complete_ident,
        &draft_ident,
        &renders_path,
    );
    let write_fields = generate_write_fields(
        &settings,
        profile,
        &optional_field_ids,
        &inline,
        &positions,
        &encodings,
        &draft_ident,
    )?;
    let read_fields = generate_read_fields(
        &settings,
        &fields,
        &complete_ident,
        &draft_ident,
        &presence_info.conditions,
        &positions,
        &encodings,
        &convert_order,
    )?;
    let profile_fields_impl = generate_profile_fields_impl(
        &complete_ident,
        profile.fields.len(),
        &write_fields,
        &read_fields,
    );
    let manager_impl = generate_camera_render_manager_impl(
        &complete_ident,
        &draft_ident,
        &mod_ident,
        &camera_struct_path,
        &renders_path,
    );

    Ok(quote! {
        #draft_struct
        #rule_module
        #complete_struct
        #from_complete_for_draft
        #from_complete_for_base
        #try_from_draft_for_complete
        #try_from_base_for_draft
        #profile_fields_impl
        #manager_impl
    })
}

fn generate_draft_struct(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[&Field],
    draft_ident: &Ident,
) -> TokenStream {
    let field_defs = fields.iter().map(|f| {
        let info = &settings[f.id()];
        let ident = info.field_ident();
        let type_path = info.type_path();
        quote! { pub #ident: Option<#type_path>, }
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
    fields: &[&Field],
    render: &Render,
    effective_rules: &[NormalizedRule],
    mod_ident: &Ident,
    draft_ident: &Ident,
    optional: &BTreeSet<String>,
) -> anyhow::Result<TokenStream> {
    let buf_ty = quote! { super::#draft_ident };
    let buf_acc = quote! { buf };
    let original_acc = quote! { original };
    let scopes = Scopes::with_original(&buf_acc, &original_acc);
    let requests = generate_requests(settings, &render.requests, &buf_ty)?;
    let emit_warnings_and_infos =
        generate_emit_warnings_and_infos(settings, effective_rules, scopes, &buf_ty)?;
    let solve = generate_solve(settings, effective_rules, scopes, &buf_ty, optional)?;
    let report = generate_report(settings, fields.iter().map(|f| f.id()), &buf_ty);
    let try_update_from = generate_try_update_from(fields, settings, &buf_ty);

    Ok(quote! {
        pub mod #mod_ident {
            #requests
            #emit_warnings_and_infos
            #solve
            #report
            #try_update_from
        }
    })
}

fn generate_try_update_from(
    fields: &[&Field],
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    buf_ty: &TokenStream,
) -> TokenStream {
    let merge_assigns = fields.iter().map(|f| {
        let info = &settings[f.id()];
        let ident = info.field_ident();
        let read = if info.is_copy {
            quote! { pins.#ident }
        } else {
            quote! { pins.#ident.clone() }
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
            original: &#buf_ty,
        ) -> ::std::result::Result<(), crate::features::simulation::SimulationError> {
            let mut pins = partial.clone();
            drop_keeps(&mut pins);

            let mut candidate = buf.clone();
            #( #merge_assigns )*
            apply_becomes(&mut candidate);

            let before = candidate.clone();
            solve(&mut candidate, &pins, original)?;
            report(&before, &candidate);
            emit_warnings_and_infos(&candidate, original)?;

            *buf = candidate;
            Ok(())
        }
    }
}

fn generate_complete_struct(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[&Field],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
) -> TokenStream {
    let field_defs = fields.iter().map(|f| {
        let info = &settings[f.id()];
        let ident = info.field_ident();
        let type_path = info.type_path();
        if optional.contains(f.id()) {
            quote! { pub #ident: Option<#type_path>, }
        } else {
            quote! { pub #ident: #type_path, }
        }
    });

    quote! {
        #[derive(::std::fmt::Debug, ::std::clone::Clone)]
        pub struct #complete_ident {
            #( #field_defs )*
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
    fields: &[&Field],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
    draft_ident: &Ident,
) -> TokenStream {
    let inits = fields.iter().map(|f| {
        let info = &settings[f.id()];
        let ident = info.field_ident();
        let value = if optional.contains(f.id()) {
            copy_field(info, &quote! { profile })
        } else {
            lift_field(info, &quote! { profile })
        };
        quote! { #ident: #value, }
    });

    quote! {
        impl ::std::convert::From<&#complete_ident> for #draft_ident {
            fn from(profile: &#complete_ident) -> Self {
                Self {
                    #( #inits )*
                }
            }
        }
    }
}

fn generate_from_complete_for_base(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[&Field],
    optional: &BTreeSet<String>,
    foreign_field_ids: &[&str],
    complete_ident: &Ident,
    renders_path: &TokenStream,
) -> TokenStream {
    let inits = fields.iter().map(|f| {
        let info = &settings[f.id()];
        let ident = info.field_ident();
        let value = if optional.contains(f.id()) {
            copy_field(info, &quote! { profile })
        } else {
            lift_field(info, &quote! { profile })
        };
        quote! { #ident: #value, }
    });
    let tail = if foreign_field_ids.is_empty() {
        quote! {}
    } else {
        quote! { ..::std::default::Default::default() }
    };

    quote! {
        impl ::std::convert::From<&#complete_ident> for #renders_path::RenderBase {
            fn from(profile: &#complete_ident) -> Self {
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
    fields: &[&Field],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
    draft_ident: &Ident,
) -> TokenStream {
    let complete_name = complete_ident.to_string();
    let inits = fields.iter().map(|f| {
        let id = f.id();
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
                candidate: #draft_ident,
            ) -> ::std::result::Result<Self, crate::features::simulation::SimulationError> {
                Ok(Self {
                    #( #inits )*
                })
            }
        }
    }
}

fn generate_try_from_base_for_draft(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[&Field],
    foreign_field_ids: &[&str],
    complete_ident: &Ident,
    draft_ident: &Ident,
    renders_path: &TokenStream,
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
    let inits = fields.iter().map(|f| {
        let info = &settings[f.id()];
        let ident = info.field_ident();
        let value = copy_field(info, &quote! { base });
        quote! { #ident: #value, }
    });

    quote! {
        impl ::std::convert::TryFrom<#renders_path::RenderBase> for #draft_ident {
            type Error = crate::features::simulation::SimulationError;
            fn try_from(
                base: #renders_path::RenderBase,
            ) -> ::std::result::Result<Self, crate::features::simulation::SimulationError> {
                #( #foreign_checks )*
                Ok(Self {
                    #( #inits )*
                })
            }
        }
    }
}

fn generate_write_fields(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    profile: &Profile,
    optional: &BTreeSet<String>,
    inline: &[&str],
    positions: &BTreeMap<&str, usize>,
    encodings: &[Encoding],
    draft_ident: &Ident,
) -> anyhow::Result<TokenStream> {
    let writes = profile
        .fields
        .iter()
        .enumerate()
        .filter(|(_, field)| settings.contains_key(field.id()))
        .map(|(position, field)| generate_write_one(settings, optional, field, position));
    let zeros = inline.iter().map(|id| {
        let position = positions[id];
        quote! { values[#position] = 0; }
    });
    let encode = if encodings.is_empty() {
        quote! {}
    } else {
        let draft = quote! { draft };
        let blocks = generate_encode(settings, encodings, positions, &draft)?;
        quote! {
            let draft = #draft_ident::from(self);
            #blocks
        }
    };

    Ok(quote! {
        #[allow(clippy::nonminimal_bool)]
        fn write_fields(&self, values: &mut [i32]) -> ::std::io::Result<()> {
            #( #writes )*
            #( #zeros )*
            #encode

            Ok(())
        }
    })
}

fn generate_write_one(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    optional: &BTreeSet<String>,
    field: &Field,
    position: usize,
) -> TokenStream {
    let id = field.id();
    let info = &settings[id];
    let ident = info.field_ident();
    let type_path = info.type_path();

    let encode = |reference: TokenStream| {
        quote! {
            <#type_path as crate::ptp::option::ConversionProfileField>
                ::try_into_conversion_profile_value(#reference)?
        }
    };

    if optional.contains(id) {
        let encoded = encode(quote! { value });
        quote! {
            values[#position] = match self.#ident.as_ref() {
                Some(value) => #encoded,
                None => 0,
            };
        }
    } else {
        let encoded = encode(quote! { &self.#ident });
        quote! { values[#position] = #encoded; }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_read_fields(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[&Field],
    complete_ident: &Ident,
    draft_ident: &Ident,
    presence_conditions: &BTreeMap<String, Dnf>,
    positions: &BTreeMap<&str, usize>,
    encodings: &[Encoding],
    convert_order: &[String],
) -> anyhow::Result<TokenStream> {
    let pattern_fields: BTreeSet<&str> = encodings
        .iter()
        .filter(|e| e.witness.is_some())
        .flat_map(|e| e.writes.iter().map(|(f, _)| f.as_str()))
        .collect();
    let mut read: Vec<(&str, usize)> = positions
        .iter()
        .filter(|(id, _)| settings.contains_key(*id) || pattern_fields.contains(*id))
        .map(|(id, position)| (*id, *position))
        .collect();
    read.sort_by_key(|(_, position)| *position);
    let raw_reads = read.into_iter().map(|(id, position)| {
        let raw = raw_ident(id);
        quote! {
            let #raw = crate::features::render::profile_value(values, #position, #id)?;
        }
    });

    let staged = quote! { staged };
    let undo = generate_undo(settings, encodings, positions, &staged)?;
    let conversions = convert_order
        .iter()
        .map(|id| {
            let field = fields
                .iter()
                .find(|f| f.id() == id.as_str())
                .expect("convert order references known field");
            let convert = generate_convert_one(settings, field, presence_conditions)?;
            let settled = settled_by(encodings, id);
            Ok(if settled.is_empty() {
                convert
            } else {
                quote! {
                    if !( #( #settled )||* ) {
                        #convert
                    }
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let settled_check = generate_settled_check(settings, encodings, presence_conditions, &staged)?;

    Ok(quote! {
        #[allow(clippy::nonminimal_bool)]
        fn read_fields(values: &[i32]) -> ::std::io::Result<Self> {
            #( #raw_reads )*

            let mut staged = #draft_ident::default();
            #undo
            #( #conversions )*
            #settled_check

            <Self as ::std::convert::TryFrom<#draft_ident>>::try_from(staged)
                .map_err(|err| ::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidData,
                    format!("{}: {err}", stringify!(#complete_ident)),
                ))
        }
    })
}

fn generate_convert_one(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    field: &Field,
    presence_conditions: &BTreeMap<String, Dnf>,
) -> anyhow::Result<TokenStream> {
    let info = &settings[field.id()];
    let ident = info.field_ident();
    let type_path = info.type_path();
    let raw = raw_ident(field.id());

    let value_expr = quote! {
        <#type_path as crate::ptp::option::ConversionProfileField>
            ::try_from_conversion_profile_value(#raw)?
    };

    if let Some(condition) = presence_conditions.get(field.id()) {
        let staged_accessor = quote! { staged };
        let cond = generate_dnf(settings, condition, Scopes::new(&staged_accessor))?;
        Ok(quote! {
            staged.#ident = {
                #[allow(clippy::manual_is_variant_and)]
                let present = #cond;
                if present { Some(#value_expr) } else { None }
            };
        })
    } else {
        Ok(quote! { staged.#ident = Some(#value_expr); })
    }
}

fn generate_profile_fields_impl(
    complete_ident: &Ident,
    len: usize,
    write_fields: &TokenStream,
    read_fields: &TokenStream,
) -> TokenStream {
    quote! {
        impl crate::features::render::ConversionProfileFields for #complete_ident {
            const LEN: usize = #len;

            #write_fields

            #read_fields
        }
    }
}

fn generate_camera_render_manager_impl(
    complete_ident: &Ident,
    draft_ident: &Ident,
    mod_ident: &Ident,
    camera_struct_path: &TokenStream,
    renders_path: &TokenStream,
) -> TokenStream {
    quote! {
        impl crate::features::render::CameraRenderManager for #camera_struct_path {
            fn read_profile(
                &self,
                ptp: &mut crate::ptp::Ptp,
            ) -> crate::error::CoreResult<#renders_path::RenderBase> {
                let current: crate::features::render::ConversionProfile<#complete_ident> =
                    ptp.get_prop(crate::ptp::DevicePropCode::FujiRawConversionProfile)?;
                Ok(<#renders_path::RenderBase as ::std::convert::From<&#complete_ident>>::from(
                    &current.fields,
                ))
            }

            fn apply_profile(
                &self,
                ptp: &mut crate::ptp::Ptp,
                partial: &#renders_path::RenderBase,
            ) -> crate::error::CoreResult<()> {
                let current: crate::features::render::ConversionProfile<#complete_ident> =
                    ptp.get_prop(crate::ptp::DevicePropCode::FujiRawConversionProfile)?;
                ::log::debug!("Profile as shot: {current:?}");
                let partial_draft = <#draft_ident as ::std::convert::TryFrom<
                    #renders_path::RenderBase,
                >>::try_from(partial.clone())?;
                let original = #draft_ident::from(&current.fields);
                let mut buf = original.clone();
                #mod_ident::try_update_from(&mut buf, &partial_draft, &original)?;
                let next = crate::features::render::ConversionProfile {
                    code: current.code,
                    fields: <#complete_ident as ::std::convert::TryFrom<#draft_ident>>::try_from(buf)?,
                    reported: current.reported,
                };
                ::log::debug!("Profile to apply: {next:?}");
                ptp.set_prop(crate::ptp::DevicePropCode::FujiRawConversionProfile, &next)?;
                Ok(())
            }
        }
    }
}
