use std::collections::BTreeMap;

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
        repair::{generate_pin_set, generate_solve},
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
    let mut blocks = Vec::with_capacity(cameras.len());
    for camera in cameras.values() {
        let block = generate_one(options, camera)
            .with_context(|| format!("generating render profile for camera `{}`", camera.id))?;
        blocks.push(block);
    }
    Ok(quote! { #( #blocks )* })
}

fn generate_one(
    options: &BTreeMap<String, FujiOption>,
    camera: &Camera,
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

    let struct_ident = upper_camel_case_ident!("{}_render_profile", camera.id);
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

    let struct_def = generate_struct_def(&settings, &render.fields, &struct_ident);
    let inherent_impl = generate_inherent_impl(
        &settings,
        render,
        &effective_rules,
        &struct_ident,
        &renders_path,
        profile_code,
    )?;
    let serialize_impl =
        generate_ptp_serialize_impl(&settings, &render.fields, &struct_ident, n_props);
    let deserialize_impl = generate_ptp_deserialize_impl(
        &settings,
        &render.fields,
        &struct_ident,
        n_props,
        &presence_info.conditions,
        &render.transformations,
        &convert_order,
    )?;
    let trait_impl =
        generate_camera_render_manager_impl(&struct_ident, &camera_struct_path, &renders_path);

    Ok(quote! {
        #struct_def
        #inherent_impl
        #serialize_impl
        #deserialize_impl
        #trait_impl
    })
}

fn generate_struct_def(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Field],
    struct_ident: &Ident,
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
        pub struct #struct_ident {
            #( #field_defs )*
        }
    }
}

fn generate_inherent_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    render: &Render,
    effective_rules: &[NormalizedRule],
    struct_ident: &Ident,
    renders_path: &TokenStream,
    profile_code: u32,
) -> anyhow::Result<TokenStream> {
    let profile_code_lit = Literal::u32_suffixed(profile_code);

    let apply_transformations = generate_apply_transformations(settings, &render.transformations)?;
    let self_acc = quote! { self };
    let original_acc = quote! { original };
    let warnings_infos = generate_emit_warnings_and_infos(
        settings,
        effective_rules,
        Scopes::with_original(&self_acc, &original_acc),
    )?;
    let solve = generate_solve(settings, effective_rules, true)?;
    let try_update_from = generate_try_update_from(settings, &render.fields, renders_path);

    Ok(quote! {
        impl #struct_ident {
            pub const PROFILE_CODE: u32 = #profile_code_lit;

            #apply_transformations
            #warnings_infos
            #solve
            #try_update_from
        }
    })
}

