use anyhow::{Context, bail};
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;

pub fn resolve_enum_repr_signed(values: &[i32]) -> anyhow::Result<bool> {
    if values.is_empty() {
        return Ok(false);
    }

    let min = values.iter().min().expect("values is non-empty");
    let max = values.iter().max().expect("values is non-empty");
    resolve_numeric_repr_signed(*min, *max)
}

pub fn resolve_numeric_repr_signed(min: i32, max: i32) -> anyhow::Result<bool> {
    let negative = min < 0;
    let above_i16_max = max > i32::from(i16::MAX);
    let below_i16_min = min < i32::from(i16::MIN);
    let above_u16_max = max > i32::from(u16::MAX);

    if below_i16_min || above_u16_max {
        bail!(
            "wire value range [{}, {}] fits neither i16 nor u16",
            min,
            max
        );
    }

    if negative && above_i16_max {
        bail!(
            "wire values [{}, {}] mix negatives with values above i16::MAX",
            min,
            max
        );
    }

    Ok(negative)
}

pub fn resolve_repr_type(signed: bool) -> Ident {
    if signed {
        Ident::new("i16", Span::call_site())
    } else {
        Ident::new("u16", Span::call_site())
    }
}

pub fn resolve_repr_type_32(signed: bool) -> Ident {
    if signed {
        Ident::new("i32", Span::call_site())
    } else {
        Ident::new("u32", Span::call_site())
    }
}

pub fn wire_literal(value: i32, signed: bool) -> anyhow::Result<Literal> {
    if signed {
        let val: i16 = value
            .try_into()
            .with_context(|| format!("wire value {value} does not fit in i16"))?;
        Ok(Literal::i16_suffixed(val))
    } else {
        let val: u16 = value
            .try_into()
            .with_context(|| format!("wire value {value} does not fit in u16"))?;
        Ok(Literal::u16_suffixed(val))
    }
}

pub fn generate_try_from_wire_impl(
    type_name: &Ident,
    signed: bool,
    repr_type: &Ident,
    items: &[(Ident, Vec<i32>)],
) -> anyhow::Result<TokenStream> {
    let arms = items
        .iter()
        .map(|(variant, wires)| {
            let lits = wires
                .iter()
                .map(|w| wire_literal(*w, signed))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(quote! { #(#lits)|* => Ok(Self::#variant), })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(quote! {
        impl #type_name {
            fn try_from_wire(value: #repr_type) -> ::std::io::Result<Self> {
                match value {
                    #(#arms)*
                    _ => Err(::std::io::Error::new(
                        ::std::io::ErrorKind::InvalidData,
                        format!(
                            "Invalid {} discriminant {}",
                            stringify!(#type_name), value,
                        ),
                    )),
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::resolve_enum_repr_signed;

    #[test]
    fn test_empty_values_defaults_to_unsigned() {
        let values: [i32; 0] = [];
        let result = resolve_enum_repr_signed(&values);
        assert!(!result.unwrap());
    }

    #[test]
    fn test_all_in_i16_positive_range_defaults_to_unsigned() {
        let values = [0, 100, i16::MAX as i32];
        assert!(!resolve_enum_repr_signed(&values).unwrap());
    }

    #[test]
    fn test_any_negative_forces_signed() {
        let values = [-3000, 0, 100];
        assert!(resolve_enum_repr_signed(&values).unwrap());
    }

    #[test]
    fn test_any_above_i16_max_forces_unsigned() {
        let values = [0, 40000];
        assert!(!resolve_enum_repr_signed(&values).unwrap());
    }

    #[test]
    fn test_mixed_negatives_and_large_positives_fails() {
        let values = [-1, 40000];
        let result = resolve_enum_repr_signed(&values);
        assert!(result.is_err());
    }

    #[test]
    fn test_out_of_absolute_bounds_fails() {
        assert!(resolve_enum_repr_signed(&[-32769]).is_err());
        assert!(resolve_enum_repr_signed(&[65536]).is_err());
    }

    #[test]
    fn test_boundaries() {
        assert!(resolve_enum_repr_signed(&[i16::MIN as i32]).unwrap());
        assert!(!resolve_enum_repr_signed(&[u16::MAX as i32]).unwrap());
    }
}
