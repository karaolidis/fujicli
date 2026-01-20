pub mod x_trans;
pub mod x_trans_ii;
pub mod x_trans_iii;
pub mod x_trans_iv;
pub mod x_trans_v;

macro_rules! define_camera {
    (
        $name:literal,
        $struct_name:ident,
        $const_name:ident,
        $vendor:expr,
        $product:expr,
        $sensor:ident,
        [ $( $cap:ident ),* $(,)? ],
        $( $chunk:expr, )?
    ) => {
        pub struct $struct_name;

        pub const $const_name: crate::SupportedCamera = crate::SupportedCamera {
            name: $name,
            vendor: $vendor,
            product: $product,
            camera_factory: || Box::new($struct_name {}),
        };

        crate::features::base::impl_camera_base!(
            $struct_name,
            &$const_name,
            [ $( $cap ),* ]
            $( , $chunk )?
        );

        impl $sensor for $struct_name {}
    };
}

pub(crate) use define_camera;
