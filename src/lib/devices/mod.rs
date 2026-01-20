pub mod x_trans;
pub mod x_trans_ii;
pub mod x_trans_iii;
pub mod x_trans_iv;
pub mod x_trans_v;

macro_rules! impl_camera_base {
    (
        camera = $camera:ty,
        definition = $def:expr,
        context = $ctx:ty,
        capabilities = [ $( $cap:ident ),* $(,)? ],
        $( chunk_size = $chunk:expr, )?
    ) => {
        impl crate::features::base::CameraBase for $camera {
            type Context = $ctx;

            fn camera_definition(&self) -> &'static crate::SupportedCamera {
                $def
            }

            $(
                fn chunk_size(&self) -> usize {
                    $chunk
                }
            )?

            $(
                crate::devices::impl_camera_base!(@cap self, $ctx, $cap);
            )*
        }
    };

    (@cap $self:ident, $ctx:ty, CameraBackupManager) => {
        fn as_backup_manager(
            &$self,
        ) -> Option<&dyn crate::features::backup::CameraBackupManager<Context = $ctx>> {
            Some($self)
        }
    };

    (@cap $self:ident, $ctx:ty, CameraSimulationParser) => {
        fn as_simulation_parser(
            &$self,
        ) -> Option<&dyn crate::features::simulation::CameraSimulationParser> {
            Some($self)
        }
    };

    (@cap $self:ident, $ctx:ty, CameraSimulationManager) => {
        fn as_simulation_manager(
            &$self,
        ) -> Option<&dyn crate::features::simulation::CameraSimulationManager<Context = $ctx>> {
            Some($self)
        }
    };

    (@cap $self:ident, $ctx:ty, CameraRenderManager) => {
        fn as_render_manager(
            &$self,
        ) -> Option<&dyn crate::features::render::CameraRenderManager<Context = $ctx>> {
            Some($self)
        }
    };
}

pub(crate) use impl_camera_base;

macro_rules! define_camera {
    (
        $name:literal,
        $struct_name:ident,
        $const_name:ident,
        $vendor:expr,
        $product:expr,
        context = $ctx:ty,
        sensor = $sensor:ident,
        capabilities = [ $( $cap:ident ),* $(,)? ],
        $( chunk_size = $chunk:expr, )?
    ) => {
        pub struct $struct_name;

        pub const $const_name: crate::SupportedCamera = crate::SupportedCamera {
            name: $name,
            vendor: $vendor,
            product: $product,
            camera_factory: || Box::new($struct_name {}),
        };

        crate::devices::impl_camera_base!(
            camera = $struct_name,
            definition = &$const_name,
            context = $ctx,
            capabilities = [ $( $cap ),* ],
            $( chunk_size = $chunk, )?
        );

        impl $sensor for $struct_name {}
    };
}

pub(crate) use define_camera;
