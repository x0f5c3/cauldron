use std::cmp;
use std::io;

/// Extends the functionality of `io::Read` with additional methods
pub trait ReadBuffer {
    /// Reads as many bytes as `buf` is long.
    ///
    /// This may issue multiple `read` calls internally. An error is returned
    /// if `read` read 0 bytes before the buffer is full.
    fn read_into(&mut self, buf: &mut [u8]) -> io::Result<()>;

    /// Reads `n` bytes and returns them in a vector.
    fn read_bytes(&mut self, n: usize) -> io::Result<Vec<u8>>;

    /// Skip over `n` bytes.
    fn skip_bytes(&mut self, n: usize) -> io::Result<()>;

    /// Reads a single byte and interprets it as an 8-bit unsigned integer.
    fn read_u8(&mut self) -> io::Result<u8>;

    /// Reads a single byte and interprets it as an 8-bit signed integer.
    #[inline(always)]
    fn read_i8(&mut self) -> io::Result<i8> {
        self.read_u8().map(|x| x as i8)
    }

    /// Reads two bytes and interprets them as a little-endian 16-bit unsigned integer.
    fn read_le_u16(&mut self) -> io::Result<u16>;

    /// Reads two bytes and interprets them as a little-endian 16-bit signed integer.
    #[inline(always)]
    fn read_le_i16(&mut self) -> io::Result<i16> {
        self.read_le_u16().map(|x| x as i16)
    }

    /// Reads two bytes and interprets them as a big-endian 16-bit unsigned integer.
    fn read_be_u16(&mut self) -> io::Result<u16>;

    /// Reads three bytes and interprets them as a little-endian 24-bit unsigned integer.
    ///
    /// The most significant byte will be 0.
    fn read_le_u24(&mut self) -> io::Result<u32>;

    /// Reads three bytes and interprets them as a little-endian 24-bit signed integer.
    ///
    /// The sign bit will be extended into the most significant byte.
    #[inline(always)]
    fn read_le_i24(&mut self) -> io::Result<i32> {
        self.read_le_u24().map(|x|
			// Test the sign bit, if it is set, extend the sign bit into the
			// most significant byte.
			if x & (1 << 23) == 0 {
				x as i32
			} else {
				(x | 0xff_00_00_00) as i32
			}
		)
    }

    /// Reads three bytes and interprets them as a big-endian 24-bit unsigned integer.
    ///
    /// Most significant byte will be 0.
    fn read_be_u24(&mut self) -> io::Result<u32>;

    /// Reads four bytes and interprets them as a little-endian 32-bit unsigned integer.
    fn read_le_u32(&mut self) -> io::Result<u32>;
    fn read_le_u64(&mut self) -> io::Result<u64>;

    /// Reads four bytes and interprets them as a little-endian 32-bit signed integer.
    #[inline(always)]
    fn read_le_i32(&mut self) -> io::Result<i32> {
        self.read_le_u32().map(|x| x as i32)
    }

    /// Reads four bytes and interprets them as a big-endian 32-bit unsigned integer.
    fn read_be_u32(&mut self) -> io::Result<u32>;

    /// Reads four bytes and interprets them as a little-endian 32-bit IEEE float.
    #[inline(always)]
    fn read_le_f32(&mut self) -> io::Result<f32> {
        self.read_le_u32().map(f32::from_bits)
    }
    fn read_le_f64(&mut self) -> io::Result<f64> {
        self.read_le_u64().map(f64::from_bits)
    }
}

