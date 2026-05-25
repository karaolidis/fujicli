use std::io::{self, Cursor};

use ptp_cursor::{PtpDeserialize, PtpSerialize, Read};

use crate::ptp::Ptp;

pub trait SimulationSetting: PtpSerialize + PtpDeserialize {
    fn prop_code() -> u16;

    fn try_push(&self, ptp: &mut Ptp) -> anyhow::Result<()> {
        ptp.set_prop(Self::prop_code(), self)
    }

    fn try_pull(ptp: &mut Ptp) -> anyhow::Result<Self> {
        ptp.get_prop(Self::prop_code())
    }
}

pub trait ConversionProfileField: Sized {
    fn try_into_conversion_profile_field_ptp(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.try_write_conversion_profile_field_ptp(&mut buf)?;
        Ok(buf)
    }

    fn try_write_conversion_profile_field_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()>;

    fn try_from_conversion_profile_field_ptp(buf: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(buf);
        let val = Self::try_read_conversion_profile_field_ptp(&mut cur)?;
        cur.expect_end()?;
        Ok(val)
    }

    fn try_read_conversion_profile_field_ptp<R: Read>(cur: &mut R) -> io::Result<Self>;
}
