#![allow(dead_code)]
#![allow(clippy::redundant_closure_for_method_calls)]

use std::io::Cursor;

use anyhow::bail;
use byteorder::{LittleEndian, ReadBytesExt};

pub trait Read: ReadBytesExt {
    fn read_ptp_u8(&mut self) -> anyhow::Result<u8> {
        Ok(self.read_u8()?)
    }

    fn read_ptp_i8(&mut self) -> anyhow::Result<i8> {
        Ok(self.read_i8()?)
    }

    fn read_ptp_u16(&mut self) -> anyhow::Result<u16> {
        Ok(self.read_u16::<LittleEndian>()?)
    }

    fn read_ptp_i16(&mut self) -> anyhow::Result<i16> {
        Ok(self.read_i16::<LittleEndian>()?)
    }

    fn read_ptp_u32(&mut self) -> anyhow::Result<u32> {
        Ok(self.read_u32::<LittleEndian>()?)
    }

    fn read_ptp_i32(&mut self) -> anyhow::Result<i32> {
        Ok(self.read_i32::<LittleEndian>()?)
    }

    fn read_ptp_u64(&mut self) -> anyhow::Result<u64> {
        Ok(self.read_u64::<LittleEndian>()?)
    }

    fn read_ptp_i64(&mut self) -> anyhow::Result<i64> {
        Ok(self.read_i64::<LittleEndian>()?)
    }

    fn read_ptp_u128(&mut self) -> anyhow::Result<u128> {
        Ok(self.read_u128::<LittleEndian>()?)
    }

    fn read_ptp_i128(&mut self) -> anyhow::Result<i128> {
        Ok(self.read_i128::<LittleEndian>()?)
    }

    fn read_ptp_vec<T: Sized, U: Fn(&mut Self) -> anyhow::Result<T>>(
        &mut self,
        func: U,
    ) -> anyhow::Result<Vec<T>> {
        let len = self.read_u32::<LittleEndian>()? as usize;
        (0..len).map(|_| func(self)).collect()
    }

    fn read_ptp_u8_vec(&mut self) -> anyhow::Result<Vec<u8>> {
        self.read_ptp_vec(|cur| cur.read_ptp_u8())
    }

    fn read_ptp_i8_vec(&mut self) -> anyhow::Result<Vec<i8>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i8())
    }

    fn read_ptp_u16_vec(&mut self) -> anyhow::Result<Vec<u16>> {
        self.read_ptp_vec(|cur| cur.read_ptp_u16())
    }

    fn read_ptp_i16_vec(&mut self) -> anyhow::Result<Vec<i16>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i16())
    }

    fn read_ptp_u32_vec(&mut self) -> anyhow::Result<Vec<u32>> {
        self.read_ptp_vec(|cur| cur.read_ptp_u32())
    }

    fn read_ptp_i32_vec(&mut self) -> anyhow::Result<Vec<i32>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i32())
    }

    fn read_ptp_u64_vec(&mut self) -> anyhow::Result<Vec<u64>> {
        self.read_ptp_vec(|cur| cur.read_ptp_u64())
    }

    fn read_ptp_i64_vec(&mut self) -> anyhow::Result<Vec<i64>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i64())
    }

    fn read_ptp_u128_vec(&mut self) -> anyhow::Result<Vec<u128>> {
        self.read_ptp_vec(|cur| cur.read_ptp_u128())
    }

    fn read_ptp_i128_vec(&mut self) -> anyhow::Result<Vec<i128>> {
        self.read_ptp_vec(|cur| cur.read_ptp_i128())
    }

    fn read_ptp_str(&mut self) -> anyhow::Result<String> {
        let len = self.read_u8()?;
        if len > 0 {
            let data: Vec<u16> = (0..(len - 1))
                .map(|_| self.read_u16::<LittleEndian>())
                .collect::<std::result::Result<_, _>>()?;
            self.read_u16::<LittleEndian>()?;
            Ok(String::from_utf16(&data)?)
        } else {
            Ok(String::new())
        }
    }

    fn expect_end(&mut self) -> anyhow::Result<()>;
}

impl<T: AsRef<[u8]>> Read for Cursor<T> {
    fn expect_end(&mut self) -> anyhow::Result<()> {
        let len = self.get_ref().as_ref().len();
        if len as u64 != self.position() {
            bail!(super::error::Error::Malformed(format!(
                "Response {} bytes, expected {} bytes",
                len,
                self.position()
            )))
        }

        Ok(())
    }
}
