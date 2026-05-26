mod types;

pub use types::ExactString;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor};

pub trait Read: ReadBytesExt {
    fn read_ptp_u8(&mut self) -> io::Result<u8> {
        self.read_u8()
    }

    fn read_ptp_i8(&mut self) -> io::Result<i8> {
        self.read_i8()
    }

    fn read_ptp_u16(&mut self) -> io::Result<u16> {
        self.read_u16::<LittleEndian>()
    }

    fn read_ptp_i16(&mut self) -> io::Result<i16> {
        self.read_i16::<LittleEndian>()
    }

    fn read_ptp_u32(&mut self) -> io::Result<u32> {
        self.read_u32::<LittleEndian>()
    }

    fn read_ptp_i32(&mut self) -> io::Result<i32> {
        self.read_i32::<LittleEndian>()
    }

    fn read_ptp_u64(&mut self) -> io::Result<u64> {
        self.read_u64::<LittleEndian>()
    }

    fn read_ptp_i64(&mut self) -> io::Result<i64> {
        self.read_i64::<LittleEndian>()
    }

    fn read_ptp_vec<T, F>(&mut self, func: F) -> io::Result<Vec<T>>
    where
        F: Fn(&mut Self) -> io::Result<T>,
    {
        let len = self.read_u32::<LittleEndian>()? as usize;
        (0..len).map(|_| func(self)).collect()
    }

    fn read_ptp_u8_vec(&mut self) -> io::Result<Vec<u8>> {
        self.read_ptp_vec(Self::read_ptp_u8)
    }

    fn read_ptp_i8_vec(&mut self) -> io::Result<Vec<i8>> {
        self.read_ptp_vec(Self::read_ptp_i8)
    }

    fn read_ptp_u16_vec(&mut self) -> io::Result<Vec<u16>> {
        self.read_ptp_vec(Self::read_ptp_u16)
    }

    fn read_ptp_i16_vec(&mut self) -> io::Result<Vec<i16>> {
        self.read_ptp_vec(Self::read_ptp_i16)
    }

    fn read_ptp_u32_vec(&mut self) -> io::Result<Vec<u32>> {
        self.read_ptp_vec(Self::read_ptp_u32)
    }

    fn read_ptp_i32_vec(&mut self) -> io::Result<Vec<i32>> {
        self.read_ptp_vec(Self::read_ptp_i32)
    }

    fn read_ptp_u64_vec(&mut self) -> io::Result<Vec<u64>> {
        self.read_ptp_vec(Self::read_ptp_u64)
    }

    fn read_ptp_i64_vec(&mut self) -> io::Result<Vec<i64>> {
        self.read_ptp_vec(Self::read_ptp_i64)
    }

    fn read_ptp_str(&mut self) -> io::Result<String> {
        let len = self.read_u8()?;
        if len == 0 {
            return Ok(String::new());
        }

        let data: Vec<u16> = (0..(len - 1))
            .map(|_| self.read_u16::<LittleEndian>())
            .collect::<io::Result<_>>()?;
        self.read_u16::<LittleEndian>()?;

        String::from_utf16(&data)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-16"))
    }

    fn read_ptp_str_exact(&mut self) -> io::Result<ExactString> {
        let len = self.read_u8()?;
        if len == 0 {
            return Ok(ExactString::new(String::new()));
        }

        // For strings that do not include a null terminator
        let data: Vec<u16> = (0..len)
            .map(|_| self.read_u16::<LittleEndian>())
            .collect::<io::Result<_>>()?;

        let s = String::from_utf16(&data)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-16"))?;
        Ok(ExactString::new(s))
    }

    fn expect_end(&mut self) -> io::Result<()>;
}

impl<T: AsRef<[u8]>> Read for Cursor<T> {
    fn expect_end(&mut self) -> io::Result<()> {
        let len = self.get_ref().as_ref().len();
        if len as u64 != self.position() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "Buffer contained {} bytes, expected {} bytes",
                    len,
                    self.position()
                ),
            ));
        }
        Ok(())
    }
}

pub trait Write: WriteBytesExt {
    fn write_ptp_u8(&mut self, v: &u8) -> io::Result<()> {
        self.write_u8(*v)
    }

    fn write_ptp_i8(&mut self, v: &i8) -> io::Result<()> {
        self.write_i8(*v)
    }

