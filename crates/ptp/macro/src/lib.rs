use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(PtpSerialize)]
pub fn derive_ptp_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named) => {
                let fields = &named.named;

                let write_fields = fields.iter().map(|f| {
                    let name = &f.ident;
                    quote! {
                        self.#name.try_write_ptp(buf)?;
                    }
                });

                quote! {
                    impl ptp_cursor::PtpSerialize for #name {
                        fn try_into_ptp(&self) -> std::io::Result<Vec<u8>> {
                            let mut buf = Vec::new();
                            self.try_write_ptp(&mut buf)?;
                            Ok(buf)
                        }

                        fn try_write_ptp(&self, buf: &mut Vec<u8>) -> std::io::Result<()> {
                            #(#write_fields)*
                            Ok(())
                        }
                    }
                }
            }
            Fields::Unnamed(unnamed) => {
                let fields = &unnamed.unnamed;

                let write_fields = (0..fields.len()).map(|i| {
                    let idx = syn::Index::from(i);
                    quote! { self.#idx.try_write_ptp(buf)?; }
                });

                quote! {
                    impl ptp_cursor::PtpSerialize for #name {
                        fn try_into_ptp(&self) -> std::io::Result<Vec<u8>> {
                            let mut buf = Vec::new();
                            self.try_write_ptp(&mut buf)?;
                            Ok(buf)
                        }

                        fn try_write_ptp(&self, buf: &mut Vec<u8>) -> std::io::Result<()> {
                            #(#write_fields)*
                            Ok(())
                        }
                    }
                }
            }
            Fields::Unit => {
                quote! {
                    impl ptp_cursor::PtpSerialize for #name {
                        fn try_into_ptp(&self) -> std::io::Result<Vec<u8>> {
                            Ok(Vec::new())
                        }

                        fn try_write_ptp(&self, _buf: &mut Vec<u8>) -> std::io::Result<()> {
                            Ok(())
                        }
                    }
                }
            }
        },
        Data::Enum(_) => {
            let repr_ty = input
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("repr"))
                .find_map(|attr| attr.parse_args::<syn::Ident>().ok())
                .expect("Enums must have a #[repr(T)] attribute for PtpSerialize");

            quote! {
                impl ptp_cursor::PtpSerialize for #name
                where
                    #name: Clone + Copy + TryFrom<#repr_ty> + Into<#repr_ty>
                {
                    fn try_into_ptp(&self) -> std::io::Result<Vec<u8>> {
                        let mut buf = Vec::new();
                        self.try_write_ptp(&mut buf)?;
                        Ok(buf)
                    }

                    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> std::io::Result<()> {
                        let discriminant: #repr_ty = (*self).into();
                        discriminant.try_write_ptp(buf)?;
                        Ok(())
                    }
                }
            }
        }
        _ => {
            unimplemented!("PtpSerialize cannot be automatically derived for unions")
        }
    };

    expanded.into()
}

#[proc_macro_derive(PtpDeserialize)]
pub fn derive_ptp_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named) => {
                let fields = &named.named;

                let read_fields = fields.iter().map(|f| {
                    let name = &f.ident;
                    let ty = &f.ty;
                    quote! {
                        #name: <#ty>::try_read_ptp(cur)?
                    }
                });

                quote! {
                    impl ptp_cursor::PtpDeserialize for #name {
                        fn try_from_ptp(buf: &[u8]) -> std::io::Result<Self> {
                            use ptp_cursor::Read;

                            let mut cur = std::io::Cursor::new(buf);
                            let val = Self::try_read_ptp(&mut cur)?;
                            cur.expect_end()?;
                            Ok(val)
                        }

                        fn try_read_ptp<R: ptp_cursor::Read>(cur: &mut R) -> std::io::Result<Self> {
                            Ok(Self { #(#read_fields),* })
                        }
                    }
                }
            }
            Fields::Unnamed(unnamed) => {
                let fields = &unnamed.unnamed;

                let read_fields = fields.iter().map(|f| {
                    let ty = &f.ty;
                    quote! { <#ty>::try_read_ptp(cur)? }
                });

                quote! {
                    impl ptp_cursor::PtpDeserialize for #name {
                        fn try_from_ptp(buf: &[u8]) -> std::io::Result<Self> {
                            use ptp_cursor::Read;

                            let mut cur = std::io::Cursor::new(buf);
                            let val = Self::try_read_ptp(&mut cur)?;
                            cur.expect_end()?;
                            Ok(val)
                        }

                        fn try_read_ptp<R: ptp_cursor::Read>(cur: &mut R) -> std::io::Result<Self> {
                            Ok(Self(#(#read_fields),*))
                        }
                    }
                }
            }
            Fields::Unit => {
                quote! {
                    impl ptp_cursor::PtpDeserialize for #name {
                        fn try_from_ptp(buf: &[u8]) -> std::io::Result<Self> {
                            use ptp_cursor::Read;

                            let mut cur = std::io::Cursor::new(buf);
                            cur.expect_end()?;
                            Ok(Self)
                        }

                        fn try_read_ptp<R: ptp_cursor::Read>(_cur: &mut R) -> std::io::Result<Self> {
                            Ok(Self)
                        }
                    }
                }
            }
        },
        Data::Enum(_) => {
            let repr_ty = input
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("repr"))
                .find_map(|attr| attr.parse_args::<syn::Ident>().ok())
                .expect("Enums must have a #[repr(T)] attribute for PtpSerialize");

            quote! {
                impl ptp_cursor::PtpDeserialize for #name
                where
                    #name: Clone + Copy + TryFrom<#repr_ty> + Into<#repr_ty>
                {
                    fn try_from_ptp(buf: &[u8]) -> std::io::Result<Self> {
                        use ptp_cursor::Read;

                        let mut cur = std::io::Cursor::new(buf);
                        let val = Self::try_read_ptp(&mut cur)?;
                        cur.expect_end()?;
                        Ok(val)
                    }

                    fn try_read_ptp<R: ptp_cursor::Read>(cur: &mut R) -> std::io::Result<Self> {
                        let discriminant = <#repr_ty>::try_read_ptp(cur)?;
                        discriminant.try_into().map_err(|_| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("Invalid discriminant for {}: {:?}", stringify!(#name), discriminant)
                            )
                        })
                    }
                }
            }
        }
        _ => {
            unimplemented!("PtpDeserialize cannot be automatically derived for unions")
        }
    };

    expanded.into()
}