fn generate_try_update_from(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Field],
    renders_path: &TokenStream,
) -> TokenStream {
    let init_fields = fields.iter().map(|f| {
        let info = &settings[f.id()];
        let ident = info.field_ident();
        let value = if info.is_copy {
            quote! { partial.#ident }
        } else {
            quote! { partial.#ident.clone() }
        };
        quote! { #ident: #value, }
    });

    let merge_assigns = fields.iter().map(|f| {
        let info = &settings[f.id()];
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
            partial: &#renders_path::RenderBase,
        ) -> ::std::result::Result<(), crate::features::simulation::SimulationError> {
            let original = self.clone();
            let mut partial_profile = Self {
                #( #init_fields )*
            };
            partial_profile.apply_transformations();

            let pin = #pin_set_expr;

            let mut candidate = self.clone();
            #( #merge_assigns )*
            candidate.apply_transformations();

            candidate.solve(&pin, &original)?;
            candidate.emit_warnings_and_infos(&original)?;

            *self = candidate;
            Ok(())
        }
    }
}

fn generate_ptp_serialize_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Field],
    struct_ident: &Ident,
    n_props: i16,
) -> TokenStream {
    let n_props_lit = Literal::i16_suffixed(n_props);
    let padding_lit = Literal::usize_suffixed(RENDER_HEADER_PADDING);

    let writes = fields
        .iter()
        .map(|field| generate_write_one(settings, field));

    quote! {
        impl ::ptp_cursor::PtpSerialize for #struct_ident {
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

fn generate_write_one(settings: &BTreeMap<&str, SettingInfo<'_>>, field: &Field) -> TokenStream {
    if field.skip_write() {
        return quote! {};
    }
    let info = &settings[field.id()];
    let ident = info.field_ident();
    let type_path = info.type_path();
    if info.option.is_some() {
        quote! {
            match self.#ident.as_ref() {
                Some(value) => {
                    <#type_path as crate::ptp::option::ConversionProfileField>
                        ::try_write_conversion_profile_field_ptp(value, buf)?;
                }
                None => {
                    ::ptp_cursor::PtpSerialize::try_write_ptp(&0i32, buf)?;
                }
            }
        }
    } else {
        quote! {
            let value: i32 = self.#ident.unwrap_or(0);
            ::ptp_cursor::PtpSerialize::try_write_ptp(&value, buf)?;
        }
    }
}

fn generate_ptp_deserialize_impl(
    settings: &BTreeMap<&str, SettingInfo<'_>>,
    fields: &[Field],
    struct_ident: &Ident,
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

    let inverses = generate_inverses(settings, transformations, &quote! { profile })?;

    Ok(quote! {
        impl ::ptp_cursor::PtpDeserialize for #struct_ident {
            fn try_from_ptp(buf: &[u8]) -> ::std::io::Result<Self> {
                let mut cur = ::std::io::Cursor::new(buf);
                let val = <Self as ::ptp_cursor::PtpDeserialize>::try_read_ptp(&mut cur)?;
                Ok(val)
            }

            #[allow(clippy::field_reassign_with_default, clippy::nonminimal_bool)]
            fn try_read_ptp<R: ::ptp_cursor::Read>(
                cur: &mut R,
            ) -> ::std::io::Result<Self> {
                let n_props = <i16 as ::ptp_cursor::PtpDeserialize>::try_read_ptp(cur)?;
                if n_props != #n_props_lit {
                    return Err(::std::io::Error::new(
                        ::std::io::ErrorKind::InvalidData,
                        format!(
                            "{}: expected {} props on the wire, got {}",
                            stringify!(#struct_ident),
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
                            stringify!(#struct_ident),
                            profile_code_str.as_str(),
                            err,
                        ),
                    ))?;
                if parsed != Self::PROFILE_CODE {
                    return Err(::std::io::Error::new(
                        ::std::io::ErrorKind::InvalidData,
                        format!(
                            "{}: expected profile code {:#x}, got {:#x}",
                            stringify!(#struct_ident),
                            Self::PROFILE_CODE,
                            parsed,
                        ),
                    ));
                }
                let mut padding = [0u8; #padding_lit];
                <R as ::std::io::Read>::read_exact(cur, &mut padding)?;

                #( #raw_reads )*

                let mut profile = Self::default();
                #( #conversions )*

                #inverses

                Ok(profile)
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

    let convert = if info.option.is_some() {
        quote! {
            profile.#ident = Some(
                <#type_path as crate::ptp::option::ConversionProfileField>
                    ::try_from_conversion_profile_field_ptp(&#raw_ident.to_le_bytes())?,
            );
        }
    } else {
        quote! { profile.#ident = Some(#raw_ident); }
    };

    if let Some(condition) = presence_conditions.get(field.id()) {
        let profile_accessor = quote! { profile };
        let cond = generate_dnf(settings, condition, Scopes::new(&profile_accessor))?;
        Ok(quote! {
            if #cond {
                #convert
            }
        })
    } else {
        Ok(convert)
    }
}

fn raw_local_ident(ident: &Ident) -> Ident {
    snake_case_ident!("raw_{}", ident)
}

fn generate_camera_render_manager_impl(
    struct_ident: &Ident,
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
                let mut profile: #struct_ident = ptp.get_prop(
                    crate::ptp::DevicePropCode::FujiRawConversionProfile,
                )?;
                profile.try_update_from(partial)?;
                ptp.set_prop(
                    crate::ptp::DevicePropCode::FujiRawConversionProfile,
                    &profile,
                )?;
                <Self as crate::features::render::CameraRenderManager>::render_image(
                    self, ptp, draft,
                )
            }
        }
    }
}
