pub mod render;
pub mod simulation;

pub mod x100vi;
pub mod x_h2;
pub mod x_h2s;
pub mod x_t5;

use crate::camera::features::base::CameraBase;

pub trait XTransV: CameraBase {}