    fn write_ptp_u16(&mut self, v: &u16) -> io::Result<()> {
        self.write_u16::<LittleEndian>(*v)
    }

    fn write_ptp_i16(&mut self, v: &i16) -> io::Result<()> {
        self.write_i16::<LittleEndian>(*v)
    }

    fn write_ptp_u32(&mut self, v: &u32) -> io::Result<()> {
        self.write_u32::<LittleEndian>(*v)
    }

    fn write_ptp_i32(&mut self, v: &i32) -> io::Result<()> {
        self.write_i32::<LittleEndian>(*v)
    }

    fn write_ptp_u64(&mut self, v: &u64) -> io::Result<()> {
        self.write_u64::<LittleEndian>(*v)
    }

    fn write_ptp_i64(&mut self, v: &i64) -> io::Result<()> {
        self.write_i64::<LittleEndian>(*v)
    }

    fn write_ptp_vec<T, F>(&mut self, vec: &[T], func: F) -> io::Result<()>
    where
        F: Fn(&mut Self, &T) -> io::Result<()>,
    {
        #[allow(clippy::cast_possible_truncation)]
        self.write_u32::<LittleEndian>(vec.len() as u32)?;
        for v in vec {
            func(self, v)?;
        }
        Ok(())
    }

    fn write_ptp_u8_vec(&mut self, vec: &[u8]) -> io::Result<()> {
        self.write_ptp_vec(vec, Self::write_ptp_u8)
    }

    fn write_ptp_i8_vec(&mut self, vec: &[i8]) -> io::Result<()> {
        self.write_ptp_vec(vec, Self::write_ptp_i8)
    }

    fn write_ptp_u16_vec(&mut self, vec: &[u16]) -> io::Result<()> {
        self.write_ptp_vec(vec, Self::write_ptp_u16)
    }

    fn write_ptp_i16_vec(&mut self, vec: &[i16]) -> io::Result<()> {
        self.write_ptp_vec(vec, Self::write_ptp_i16)
    }

    fn write_ptp_u32_vec(&mut self, vec: &[u32]) -> io::Result<()> {
        self.write_ptp_vec(vec, Self::write_ptp_u32)
    }

    fn write_ptp_i32_vec(&mut self, vec: &[i32]) -> io::Result<()> {
        self.write_ptp_vec(vec, Self::write_ptp_i32)
    }

    fn write_ptp_u64_vec(&mut self, vec: &[u64]) -> io::Result<()> {
        self.write_ptp_vec(vec, Self::write_ptp_u64)
    }

    fn write_ptp_i64_vec(&mut self, vec: &[i64]) -> io::Result<()> {
        self.write_ptp_vec(vec, Self::write_ptp_i64)
    }

    fn write_ptp_str(&mut self, s: &str) -> io::Result<()> {
        if s.is_empty() {
            return self.write_u8(0);
        }

        let utf16: Vec<u16> = s.encode_utf16().collect();
        #[allow(clippy::cast_possible_truncation)]
        self.write_u8((utf16.len() + 1) as u8)?;
        for c in utf16 {
            self.write_u16::<LittleEndian>(c)?;
        }
        self.write_u16::<LittleEndian>(0)?;
        Ok(())
    }

    fn write_ptp_str_exact(&mut self, s: &str) -> io::Result<()> {
        if s.is_empty() {
            return self.write_u8(0);
        }

        let utf16: Vec<u16> = s.encode_utf16().collect();
        #[allow(clippy::cast_possible_truncation)]
        self.write_u8((utf16.len()) as u8)?;
        for c in utf16 {
            self.write_u16::<LittleEndian>(c)?;
        }

        Ok(())
    }
}

impl Write for Vec<u8> {}

pub trait PtpSerialize {
    fn try_into_ptp(&self) -> io::Result<Vec<u8>>;

    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()>;
}

pub trait PtpDeserialize: Sized {
    fn try_from_ptp(buf: &[u8]) -> io::Result<Self>;

    fn try_read_ptp<R: Read>(cur: &mut R) -> io::Result<Self>;
}

macro_rules! ptp_ser {
    ($ty:ty, $write_fn:ident) => {
        impl PtpSerialize for $ty {
            fn try_into_ptp(&self) -> io::Result<Vec<u8>> {
                let mut buf = Vec::new();
                self.try_write_ptp(&mut buf)?;
                Ok(buf)
            }

            fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()> {
                buf.$write_fn(self)
            }
        }
    };
}

