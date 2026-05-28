use std::collections::BTreeMap;

use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;

use crate::{ast::Camera, upper_camel_case_ident, uppercase_ident};

struct CameraNames {
    r#struct: Ident,
    r#const: Ident,
}

impl From<&Camera> for CameraNames {
    fn from(camera: &Camera) -> Self {
        Self {
            r#struct: upper_camel_case_ident!("{}", camera.id),
            r#const: uppercase_ident!("C_{}", camera.id),
        }
    }
}

struct Features {
    has_backup: bool,
    has_simulation: bool,
    has_render: bool,
}

impl From<&Camera> for Features {
    fn from(camera: &Camera) -> Self {
        let features = camera.spec.features.as_ref();
        Self {
            has_backup: features.is_some_and(|f| f.backup),
            has_simulation: features.and_then(|f| f.simulation.as_ref()).is_some(),
            has_render: features.and_then(|f| f.render.as_ref()).is_some(),
        }
    }
}

pub fn generate(cameras: &BTreeMap<String, Camera>) -> anyhow::Result<TokenStream> {
    let mut sorted: Vec<&Camera> = cameras.values().collect();
    sorted.sort_by(|a, b| {
        (a.spec.generation.as_str(), a.id.as_str())
            .cmp(&(b.spec.generation.as_str(), b.id.as_str()))
    });

    let mut defs = Vec::with_capacity(sorted.len());
    let mut supported_entries = Vec::with_capacity(sorted.len());

    for camera in sorted {
        let names = CameraNames::from(camera);
        defs.push(generate_one(camera, &names)?);
        let r#const = &names.r#const;
        supported_entries.push(quote! { #r#const });
    }

    Ok(quote! {
        //! Generated camera definitions and supported device registry. Do not edit.

        #(#defs)*

        pub const SUPPORTED: &[crate::SupportedCamera] = &[
            #(#supported_entries,)*
        ];
    })
}

pub fn path() -> TokenStream {
    quote! { crate::generated::cameras }
}

fn generate_one(camera: &Camera, names: &CameraNames) -> anyhow::Result<TokenStream> {
    let features = Features::from(camera);

    let struct_name = &names.r#struct;

    let base_impl = generate_base_impl(camera, names, &features)?;
    let supported_const = generate_supported_camera_const(camera, names);

    Ok(quote! {
        pub struct #struct_name;

        #base_impl
        #supported_const
    })
}

fn generate_base_impl(
    camera: &Camera,
    names: &CameraNames,
    features: &Features,
) -> anyhow::Result<TokenStream> {
    let struct_name = &names.r#struct;
    let const_name = &names.r#const;
    let chunk_size = Literal::usize_suffixed(camera.spec.usb.chunk_size.try_into()?);
    let capabilities = generate_capabilities(features);
    let (backup_override, simulation_override, render_override) =
        generate_feature_overrides(features);

    Ok(quote! {
        impl crate::features::base::CameraBase for #struct_name {
            type Context = rusb::GlobalContext;

            fn camera_definition(&self) -> &'static crate::SupportedCamera {
                &#const_name
            }

            fn chunk_size(&self) -> usize {
                #chunk_size
            }

            #capabilities
            #backup_override
            #simulation_override
            #render_override
        }
    })
}

fn generate_capabilities(features: &Features) -> Option<TokenStream> {
    let mut entries: Vec<TokenStream> = Vec::new();
    if features.has_backup {
        entries.push(quote! { crate::error::Capability::BackupManagement });
    }
    if features.has_simulation {
        entries.push(quote! { crate::error::Capability::SimulationParsing });
        entries.push(quote! { crate::error::Capability::SimulationManagement });
    }
    if features.has_render {
        entries.push(quote! { crate::error::Capability::RenderManagement });
    }
    if entries.is_empty() {
        return None;
    }
    Some(quote! {
        fn capabilities(&self) -> &'static [crate::error::Capability] {
            &[ #( #entries, )* ]
        }
    })
}

fn generate_feature_overrides(
    features: &Features,
) -> (
    Option<TokenStream>,
    Option<TokenStream>,
    Option<TokenStream>,
) {
    let backup = features.has_backup.then(|| {
        quote! {
            fn as_backup_manager(
                &self,
            ) -> Option<&dyn crate::features::backup::CameraBackupManager<Context = Self::Context>> {
                Some(self)
            }
        }
    });
    let simulation = features.has_simulation.then(|| {
        quote! {
            fn as_simulation_parser(
                &self,
            ) -> Option<&dyn crate::features::simulation::CameraSimulationParser> {
                Some(self)
            }

            fn as_simulation_manager(
                &self,
            ) -> Option<&dyn crate::features::simulation::CameraSimulationManager<Context = Self::Context>> {
                Some(self)
            }
        }
    });
    let render = features.has_render.then(|| {
        quote! {
            fn as_render_manager(
                &self,
            ) -> Option<&dyn crate::features::render::CameraRenderManager<Context = Self::Context>> {
                Some(self)
            }
        }
    });
    (backup, simulation, render)
}

fn generate_supported_camera_const(camera: &Camera, names: &CameraNames) -> TokenStream {
    let const_name = &names.r#const;
    let struct_name = &names.r#struct;
    let name_str = &camera.spec.name;
    let vendor = Literal::u16_suffixed(camera.spec.usb.vendor_id);
    let product = Literal::u16_suffixed(camera.spec.usb.product_id);

    quote! {
        pub const #const_name: crate::SupportedCamera = crate::SupportedCamera {
            name: #name_str,
            vendor: #vendor,
            product: #product,
            camera_factory: || Box::new(#struct_name),
        };
    }
}
