use std::collections::BTreeMap;

use proc_macro2::{Literal, TokenStream};
use quote::quote;

use crate::{ast::Camera, upper_camel_case_ident, uppercase_ident};

pub fn generate(cameras: &BTreeMap<String, Camera>) -> anyhow::Result<TokenStream> {
    let mut sorted: Vec<&Camera> = cameras.values().collect();
    sorted.sort_by(|a, b| {
        (a.spec.generation.as_str(), a.id.as_str())
            .cmp(&(b.spec.generation.as_str(), b.id.as_str()))
    });

    let mut defs = Vec::new();
    let mut supported_entries = Vec::new();

    for camera in sorted {
        let struct_name = upper_camel_case_ident!("{}", camera.id);
        let const_name = uppercase_ident!("C_{}", camera.id);
        let name_str = &camera.spec.name;
        let vendor = Literal::u16_suffixed(camera.spec.usb.vendor_id);
        let product = Literal::u16_suffixed(camera.spec.usb.product_id);
        let chunk_size = Literal::usize_suffixed(camera.spec.usb.chunk_size.try_into()?);

        let features = camera.spec.features.as_ref();

        let has_backup = features.is_some_and(|f| f.backup);
        let has_simulation = features.and_then(|f| f.simulation.as_ref()).is_some();
        let has_render = features.and_then(|f| f.render.as_ref()).is_some();

        let backup_override = has_backup.then(|| {
            quote! {
                fn as_backup_manager(
                    &self,
                ) -> Option<&dyn crate::features::backup::CameraBackupManager<Context = Self::Context>> {
                    Some(self)
                }
            }
        });
        let simulation_override = has_simulation.then(|| {
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
        let render_override = has_render.then(|| {
            quote! {
                fn as_render_manager(
                    &self,
                ) -> Option<&dyn crate::features::render::CameraRenderManager<Context = Self::Context>> {
                    Some(self)
                }
            }
        });

        let mut capabilities: Vec<TokenStream> = Vec::new();
        if has_backup {
            capabilities.push(quote! { crate::error::Capability::BackupManagement });
        }
        if has_simulation {
            capabilities.push(quote! { crate::error::Capability::SimulationParsing });
            capabilities.push(quote! { crate::error::Capability::SimulationManagement });
        }
        if has_render {
            capabilities.push(quote! { crate::error::Capability::RenderManagement });
        }
        let capabilities = (!capabilities.is_empty()).then(|| {
            quote! {
                fn capabilities(&self) -> &'static [crate::error::Capability] {
                    &[ #( #capabilities, )* ]
                }
            }
        });

        defs.push(quote! {
            pub struct #struct_name;

            pub const #const_name: crate::SupportedCamera = crate::SupportedCamera {
                name: #name_str,
                vendor: #vendor,
                product: #product,
                camera_factory: || Box::new(#struct_name),
            };

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
        });
        supported_entries.push(quote! { #const_name });
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
