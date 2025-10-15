use anyhow::bail;
use rusb::GlobalContext;

use super::CameraImpl;

type ImplFactory<P> = fn() -> Box<dyn CameraImpl<P>>;

#[derive(Debug, Clone, Copy)]
pub struct SupportedCamera<P: rusb::UsbContext> {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
    pub impl_factory: ImplFactory<P>,
}

impl<P: rusb::UsbContext> SupportedCamera<P> {
    pub fn new_camera(&self, device: &rusb::Device<P>) -> anyhow::Result<Box<dyn CameraImpl<P>>> {
        let descriptor = device.device_descriptor()?;

        let matches =
            descriptor.vendor_id() == self.vendor && descriptor.product_id() == self.product;

        if !matches {
            bail!(
                "Device with vendor {:04x} and product {:04x} does not match {}",
                descriptor.vendor_id(),
                descriptor.product_id(),
                self.name
            );
        }

        Ok((self.impl_factory)())
    }
}

macro_rules! default_camera_impl {
    (
        $const_name:ident,
        $struct_name:ident,
        $vendor:expr,
        $product:expr,
        $display_name:expr
    ) => {
        pub const $const_name: SupportedCamera<GlobalContext> = SupportedCamera {
            name: $display_name,
            vendor: $vendor,
            product: $product,
            impl_factory: || Box::new($struct_name {}),
        };

        pub struct $struct_name {}

        impl crate::camera::CameraImpl<GlobalContext> for $struct_name {
            fn supported_camera(&self) -> &'static SupportedCamera<GlobalContext> {
                &$const_name
            }
        }
    };
}

default_camera_impl!(FUJIFILM_XT5, FujifilmXT5, 0x04cb, 0x02fc, "FUJIFILM XT-5");

pub const SUPPORTED: &[SupportedCamera<GlobalContext>] = &[FUJIFILM_XT5];