impl<R: io::Read> ReadBuffer for R {
    #[inline(always)]
    fn read_into(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let mut n = 0;
        while n < buf.len() {
            let progress = self.read(&mut buf[n..])?;
            if progress > 0 {
                n += progress;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Failed to read enough bytes.",
                ));
            }
        }
        Ok(())
    }

    //noinspection RsExternalLinter
    #[inline(always)]
    fn read_bytes(&mut self, n: usize) -> io::Result<Vec<u8>> {
        // We allocate a runtime fixed size buffer, and we are going to read
        // into it, so zeroing or filling the buffer is a waste. This method
        // is safe, because the contents of the buffer are only exposed when
        // they have been overwritten completely by the read.
        let mut buf = Vec::with_capacity(n);
        unsafe {
            buf.set_len(n);
        }
        self.read_into(&mut buf[..])?;
        Ok(buf)
    }

    #[inline(always)]
    fn skip_bytes(&mut self, n: usize) -> io::Result<()> {
        // Read from the input in chunks of 1024 bytes at a time, and discard
        // the result. 1024 is a tradeoff between doing a lot of calls, and
        // using too much stack space. This method is not in a hot path, so it
        // can afford to do this.
        let mut n_read = 0;
        let mut buf = [0u8; 1024];
        while n_read < n {
            let end = cmp::min(n - n_read, 1024);
            let progress = self.read(&mut buf[0..end])?;
            if progress > 0 {
                n_read += progress;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Failed to read enough bytes.",
                ));
            }
        }
        Ok(())
    }

    #[inline(always)]
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buf = [0u8; 1];
        self.read_into(&mut buf)?;
        Ok(buf[0])
    }

    #[inline(always)]
    fn read_le_u16(&mut self) -> io::Result<u16> {
        let mut buf = [0u8; 2];
        self.read_into(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    #[inline(always)]
    fn read_be_u16(&mut self) -> io::Result<u16> {
        let mut buf = [0u8; 2];
        self.read_into(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    #[inline(always)]
    fn read_le_u24(&mut self) -> io::Result<u32> {
        let mut buf = [0u8; 3];
        self.read_into(&mut buf)?;
        Ok((buf[2] as u32) << 16 | (buf[1] as u32) << 8 | buf[0] as u32)
    }

    #[inline(always)]
    fn read_be_u24(&mut self) -> io::Result<u32> {
        let mut buf = [0u8; 3];
        self.read_into(&mut buf)?;
        Ok((buf[0] as u32) << 16 | (buf[1] as u32) << 8 | buf[2] as u32)
    }

    #[inline(always)]
    fn read_le_u32(&mut self) -> io::Result<u32> {
        let mut buf = [0u8; 4];
        self.read_into(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_le_u64(&mut self) -> io::Result<u64> {
        let mut buf = [0u8; 8];
        self.read_into(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    #[inline(always)]
    fn read_be_u32(&mut self) -> io::Result<u32> {
        let mut buf = [0u8; 4];
        self.read_into(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
}

/// Wraps a `BufferReader` to facilitate reading that is not byte-aligned.
pub struct BitStream<'r, R: ReadBuffer> {
    /// The source where bits are read from.
    reader: &'r mut R,
    /// Data read from the reader, but not yet fully consumed.
    data: u8,
    /// The number of bits of `data` that have not been consumed.
    bits_left: u32,
}

impl<'r, R: ReadBuffer> BitStream<'r, R> {
    /// creates a new bitstream reader
    pub fn new(reader: &mut R) -> BitStream<R> {
        BitStream {
            reader,
            data: 0,
            bits_left: 0,
        }
    }

    /// Returns true if no bits are left and input is in byte aligned state
    #[inline(always)]
    pub fn is_aligned(&self) -> bool {
        self.bits_left == 0
    }

    /// Reads a single bit
    #[inline(always)]
    pub fn read_bit(&mut self) -> io::Result<bool> {
        // If no bits are left, we will need to read the next byte.
        let result = if self.bits_left == 0 {
            let fresh_byte = self.reader.read_u8()?;

            // What remains later are the 7 least significant bits.
            self.data = fresh_byte << 1;
            self.bits_left = 7;

            // What we report is the most significant bit of the fresh byte.
            fresh_byte & 0b1000_0000
        } else {
            // Consume the most significant bit of the buffer byte.
            let bit = self.data & 0b1000_0000;
            self.data <<= 1;
            self.bits_left = self.bits_left - 1;
            bit
        };

        Ok(result != 0)
    }

    /// Reads at most 8 bits.
    #[inline(always)]
    pub fn read_len_u8(&mut self, bits: u32) -> io::Result<u8> {
        // If not enough bits left, we will need to read the next byte.
        let result = if self.bits_left < bits {
            // Most significant bits are shifted to the right position already.
            let msb = self.data;

            // Read a single byte.
            self.data = self.reader.read_u8()?;

            // From the next byte, we take the additional bits that we need.
            // Those start at the most significant bit, so we need to shift so
            // that it does not overlap with what we have already.
            let lsb =
                (self.data & BitStream::<R>::mask_u8(bits - self.bits_left)) >> self.bits_left;

            // Shift out the bits that we have consumed.
            self.data = BitStream::<R>::shift_left(self.data, bits - self.bits_left);
            self.bits_left = 8 - (bits - self.bits_left);

            msb | lsb
        } else {
            let result = self.data & BitStream::<R>::mask_u8(bits);

            // Shift out the bits that we have consumed.
            self.data = self.data << bits;
            self.bits_left = self.bits_left - bits;

            result
        };

        // The resulting data is padded with zeros in the least significant
        // bits, but we want to pad in the most significant bits, so shift.
        Ok(BitStream::<R>::shift_right(result, 8 - bits))
    }

    /// Reads at most 16 bits.
    #[inline(always)]
    pub fn read_len_u16(&mut self, bits: u32) -> io::Result<u16> {
        // Note: the following is not the most efficient implementation
        // possible, but it avoids duplicating the complexity of `read_len_u8`.

        if bits <= 8 {
            let result = self.read_len_u8(bits)?;
            Ok(result as u16)
        } else {
            // First read the 8 most significant bits, then read what is left.
            let msb = self.read_len_u8(8)? as u16;
            let lsb = self.read_len_u8(bits - 8)? as u16;
            Ok((msb << (bits - 8)) | lsb)
        }
    }

    /// Reads at most 32 bits.
    #[inline(always)]
    pub fn read_len_u32(&mut self, bits: u32) -> io::Result<u32> {
        // As with read_len_u8, this only makes sense if we read <= 32 bits.
        debug_assert!(bits <= 32);

        // Note: the following is not the most efficient implementation
        // possible, but it avoids duplicating the complexity of `read_len_u8`.

        if bits <= 16 {
            let result = self.read_len_u16(bits)?;
            Ok(result as u32)
        } else {
            // First read the 16 most significant bits, then read what is left.
            let msb = self.read_len_u16(16)? as u32;
            let lsb = self.read_len_u16(bits - 16)? as u32;
            Ok((msb << (bits - 16)) | lsb)
        }
    }

    /// Reads bits until a 1 is read, and returns the number of zeros read.
    /// See here https://en.wikipedia.org/wiki/Unary_coding
    #[inline(always)]
    pub fn read_unary(&mut self) -> io::Result<u32> {
        // Count the zeroes already present in the buffer
        // (counting from the most significant bit).
        let mut n = self.data.leading_zeros();

        // If the number of zeros plus the one following it was not more than
        // the bytes left, then there is no need to look further.
        if n < self.bits_left {
            // save the bits left in data
            self.data = self.data << (n + 1);
            self.bits_left = self.bits_left - (n + 1);
        } else {
            // counter the case when no bits are left and data = 0
            n = self.bits_left;

            // Continue reading bytes until we encounter a one.
            loop {
                let fresh_byte = self.reader.read_u8()?;
                let zeros = fresh_byte.leading_zeros();
                n = n + zeros;
                if zeros < 8 {
                    // We consumed the zeros, plus the one following it.
                    self.bits_left = 8 - (zeros + 1);
                    if zeros == 7 {
                        self.data = 0;
                    } else {
                        self.data = fresh_byte << (zeros + 1);
                    }
                    break;
                }
            }
        }

        Ok(n)
    }

    #[inline(always)]
    pub fn skip_len_u8(&mut self, bits: u32) -> io::Result<()> {
        // If not enough bits left, we will need to read the next byte.
        if self.bits_left < bits {
            // Read a single byte.
            self.data = self.reader.read_u8()?;

            // Shift out the bits that we have consumed.
            self.data = BitStream::<R>::shift_left(self.data, bits - self.bits_left);
            self.bits_left = 8 - (bits - self.bits_left);
        } else {
            // Shift out the bits that we have consumed.
            self.data = self.data << bits;
            self.bits_left = self.bits_left - bits;
        }

        Ok(())
    }

    // Generates a bitmask with 1s in the `bits` most significant bits.
    #[inline(always)]
    fn mask_u8(bits: u32) -> u8 {
        debug_assert!(bits <= 8);

        BitStream::<R>::shift_left(0xff, 8 - bits)
    }

    fn shift_left(x: u8, shift: u32) -> u8 {
        debug_assert!(shift <= 8);

        // We cannot shift a u8 by 8 or more, because Rust panics when shifting by
        // the integer width. But we can definitely shift a u32.
        ((x as u16) << shift) as u8
    }

    /// Right shift that does not panic when shifting by the integer width.
    #[inline(always)]
    fn shift_right(x: u8, shift: u32) -> u8 {
        debug_assert!(shift <= 8);

        // We cannot shift a u8 by 8 or more, because Rust panics when shifting by
        // the integer width. But we can definitely shift a u32.
        ((x as u32) >> shift) as u8
    }
}
