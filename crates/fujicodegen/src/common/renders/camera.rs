use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;

use crate::{
    ast::{Camera, Dnf, Field, FujiOption, Render, Transformation},
    common::{cameras, renders},
    schema::{
        alias::{NormalizedRule, NormalizedTransformation},
        grammar::{
            Scopes, SettingInfo, build_settings, generate_apply_transformations, generate_dnf,
            generate_emit_warnings_and_infos,
        },
        inverse::generate_inverses,
        presence::PresenceDag,
        repair::generate_solve,
    },
    snake_case_ident, upper_camel_case_ident,
    util::dag::Dag,
};

// NOTE: Naively assume the same padding holds for all Fujifilm cameras
// until we have a second render-capable camera to compare against.
const RENDER_HEADER_PADDING: usize = 0x1EE;

pub fn generate(
    options: &BTreeMap<String, FujiOption>,
    cameras: &BTreeMap<String, Camera>,
) -> anyhow::Result<TokenStream> {
    let base_union = collect_base_union(cameras);
    let mut blocks = Vec::with_capacity(cameras.len());
    for camera in cameras.values() {
        let block = generate_one(options, camera, &base_union)
            .with_context(|| format!("generating render profile for camera `{}`", camera.id))?;
        blocks.push(block);
    }
    Ok(quote! { #( #blocks )* })
}

fn collect_base_union(cameras: &BTreeMap<String, Camera>) -> BTreeSet<String> {
    cameras
        .values()
        .filter_map(|c| c.spec.features.as_ref()?.render.as_ref())
        .flat_map(|r| r.fields.iter().map(|f| f.id().to_string()))
        .collect()
}

fn generate_one(
    options: &BTreeMap<String, FujiOption>,
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

    let settings = build_settings(options, &render.fields);

    let aliases: Vec<NormalizedTransformation> = render
        .transformations
        .iter()
        .cloned()
        .filter_map(Option::from)
        .collect();
    let effective_rules: Vec<NormalizedRule> = render
        .rules
        .iter()
        .map(|r| NormalizedRule::from_rule(r, &aliases))
        .collect();

    let complete_ident = upper_camel_case_ident!("{}_render_profile", camera.id);
    let draft_ident = upper_camel_case_ident!("{}_render_profile_draft", camera.id);
    let mod_ident = snake_case_ident!("{}", camera.id);
    let camera_struct_ident = upper_camel_case_ident!("{}", camera.id);
    let cameras_path = cameras::path();
    let camera_struct_path = quote! { #cameras_path::#camera_struct_ident };
    let renders_path = renders::path();

    let presence_info = PresenceDag::try_from_rules(&effective_rules)
        .with_context(|| format!("extracting read DAG for `{}`", camera.id))?;

    let nodes: Vec<&str> = render.fields.iter().map(Field::id).collect();
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

    let n_props = i16::try_from(render.fields.len())
        .with_context(|| format!("too many render fields on camera `{}`", camera.id))?;
    let profile_code = render.profile_code;

    let optional_field_ids: BTreeSet<String> = presence_info.conditions.keys().cloned().collect();
    let foreign_field_ids: Vec<&str> = base_union
        .iter()
        .map(String::as_str)
        .filter(|id| !settings.contains_key(*id))
        .collect();

    let draft_struct = generate_draft_struct(&settings, &render.fields, &draft_ident);
    let rule_module = generate_rule_module(
        &settings,
        render,
        &effective_rules,
        &mod_ident,
        &draft_ident,
    )?;
    let complete_struct = generate_complete_struct(
        &settings,
        &render.fields,
        &optional_field_ids,
        &complete_ident,
    );
    let inherent_impl = generate_inherent_impl(&complete_ident, profile_code);
    let from_complete_for_draft = generate_from_complete_for_draft(
        &settings,
        &render.fields,
        &optional_field_ids,
        &complete_ident,
        &draft_ident,
    );
    let from_complete_for_base = generate_from_complete_for_base(
        &settings,
        &render.fields,
        &optional_field_ids,
        &foreign_field_ids,
        &complete_ident,
        &renders_path,
    );
    let try_from_draft_for_complete = generate_try_from_draft_for_complete(
        &settings,
        &render.fields,
        &optional_field_ids,
        &complete_ident,
        &draft_ident,
        &mod_ident,
    );
    let try_from_base_for_draft = generate_try_from_base_for_draft(
        &settings,
        &render.fields,
        &foreign_field_ids,
        &complete_ident,
        &draft_ident,
        &renders_path,
    );
    let ptp_serialize = generate_ptp_serialize_impl(
        &settings,
        &render.fields,
        &optional_field_ids,
        &complete_ident,
        n_props,
    );
    let ptp_deserialize = generate_ptp_deserialize_impl(
        &settings,
        &render.fields,
        &complete_ident,
        &draft_ident,
        n_props,
        &presence_info.conditions,
        &render.transformations,
        &convert_order,
    )?;
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
        #inherent_impl
        #from_complete_for_draft
        #from_complete_for_base
        #try_from_draft_for_complete
        #try_from_base_for_draft
        #ptp_serialize
        #ptp_deserialize
        #manager_impl
    })
}

fn generate_draft_struct(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Field],
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
    render: &Render,
    effective_rules: &[NormalizedRule],
    mod_ident: &Ident,
    draft_ident: &Ident,
) -> anyhow::Result<TokenStream> {
    let buf_ty = quote! { super::#draft_ident };
    let buf_acc = quote! { buf };
    let original_acc = quote! { original };
    let scopes = Scopes::with_original(&buf_acc, &original_acc);
    let apply_transformations =
        generate_apply_transformations(settings, &render.transformations, &buf_acc, &buf_ty)?;
    let emit_warnings_and_infos =
        generate_emit_warnings_and_infos(settings, effective_rules, scopes, &buf_ty)?;
    let solve = generate_solve(settings, effective_rules, scopes, &buf_ty)?;
    let try_update_from = generate_try_update_from(&render.fields, settings, &buf_ty);

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
    fields: &[Field],
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    buf_ty: &TokenStream,
) -> TokenStream {
    let merge_assigns = fields.iter().map(|f| {
        let info = &settings[f.id()];
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
            let original = buf.clone();
            let mut partial_normalized = partial.clone();
            apply_transformations(&mut partial_normalized);

            let mut candidate = buf.clone();
            #( #merge_assigns )*
            apply_transformations(&mut candidate);

            solve(&mut candidate, &partial_normalized, &original)?;
            emit_warnings_and_infos(&candidate, &original)?;

            *buf = candidate;
            Ok(())
        }
    }
}

fn generate_complete_struct(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Field],
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

fn generate_inherent_impl(complete_ident: &Ident, profile_code: u32) -> TokenStream {
    let profile_code_lit = Literal::u32_suffixed(profile_code);
    quote! {
        impl #complete_ident {
            pub const PROFILE_CODE: u32 = #profile_code_lit;
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
    fields: &[Field],
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
    fields: &[Field],
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
    fields: &[Field],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
    draft_ident: &Ident,
    mod_ident: &Ident,
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
    fields: &[Field],
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

fn generate_ptp_serialize_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Field],
    optional: &BTreeSet<String>,
    complete_ident: &Ident,
    n_props: i16,
) -> TokenStream {
    let n_props_lit = Literal::i16_suffixed(n_props);
    let padding_lit = Literal::usize_suffixed(RENDER_HEADER_PADDING);

    let writes = fields
        .iter()
        .map(|field| generate_write_one(settings, optional, field));

    quote! {
        impl ::ptp_cursor::PtpSerialize for #complete_ident {
            fn try_into_ptp(&self) -> ::std::io::Result<Vec<u8>> {
                let mut buf = Vec::new();
                <Self as ::ptp_cursor::PtpSerialize>::try_write_ptp(self, &mut buf)?;
                Ok(buf)
            }

            fn try_write_ptp(&self, buf: &mut Vec<u8>) -> ::std::io::Result<()> {
                let n_props: i16 = #n_props_lit;
                ::ptp_cursor::PtpSerialize::try_write_ptp(&n_props, buf)?;
                let profile_code = ::ptp_cursor::ExactString::from(
                    format!("{:x}", Self::PROFILE_CODE),
                );
                ::ptp_cursor::PtpSerialize::try_write_ptp(&profile_code, buf)?;
                let padding = [0u8; #padding_lit];
                ::std::io::Write::write_all(buf, &padding)?;

                #( #writes )*

                Ok(())
            }
        }
    }
}

fn generate_write_one(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    optional: &BTreeSet<String>,
    field: &Field,
) -> TokenStream {
    if field.skip_write() {
        return quote! {};
    }
    let id = field.id();
    let info = &settings[id];
    let ident = info.field_ident();
    let type_path = info.type_path();
    let is_optional = optional.contains(id);

    let write = |value: TokenStream| {
        if info.option.is_some() {
            quote! {
                <#type_path as crate::ptp::option::ConversionProfileField>
                    ::try_write_conversion_profile_field_ptp(#value, buf)?;
            }
        } else {
            quote! {
                ::ptp_cursor::PtpSerialize::try_write_ptp(#value, buf)?;
            }
        }
    };

    if is_optional {
        let write_some = write(quote! { value });
        quote! {
            match self.#ident.as_ref() {
                Some(value) => { #write_some }
                None => {
                    ::ptp_cursor::PtpSerialize::try_write_ptp(&0i32, buf)?;
                }
            }
        }
    } else {
        write(quote! { &self.#ident })
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_ptp_deserialize_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Field],
    complete_ident: &Ident,
    draft_ident: &Ident,
    n_props: i16,
    presence_conditions: &BTreeMap<String, Dnf>,
    transformations: &[Transformation],
    convert_order: &[String],
) -> anyhow::Result<TokenStream> {
    let n_props_lit = Literal::i16_suffixed(n_props);
    let padding_lit = Literal::usize_suffixed(RENDER_HEADER_PADDING);

    let raw_reads: Vec<TokenStream> = fields
        .iter()
        .filter(|f| !f.skip_read())
        .map(|field| {
            let info = &settings[field.id()];
            let raw_ident = raw_local_ident(&info.field_ident());
            quote! {
                let #raw_ident = <i32 as ::ptp_cursor::PtpDeserialize>::try_read_ptp(cur)?;
            }
        })
        .collect();

    let conversions = convert_order
        .iter()
        .map(|id| {
            let field = fields
                .iter()
                .find(|f| f.id() == id.as_str())
                .expect("convert order references known field");
            generate_convert_one(settings, field, presence_conditions)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let inverses = generate_inverses(settings, transformations, &quote! { staged })?;

    Ok(quote! {
        impl ::ptp_cursor::PtpDeserialize for #complete_ident {
            fn try_from_ptp(buf: &[u8]) -> ::std::io::Result<Self> {
                let mut cur = ::std::io::Cursor::new(buf);
                let val = <Self as ::ptp_cursor::PtpDeserialize>::try_read_ptp(&mut cur)?;
                Ok(val)
            }

            #[allow(clippy::nonminimal_bool)]
            fn try_read_ptp<R: ::ptp_cursor::Read>(
                cur: &mut R,
            ) -> ::std::io::Result<Self> {
                let n_props = <i16 as ::ptp_cursor::PtpDeserialize>::try_read_ptp(cur)?;
                if n_props != #n_props_lit {
                    return Err(::std::io::Error::new(
                        ::std::io::ErrorKind::InvalidData,
                        format!(
                            "{}: expected {} props on the wire, got {}",
                            stringify!(#complete_ident),
                            #n_props_lit,
                            n_props,
                        ),
                    ));
                }
                let profile_code_str =
                    <::ptp_cursor::ExactString as ::ptp_cursor::PtpDeserialize>
                        ::try_read_ptp(cur)?;
                let parsed = u32::from_str_radix(profile_code_str.as_str(), 16)
                    .map_err(|err| ::std::io::Error::new(
                        ::std::io::ErrorKind::InvalidData,
                        format!(
                            "{}: invalid profile-code hex `{}`: {}",
                            stringify!(#complete_ident),
                            profile_code_str.as_str(),
                            err,
                        ),
                    ))?;
                if parsed != Self::PROFILE_CODE {
                    return Err(::std::io::Error::new(
                        ::std::io::ErrorKind::InvalidData,
                        format!(
                            "{}: expected profile code {:#x}, got {:#x}",
                            stringify!(#complete_ident),
                            Self::PROFILE_CODE,
                            parsed,
                        ),
                    ));
                }
                let mut padding = [0u8; #padding_lit];
                <R as ::std::io::Read>::read_exact(cur, &mut padding)?;

                #( #raw_reads )*

                let mut staged = #draft_ident::default();
                #( #conversions )*

                #inverses

                <Self as ::std::convert::TryFrom<#draft_ident>>::try_from(staged)
                    .map_err(|err| ::std::io::Error::new(
                        ::std::io::ErrorKind::InvalidData,
                        format!("{}: {err}", stringify!(#complete_ident)),
                    ))
            }
        }
    })
}

fn generate_convert_one(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    field: &Field,
    presence_conditions: &BTreeMap<String, Dnf>,
) -> anyhow::Result<TokenStream> {
    if field.skip_read() {
        return Ok(quote! {});
    }

    let info = &settings[field.id()];
    let ident = info.field_ident();
    let type_path = info.type_path();
    let raw_ident = raw_local_ident(&ident);

    let value_expr = if info.option.is_some() {
        quote! {
            <#type_path as crate::ptp::option::ConversionProfileField>
                ::try_from_conversion_profile_field_ptp(&#raw_ident.to_le_bytes())?
        }
    } else {
        quote! { #raw_ident }
    };

    if let Some(condition) = presence_conditions.get(field.id()) {
        let staged_accessor = quote! { staged };
        let cond = generate_dnf(settings, condition, Scopes::new(&staged_accessor))?;
        Ok(quote! {
            staged.#ident = {
                let present = #cond;
                if present { Some(#value_expr) } else { None }
            };
        })
    } else {
        Ok(quote! { staged.#ident = Some(#value_expr); })
    }
}

fn raw_local_ident(ident: &Ident) -> Ident {
    snake_case_ident!("raw_{}", ident)
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
            fn render(
                &self,
                ptp: &mut crate::ptp::Ptp,
                image: &[u8],
                partial: &#renders_path::RenderBase,
                draft: bool,
            ) -> crate::error::CoreResult<Vec<u8>> {
                <Self as crate::features::render::CameraRenderManager>::send_image(self, ptp, image)?;
                let current: #complete_ident = ptp.get_prop(
                    crate::ptp::DevicePropCode::FujiRawConversionProfile,
                )?;
                let partial_draft = <#draft_ident as ::std::convert::TryFrom<
                    #renders_path::RenderBase,
                >>::try_from(partial.clone())?;
                let mut buf = #draft_ident::from(&current);
                #mod_ident::try_update_from(&mut buf, &partial_draft)?;
                let next = <#complete_ident as ::std::convert::TryFrom<#draft_ident>>::try_from(buf)?;
                ptp.set_prop(
                    crate::ptp::DevicePropCode::FujiRawConversionProfile,
                    &next,
                )?;
                <Self as crate::features::render::CameraRenderManager>::render_image(
                    self, ptp, draft,
                )
            }
        }
    }
}
