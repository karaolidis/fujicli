use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    common::descriptors::common::generate_descriptor, snake_case_ident, upper_camel_case_ident,
};

pub fn generate(id: &str, name: &str) -> TokenStream {
    let type_ident = upper_camel_case_ident!("{}", id);
    let field_ident = snake_case_ident!("{}", id);

    let ops = quote! {
        crate::features::simulation::OptionOps::Float(
            crate::features::simulation::FloatOps {
                min:  crate::generated::options::#type_ident::MIN,
                max:  crate::generated::options::#type_ident::MAX,
                step: crate::generated::options::#type_ident::STEP,
                jump: crate::generated::options::#type_ident::STEP * 10.0f32,
                step_fn: |base, dir, mag, validator| {
                    let ::std::option::Option::Some(cur) = base.#field_ident.map(f32::from) else {
                        return ::std::result::Result::Err(
                            crate::features::simulation::BumpError::Unset,
                        );
                    };
                    let stride = match mag {
                        crate::features::simulation::Magnitude::Single =>
                            crate::generated::options::#type_ident::STEP,
                        crate::features::simulation::Magnitude::Big =>
                            crate::generated::options::#type_ident::STEP * 10.0f32,
                    };
                    let signed = match dir {
                        crate::features::simulation::Direction::Next => stride,
                        crate::features::simulation::Direction::Prev => -stride,
                    };
                    let min = crate::generated::options::#type_ident::MIN;
                    let max = crate::generated::options::#type_ident::MAX;
                    let raw_target = cur + signed;
                    let (mut try_val, walk_step) = if (min..=max).contains(&raw_target) {
                        (raw_target, signed)
                    } else {
                        let clamped = raw_target.clamp(min, max);
                        #[allow(clippy::float_cmp)]
                        if clamped == cur {
                            return ::std::result::Result::Err(
                                crate::features::simulation::BumpError::Exhausted,
                            );
                        }
                        let inward = match dir {
                            crate::features::simulation::Direction::Next =>
                                -crate::generated::options::#type_ident::STEP,
                            crate::features::simulation::Direction::Prev =>
                                crate::generated::options::#type_ident::STEP,
                        };
                        (clamped, inward)
                    };
                    #[allow(clippy::float_cmp)]
                    while (min..=max).contains(&try_val) && try_val != cur {
                        if let ::std::result::Result::Ok(want) = <
                            crate::generated::options::#type_ident as ::std::convert::TryFrom<f32>
                        >::try_from(try_val) {
                            let mut candidate = base.clone();
                            candidate.#field_ident = ::std::option::Option::Some(want);
                            if let ::std::option::Option::Some(v) = validator(candidate)
                                && v.#field_ident == ::std::option::Option::Some(want)
                            {
                                *base = v;
                                return ::std::result::Result::Ok(());
                            }
                        }
                        try_val += walk_step;
                    }
                    ::std::result::Result::Err(crate::features::simulation::BumpError::Exhausted)
                },
                jump_fn: |base, ext, validator| {
                    let (mut try_val, signed) = match ext {
                        crate::features::simulation::Extreme::Min => (
                            crate::generated::options::#type_ident::MIN,
                            crate::generated::options::#type_ident::STEP,
                        ),
                        crate::features::simulation::Extreme::Max => (
                            crate::generated::options::#type_ident::MAX,
                            -crate::generated::options::#type_ident::STEP,
                        ),
                    };
                    while (
                        crate::generated::options::#type_ident::MIN
                        ..=crate::generated::options::#type_ident::MAX
                    ).contains(&try_val) {
                        if let ::std::result::Result::Ok(want) = <
                            crate::generated::options::#type_ident as ::std::convert::TryFrom<f32>
                        >::try_from(try_val) {
                            let mut candidate = base.clone();
                            candidate.#field_ident = ::std::option::Option::Some(want);
                            if let ::std::option::Option::Some(v) = validator(candidate)
                                && v.#field_ident == ::std::option::Option::Some(want)
                            {
                                *base = v;
                                return ::std::result::Result::Ok(());
                            }
                        }
                        try_val += signed;
                    }
                    ::std::result::Result::Err(crate::features::simulation::BumpError::Exhausted)
                },
                set_default: |base| {
                    base.#field_ident = ::std::option::Option::Some(
                        <crate::generated::options::#type_ident as ::std::default::Default>::default(),
                    );
                },
            }
        )
    };

    generate_descriptor(id, name, &ops, true)
}
