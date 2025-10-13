use super::Camera;

#[derive(Debug)]
pub struct FujifilmXT5;

impl Camera for FujifilmXT5 {
    fn vendor_id(&self) -> u16 {
        0x04cb
    }

    fn product_id(&self) -> u16 {
        0x02fc
    }

    fn name(&self) -> &'static str {
        "FUJIFILM X-T5"
    }
}
