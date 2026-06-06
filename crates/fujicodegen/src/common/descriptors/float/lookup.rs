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
                min:  crate::generated::options::#type_ident::LOGICAL_MIN,
                max:  crate::generated::options::#type_ident::LOGICAL_MAX,
                step: 1.0f32,
                jump: 10.0f32,
                step_fn: |base, dir, mag, validator| {
                    let ::std::option::Option::Some(current) = base.#field_ident else {
                        return ::std::result::Result::Err(
                            crate::features::simulation::BumpError::Unset,
                        );
                    };
                    let variants: ::std::vec::Vec<_> = <
                        crate::generated::options::#type_ident as ::strum::IntoEnumIterator
                    >::iter().collect();
                    let n = variants.len();
                    let cur_idx = variants
                        .iter()
                        .position(|v| *v == current)
                        .expect("current value is a variant");
                    let stride = match mag {
                        crate::features::simulation::Magnitude::Single => 1usize,
                        crate::features::simulation::Magnitude::Big => 10usize,
                    };
                    let (mut idx, walk_inward) = match dir {
                        crate::features::simulation::Direction::Next => {
                            let raw = cur_idx.saturating_add(stride);
                            if raw < n {
                                (raw, false)
                            } else if cur_idx + 1 < n {
                                (n - 1, true)
                            } else {
                                return ::std::result::Result::Err(
                                    crate::features::simulation::BumpError::Exhausted,
                                );
                            }
                        }
                        crate::features::simulation::Direction::Prev => {
                            match cur_idx.checked_sub(stride) {
                                ::std::option::Option::Some(raw) => (raw, false),
                                ::std::option::Option::None if cur_idx > 0 => (0usize, true),
                                ::std::option::Option::None => {
                                    return ::std::result::Result::Err(
                                        crate::features::simulation::BumpError::Exhausted,
                                    );
                                }
                            }
                        }
                    };
                    loop {
                        let want = variants[idx];
                        let mut candidate = base.clone();
                        candidate.#field_ident = ::std::option::Option::Some(want);
                        if let ::std::option::Option::Some(v) = validator(candidate)
                            && v.#field_ident == ::std::option::Option::Some(want)
                        {
                            *base = v;
                            return ::std::result::Result::Ok(());
                        }
                        let next = match (dir, walk_inward) {
                            (crate::features::simulation::Direction::Next, false) => {
                                idx.checked_add(stride).filter(|i| *i < n)
                            }
                            (crate::features::simulation::Direction::Next, true) => {
                                idx.checked_sub(1).filter(|i| *i > cur_idx)
                            }
                            (crate::features::simulation::Direction::Prev, false) => {
                                idx.checked_sub(stride)
                            }
                            (crate::features::simulation::Direction::Prev, true) => {
                                ::std::option::Option::Some(idx + 1).filter(|i| *i < cur_idx)
                            }
                        };
                        match next {
                            ::std::option::Option::Some(ni) => idx = ni,
                            ::std::option::Option::None => break,
                        }
                    }
                    ::std::result::Result::Err(crate::features::simulation::BumpError::Exhausted)
                },
                jump_fn: |base, ext, validator| {
                    let variants: ::std::vec::Vec<_> = <
                        crate::generated::options::#type_ident as ::strum::IntoEnumIterator
                    >::iter().collect();
                    let probe = |base: &mut crate::generated::simulations::SimulationBase,
                                 want: crate::generated::options::#type_ident| -> bool {
                        let mut candidate = base.clone();
                        candidate.#field_ident = ::std::option::Option::Some(want);
                        if let ::std::option::Option::Some(v) = validator(candidate)
                            && v.#field_ident == ::std::option::Option::Some(want)
                        {
                            *base = v;
                            return true;
                        }
                        false
                    };
                    match ext {
                        crate::features::simulation::Extreme::Min => {
                            for &value in &variants {
                                if probe(base, value) {
                                    return ::std::result::Result::Ok(());
                                }
                            }
                        }
                        crate::features::simulation::Extreme::Max => {
                            for &value in variants.iter().rev() {
                                if probe(base, value) {
                                    return ::std::result::Result::Ok(());
                                }
                            }
                        }
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
