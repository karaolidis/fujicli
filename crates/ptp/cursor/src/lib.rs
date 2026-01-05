#![allow(dead_code)]
#![allow(clippy::redundant_closure_for_method_calls)]

mod types;

pub use types::*;

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
        self.read_ptp_vec(|cur| cur.read_ptp_u8())
    }

    fn read_ptp_i8_vec(&mut self) -> io::Result<Vec<i8>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i8())
    }

    fn read_ptp_u16_vec(&mut self) -> io::Result<Vec<u16>> {
        self.read_ptp_vec(|cur| cur.read_ptp_u16())
    }

    fn read_ptp_i16_vec(&mut self) -> io::Result<Vec<i16>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i16())
    }

    fn read_ptp_u32_vec(&mut self) -> io::Result<Vec<u32>> {
        self.read_ptp_vec(|cur| cur.read_ptp_u32())
    }

    fn read_ptp_i32_vec(&mut self) -> io::Result<Vec<i32>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i32())
    }

    fn read_ptp_u64_vec(&mut self) -> io::Result<Vec<u64>> {
        self.read_ptp_vec(|cur| cur.read_ptp_u64())
    }

    fn read_ptp_i64_vec(&mut self) -> io::Result<Vec<i64>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i64())
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
        self.write_u32::<LittleEndian>(vec.len() as u32)?;
        for v in vec {
            func(self, v)?;
        }
        Ok(())
    }

    fn write_ptp_u8_vec(&mut self, vec: &[u8]) -> io::Result<()> {
        self.write_ptp_vec(vec, |cur, v| cur.write_ptp_u8(v))
    }

    fn write_ptp_i8_vec(&mut self, vec: &[i8]) -> io::Result<()> {
        self.write_ptp_vec(vec, |cur, v| cur.write_ptp_i8(v))
    }

    fn write_ptp_u16_vec(&mut self, vec: &[u16]) -> io::Result<()> {
        self.write_ptp_vec(vec, |cur, v| cur.write_ptp_u16(v))
    }

    fn write_ptp_i16_vec(&mut self, vec: &[i16]) -> io::Result<()> {
        self.write_ptp_vec(vec, |cur, v| cur.write_ptp_i16(v))
    }

    fn write_ptp_u32_vec(&mut self, vec: &[u32]) -> io::Result<()> {
        self.write_ptp_vec(vec, |cur, v| cur.write_ptp_u32(v))
    }

    fn write_ptp_i32_vec(&mut self, vec: &[i32]) -> io::Result<()> {
        self.write_ptp_vec(vec, |cur, v| cur.write_ptp_i32(v))
    }

    fn write_ptp_u64_vec(&mut self, vec: &[u64]) -> io::Result<()> {
        self.write_ptp_vec(vec, |cur, v| cur.write_ptp_u64(v))
    }

    fn write_ptp_i64_vec(&mut self, vec: &[i64]) -> io::Result<()> {
        self.write_ptp_vec(vec, |cur, v| cur.write_ptp_i64(v))
    }

    fn write_ptp_str(&mut self, s: &str) -> io::Result<()> {
        if s.is_empty() {
            return self.write_u8(0);
        }

        let utf16: Vec<u16> = s.encode_utf16().collect();
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
