use std::collections::BTreeMap;

use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;

use crate::{ast::Camera, snake_case_ident, upper_camel_case_ident, uppercase_ident};

struct CameraNames {
    r#struct: Ident,
    r#const: Ident,
    simulation_mod: Ident,
    simulation_const: Ident,
    simulation_draft: Ident,
    simulation_complete: Ident,
    render_mod: Ident,
    render_const: Ident,
    render_draft: Ident,
    render_complete: Ident,
}

impl From<&Camera> for CameraNames {
    fn from(camera: &Camera) -> Self {
        Self {
            r#struct: upper_camel_case_ident!("{}", camera.id),
            r#const: uppercase_ident!("C_{}", camera.id),
            simulation_mod: snake_case_ident!("{}", camera.id),
            simulation_const: uppercase_ident!("C_{}_SIMULATION", camera.id),
            simulation_draft: upper_camel_case_ident!("{}_simulation_draft", camera.id),
            simulation_complete: upper_camel_case_ident!("{}_simulation", camera.id),
            render_mod: snake_case_ident!("{}", camera.id),
            render_const: uppercase_ident!("C_{}_RENDER", camera.id),
            render_draft: upper_camel_case_ident!("{}_render_profile_draft", camera.id),
            render_complete: upper_camel_case_ident!("{}_render_profile", camera.id),
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
    let simulation_const = generate_simulation_descriptors_const(names, &features);
    let render_const = generate_render_descriptors_const(names, &features);
    let supported_const = generate_supported_camera_const(camera, names, &features);

    Ok(quote! {
        pub struct #struct_name;

        #base_impl
        #simulation_const
        #render_const
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

fn generate_supported_camera_const(
    camera: &Camera,
    names: &CameraNames,
    features: &Features,
) -> TokenStream {
    let const_name = &names.r#const;
    let struct_name = &names.r#struct;
    let name_str = &camera.spec.name;
    let vendor = Literal::u16_suffixed(camera.spec.usb.vendor_id);
    let product = Literal::u16_suffixed(camera.spec.usb.product_id);
    let simulation = if features.has_simulation {
        let simulation_const = &names.simulation_const;
        quote! { Some(&#simulation_const) }
    } else {
        quote! { None }
    };
    let render = if features.has_render {
        let render_const = &names.render_const;
        quote! { Some(&#render_const) }
    } else {
        quote! { None }
    };

    quote! {
        pub const #const_name: crate::SupportedCamera = crate::SupportedCamera {
            name: #name_str,
            usb_id: crate::UsbId { vendor: #vendor, product: #product },
            camera_factory: || Box::new(#struct_name),
            simulation: #simulation,
            render: #render,
        };
    }
}

fn generate_simulation_descriptors_const(
    names: &CameraNames,
    features: &Features,
) -> Option<TokenStream> {
    if !features.has_simulation {
        return None;
    }

    let const_name = &names.simulation_const;
    let complete = &names.simulation_complete;
    let draft = &names.simulation_draft;
    let r#mod = &names.simulation_mod;

    Some(quote! {
        pub const #const_name: crate::features::simulation::SimulationDescriptors =
            crate::features::simulation::SimulationDescriptors {
                fields: <crate::generated::simulations::#complete>::FIELDS,
                slots: <crate::generated::simulations::#complete>::SLOTS as usize,
                validate: |base| {
                    let draft = <crate::generated::simulations::#draft as ::std::convert::TryFrom<
                        crate::generated::simulations::SimulationBase,
                    >>::try_from(base)?;
                    let complete = <crate::generated::simulations::#complete as ::std::convert::TryFrom<
                        crate::generated::simulations::#draft,
                    >>::try_from(draft)?;
                    ::std::result::Result::Ok(
                        <crate::generated::simulations::SimulationBase as ::std::convert::From<
                            &crate::generated::simulations::#complete,
                        >>::from(&complete),
                    )
                },
                validate_partial: |base| {
                    let mut draft = <crate::generated::simulations::#draft as ::std::convert::TryFrom<
                        crate::generated::simulations::SimulationBase,
                    >>::try_from(base)?;
                    crate::generated::simulations::#r#mod::try_update_from(
                        &mut draft,
                        &<crate::generated::simulations::#draft as ::std::default::Default>::default(),
                    )?;
                    ::std::result::Result::Ok(
                        <crate::generated::simulations::SimulationBase as ::std::convert::From<
                            &crate::generated::simulations::#draft,
                        >>::from(&draft),
                    )
                },
            };
    })
}

fn generate_render_descriptors_const(
    names: &CameraNames,
    features: &Features,
) -> Option<TokenStream> {
    if !features.has_render {
        return None;
    }

    let const_name = &names.render_const;
    let complete = &names.render_complete;
    let draft = &names.render_draft;
    let r#mod = &names.render_mod;

    Some(quote! {
        pub const #const_name: crate::features::render::RenderDescriptors =
            crate::features::render::RenderDescriptors {
                fields: <crate::generated::renders::#complete>::FIELDS,
                validate: |base| {
                    let draft = <crate::generated::renders::#draft as ::std::convert::TryFrom<
                        crate::generated::renders::RenderBase,
                    >>::try_from(base)?;
                    let complete = <crate::generated::renders::#complete as ::std::convert::TryFrom<
                        crate::generated::renders::#draft,
                    >>::try_from(draft)?;
                    ::std::result::Result::Ok(
                        <crate::generated::renders::RenderBase as ::std::convert::From<
                            &crate::generated::renders::#complete,
                        >>::from(&complete),
                    )
                },
                validate_partial: |base| {
                    let mut draft = <crate::generated::renders::#draft as ::std::convert::TryFrom<
                        crate::generated::renders::RenderBase,
                    >>::try_from(base)?;
                    crate::generated::renders::#r#mod::try_update_from(
                        &mut draft,
                        &<crate::generated::renders::#draft as ::std::default::Default>::default(),
                    )?;
                    ::std::result::Result::Ok(
                        <crate::generated::renders::RenderBase as ::std::convert::From<
                            &crate::generated::renders::#draft,
                        >>::from(&draft),
                    )
                },
            };
    })
}