macro_rules! ptp_de {
    ($ty:ty, $read_fn:ident) => {
        impl PtpDeserialize for $ty {
            fn try_from_ptp(buf: &[u8]) -> io::Result<Self> {
                let mut cur = Cursor::new(buf);
                let val = Self::try_read_ptp(&mut cur)?;
                cur.expect_end()?;
                Ok(val)
            }

            fn try_read_ptp<R: Read>(cur: &mut R) -> io::Result<Self> {
                cur.$read_fn()
            }
        }
    };
}

ptp_ser!(u8, write_ptp_u8);
ptp_de!(u8, read_ptp_u8);
ptp_ser!(i8, write_ptp_i8);
ptp_de!(i8, read_ptp_i8);
ptp_ser!(u16, write_ptp_u16);
ptp_de!(u16, read_ptp_u16);
ptp_ser!(i16, write_ptp_i16);
ptp_de!(i16, read_ptp_i16);
ptp_ser!(u32, write_ptp_u32);
ptp_de!(u32, read_ptp_u32);
ptp_ser!(i32, write_ptp_i32);
ptp_de!(i32, read_ptp_i32);
ptp_ser!(u64, write_ptp_u64);
ptp_de!(u64, read_ptp_u64);
ptp_ser!(i64, write_ptp_i64);
ptp_de!(i64, read_ptp_i64);
ptp_ser!(&str, write_ptp_str);
ptp_ser!(String, write_ptp_str);
ptp_de!(String, read_ptp_str);
ptp_ser!(ExactString, write_ptp_str_exact);
ptp_de!(ExactString, read_ptp_str_exact);
ptp_ser!(Vec<u8>, write_ptp_u8_vec);
ptp_de!(Vec<u8>, read_ptp_u8_vec);
ptp_ser!(Vec<i8>, write_ptp_i8_vec);
ptp_de!(Vec<i8>, read_ptp_i8_vec);
ptp_ser!(Vec<u16>, write_ptp_u16_vec);
ptp_de!(Vec<u16>, read_ptp_u16_vec);
ptp_ser!(Vec<i16>, write_ptp_i16_vec);
ptp_de!(Vec<i16>, read_ptp_i16_vec);
ptp_ser!(Vec<u32>, write_ptp_u32_vec);
ptp_de!(Vec<u32>, read_ptp_u32_vec);
ptp_ser!(Vec<i32>, write_ptp_i32_vec);
ptp_de!(Vec<i32>, read_ptp_i32_vec);
ptp_ser!(Vec<u64>, write_ptp_u64_vec);
ptp_de!(Vec<u64>, read_ptp_u64_vec);
ptp_ser!(Vec<i64>, write_ptp_i64_vec);
ptp_de!(Vec<i64>, read_ptp_i64_vec);

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{ExactString, PtpDeserialize, PtpSerialize, Read};

    macro_rules! check_ptp_round_trip {
        ($ty:ty, $value:expr, $bytes:expr) => {{
            let value: $ty = $value;
            let bytes = value.try_into_ptp().unwrap();
            assert_eq!(bytes, $bytes, "wire bytes for {}", stringify!($ty));

            let parsed = <$ty>::try_from_ptp(&bytes).unwrap();
            assert_eq!(parsed, value, "round-trip for {}", stringify!($ty));
        }};
    }

    #[test]
    fn u8_wire_format() {
        check_ptp_round_trip!(u8, 0x42, vec![0x42]);
    }

    #[test]
    fn i8_negative_wire_format() {
        check_ptp_round_trip!(i8, -1, vec![0xFF]);
    }

    #[test]
    fn u16_little_endian() {
        check_ptp_round_trip!(u16, 0x1234, vec![0x34, 0x12]);
    }

    #[test]
    fn i16_little_endian_negative() {
        check_ptp_round_trip!(i16, -1, vec![0xFF, 0xFF]);
    }

    #[test]
    fn u32_little_endian() {
        check_ptp_round_trip!(u32, 0x1234_5678, vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn i32_little_endian_negative() {
        #[allow(clippy::cast_sign_loss)]
        let bytes = ((-3000i32) as u32).to_le_bytes().to_vec();
        check_ptp_round_trip!(i32, -3000, bytes);
    }

    #[test]
    fn u64_little_endian() {
        check_ptp_round_trip!(
            u64,
            0x0102_0304_0506_0708,
            vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
    }

    #[test]
    fn i64_little_endian() {
        check_ptp_round_trip!(i64, -1, vec![0xFF; 8]);
    }

    #[test]
    fn vec_u8_includes_u32_length_prefix() {
        let bytes = vec![0xAAu8, 0xBB, 0xCC].try_into_ptp().unwrap();
        assert_eq!(bytes, vec![0x03, 0x00, 0x00, 0x00, 0xAA, 0xBB, 0xCC]);

        let parsed: Vec<u8> = Vec::<u8>::try_from_ptp(&bytes).unwrap();
        assert_eq!(parsed, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn vec_u16_round_trips() {
        let bytes = vec![0x0001u16, 0x0203, 0xFFFF].try_into_ptp().unwrap();
        assert_eq!(
            bytes,
            vec![
                0x03, 0x00, 0x00, 0x00, // length = 3
                0x01, 0x00, // 0x0001 LE
                0x03, 0x02, // 0x0203 LE
                0xFF, 0xFF, // 0xFFFF LE
            ],
        );

        let parsed = Vec::<u16>::try_from_ptp(&bytes).unwrap();
        assert_eq!(parsed, vec![0x0001, 0x0203, 0xFFFF]);
    }

    #[test]
    fn vec_empty_is_just_the_length_prefix() {
        let bytes = Vec::<u32>::new().try_into_ptp().unwrap();
        assert_eq!(bytes, vec![0x00, 0x00, 0x00, 0x00]);

        let parsed = Vec::<u32>::try_from_ptp(&bytes).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn vec_i32_signed_round_trip() {
        let original: Vec<i32> = vec![-3000, 0, 3000];
        let bytes = original.try_into_ptp().unwrap();
        let parsed = Vec::<i32>::try_from_ptp(&bytes).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn string_empty_is_single_zero_byte() {
        let bytes = String::new().try_into_ptp().unwrap();
        assert_eq!(bytes, vec![0x00]);

        let parsed = String::try_from_ptp(&bytes).unwrap();
        assert_eq!(parsed, "");
    }

    #[test]
    fn string_writes_length_then_utf16_then_null() {
        let bytes = "AB".to_string().try_into_ptp().unwrap();
        assert_eq!(
            bytes,
            vec![
                0x03, // length: 2 chars + null terminator
                0x41, 0x00, // 'A' UTF-16 LE
                0x42, 0x00, // 'B' UTF-16 LE
                0x00, 0x00, // null terminator UTF-16 LE
            ],
        );

        let parsed = String::try_from_ptp(&bytes).unwrap();
        assert_eq!(parsed, "AB");
    }

    #[test]
    fn exact_string_omits_null_terminator() {
        let bytes = ExactString::new("AB".to_string()).try_into_ptp().unwrap();
        assert_eq!(bytes, vec![0x02, 0x41, 0x00, 0x42, 0x00]);

        let parsed = ExactString::try_from_ptp(&bytes).unwrap();
        assert_eq!(parsed.as_ref(), "AB");
    }

    #[test]
    fn exact_string_empty_is_single_zero_byte() {
        let bytes = ExactString::new(String::new()).try_into_ptp().unwrap();
        assert_eq!(bytes, vec![0x00]);
    }

    #[test]
    fn try_from_ptp_rejects_buffer_with_trailing_bytes() {
        let bytes = vec![0x34, 0x12, 0xAA, 0xBB];
        let err = u16::try_from_ptp(&bytes).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn try_from_ptp_rejects_truncated_buffer() {
        let err = u32::try_from_ptp(&[0x01, 0x02]).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn try_read_ptp_consumes_only_its_own_bytes_from_a_cursor() {
        let buf = [0x34u8, 0x12, 0x78, 0x56, 0x34, 0x12];
        let mut cur = Cursor::new(&buf[..]);

        let a = u16::try_read_ptp(&mut cur).unwrap();
        let b = u32::try_read_ptp(&mut cur).unwrap();
        assert_eq!(a, 0x1234);
        assert_eq!(b, 0x1234_5678);
        cur.expect_end().unwrap();
    }

    #[test]
    fn exact_string_display_and_from_str() {
        let s: ExactString = "hello".parse().unwrap();
        assert_eq!(format!("{s}"), "hello");
        assert_eq!(s.into_inner(), "hello");
    }
}
