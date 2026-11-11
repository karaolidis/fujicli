use std::io;

use ptp_cursor::{ExactString, PtpDeserialize, PtpSerialize, Read};

/// Bytes the profile code occupies on the wire, including its padding.
const CODE_REGION_LEN: usize = 0x1FF;

/// The fields of a camera's conversion profile, in wire order.
pub trait ConversionProfileFields: Sized {
    const FIELD_COUNT: i16;

    fn write_fields(&self, buf: &mut Vec<u8>) -> io::Result<()>;

    fn read_fields<R: Read>(cur: &mut R) -> io::Result<Self>;
}

/// A conversion profile as exchanged with the camera.
#[derive(Debug, Clone)]
pub struct ConversionProfile<T> {
    pub code: ExactString,
    pub fields: T,
}

impl<T: ConversionProfileFields> PtpSerialize for ConversionProfile<T> {
    fn try_into_ptp(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.try_write_ptp(&mut buf)?;
        Ok(buf)
    }

    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()> {
        T::FIELD_COUNT.try_write_ptp(buf)?;

        let start = buf.len();
        self.code.try_write_ptp(buf)?;
        let padding = CODE_REGION_LEN
            .checked_sub(buf.len() - start)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("profile code `{}` is too long", self.code.as_str()),
                )
            })?;
        buf.resize(buf.len() + padding, 0);

        self.fields.write_fields(buf)
    }
}

impl<T: ConversionProfileFields> PtpDeserialize for ConversionProfile<T> {
    fn try_from_ptp(buf: &[u8]) -> io::Result<Self> {
        let mut cur = io::Cursor::new(buf);
        Self::try_read_ptp(&mut cur)
    }

    fn try_read_ptp<R: Read>(cur: &mut R) -> io::Result<Self> {
        let field_count = i16::try_read_ptp(cur)?;
        if field_count != T::FIELD_COUNT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "expected {} profile fields, got {field_count}",
                    T::FIELD_COUNT
                ),
            ));
        }

        let code = ExactString::try_read_ptp(cur)?;
        let code_len = code.encode_utf16().count() * 2 + 1;
        let padding = CODE_REGION_LEN.checked_sub(code_len).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("profile code `{}` is too long", code.as_str()),
            )
        })?;
        io::Read::read_exact(cur, &mut vec![0; padding])?;

        let fields = T::read_fields(cur)?;
        Ok(Self { code, fields })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Two(i32, i32);

    impl ConversionProfileFields for Two {
        const FIELD_COUNT: i16 = 2;

        fn write_fields(&self, buf: &mut Vec<u8>) -> io::Result<()> {
            self.0.try_write_ptp(buf)?;
            self.1.try_write_ptp(buf)
        }

        fn read_fields<R: Read>(cur: &mut R) -> io::Result<Self> {
            Ok(Self(i32::try_read_ptp(cur)?, i32::try_read_ptp(cur)?))
        }
    }

    #[test]
    fn round_trips_and_keeps_the_code() {
        let profile = ConversionProfile {
            code: ExactString::from(String::from("FF179502")),
            fields: Two(1, -2),
        };
        let bytes = profile.try_into_ptp().unwrap();
        assert_eq!(bytes.len(), 2 + CODE_REGION_LEN + 8);

        let read = ConversionProfile::<Two>::try_from_ptp(&bytes).unwrap();
        assert_eq!(read.code.as_str(), "FF179502");
        assert_eq!(read.fields, Two(1, -2));
    }

    #[test]
    fn rejects_wrong_field_count() {
        let mut bytes = ConversionProfile {
            code: ExactString::from(String::from("FF179502")),
            fields: Two(0, 0),
        }
        .try_into_ptp()
        .unwrap();
        bytes[0] = 3;
        assert!(ConversionProfile::<Two>::try_from_ptp(&bytes).is_err());
    }
}
