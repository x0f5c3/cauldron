use num_traits::ToPrimitive;
use std::io;
use std::io::{ErrorKind, Write};

/// Extends the functionality of `io::Write` with additional methods.
pub trait WriteBuffer: Write {
    /// Writes an unsigned 8-bit integer.
    fn write_u8(&mut self, x: u8) -> io::Result<()>;

    /// Writes a signed 16-bit integer in little endian format.
    fn write_le_i16(&mut self, x: i16) -> io::Result<()>;

    /// Writes an unsigned 16-bit integer in little endian format.
    fn write_le_u16(&mut self, x: u16) -> io::Result<()>;

    /// Writes a signed 24-bit integer in little endian format.
    ///
    /// The most significant byte of the `i32` is ignored.
    fn write_le_i24(&mut self, x: i32) -> io::Result<()>;

    /// Writes an unsigned 24-bit integer in little endian format.
    ///
    /// The most significant byte of the `u32` is ignored.
    fn write_le_u24(&mut self, x: u32) -> io::Result<()>;

    /// Writes a signed 32-bit integer in little endian format.
    fn write_le_i32(&mut self, x: i32) -> io::Result<()>;

    /// Writes an unsigned 32-bit integer in little endian format.
    fn write_le_u32(&mut self, x: u32) -> io::Result<()>;

    fn write_le_u64(&mut self, x: u64) -> io::Result<()>;

    /// Writes an IEEE float in little endian format.
    fn write_le_f32(&mut self, x: f32) -> io::Result<()>;

    fn write_le_f64(&mut self, x: f64) -> io::Result<()>;
}

impl<W> WriteBuffer for W
where
    W: Write,
{
    #[inline(always)]
    fn write_u8(&mut self, x: u8) -> io::Result<()> {
        let buf = [x];
        self.write_all(&buf)
    }

    #[inline(always)]
    fn write_le_i16(&mut self, x: i16) -> io::Result<()> {
        self.write_le_u16(x as u16)
    }

    #[inline(always)]
    fn write_le_u16(&mut self, x: u16) -> io::Result<()> {
        let mut buf = [0u8; 2];
        buf[0] = (x & 0xff) as u8;
        buf[1] = (x >> 8) as u8;
        self.write_all(&buf)
    }

    #[inline(always)]
    fn write_le_i24(&mut self, x: i32) -> io::Result<()> {
        self.write_le_u24(x as u32)
    }

    #[inline(always)]
    fn write_le_u24(&mut self, x: u32) -> io::Result<()> {
        let mut buf = [0u8; 3];
        buf[0] = (x & 0xff) as u8;
        buf[1] = ((x >> 8) & 0xff) as u8;
        buf[2] = ((x >> 16) & 0xff) as u8;
        self.write_all(&buf)
    }

    #[inline(always)]
    fn write_le_i32(&mut self, x: i32) -> io::Result<()> {
        self.write_le_u32(x as u32)
    }

    #[inline(always)]
    fn write_le_u32(&mut self, x: u32) -> io::Result<()> {
        let mut buf = [0u8; 4];
        buf[0] = (x & 0xff) as u8;
        buf[1] = ((x >> 8) & 0xff) as u8;
        buf[2] = ((x >> 16) & 0xff) as u8;
        buf[3] = ((x >> 24) & 0xff) as u8;
        self.write_all(&buf)
    }

    fn write_le_u64(&mut self, x: u64) -> io::Result<()> {
        let mut buf = [0u8; 8];
        buf[0] = (x & 0xff) as u8;
        buf[1] = ((x >> 8) & 0xff) as u8;
        buf[2] = ((x >> 16) & 0xff) as u8;
        buf[3] = ((x >> 24) & 0xff) as u8;
        buf[4] = ((x >> 32) & 0xff) as u8;
        buf[5] = ((x >> 40) & 0xff) as u8;
        buf[6] = ((x >> 48) & 0xff) as u8;
        buf[7] = ((x >> 56) & 0xff) as u8;
        self.write_all(&buf)
    }

    #[inline(always)]
    fn write_le_f32(&mut self, x: f32) -> io::Result<()> {
        let u = x.to_u32().ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "Failed to convert f32 to u32")
        })?;
        self.write_le_u32(u)
    }

    fn write_le_f64(&mut self, x: f64) -> io::Result<()> {
        let u = x.to_u64().ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "Failed to convert f64 to u64")
        })?;
        self.write_le_u64(u)
    }
}
