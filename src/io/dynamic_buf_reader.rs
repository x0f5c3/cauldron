use std::cmp;
use std::io;

/// A buffer reader with dynamic cache size. Cache grows from 8kb to max 32kb.
pub struct DynamicBufReader<R> {
    /// The source reader.
    inner: R,

    /// The combined read-ahead/rewind buffer filled from the inner reader.
    buf: Box<[u8]>,

    /// The index of the next readable byte in buf.
    pos: usize,

    /// capacity of the buffer
    end_pos: usize,

    /// The capacity of the read-ahead buffer at this moment. Grows exponentially as more sequential
    /// reads are serviced.
    cur_capacity: usize,
}

#[allow(dead_code)]
impl<R: io::Read> DynamicBufReader<R> {
    /// The maximum capacity of the read-ahead buffer. Must be a power-of-2.
    const MAX_CAPACITY: usize = 32 * 1024;

    /// The initial capacity of the read-ahead buffer. Must be less than MAX_CAPACITY, and a
    /// power-of-2.
    const INIT_CAPACITY: usize = 8 * 1024;

    pub fn new(source: R) -> Self {
        DynamicBufReader {
            inner: source,
            cur_capacity: Self::INIT_CAPACITY,
            buf: vec![0u8; Self::MAX_CAPACITY].into_boxed_slice(),
            pos: 0,
            end_pos: 0,
        }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }

    #[inline]
    fn discard_buffer(&mut self) {
        self.pos = 0;
        self.end_pos = 0;
    }

    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        // Branch using `>=` instead of the more correct `==`
        // to tell the compiler that the pos..cap slice is always valid.
        if self.pos >= self.end_pos {
            self.end_pos = self.inner.read(&mut self.buf[0..self.cur_capacity])?;
            self.pos = 0;

            if self.cur_capacity < Self::MAX_CAPACITY {
                self.cur_capacity *= 2;
            }
        }
        Ok(&self.buf[self.pos..self.end_pos])
    }
}

impl<R: io::Read> io::Read for DynamicBufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        if self.pos == self.end_pos && buf.len() >= self.buf.len() {
            self.discard_buffer();
            return self.inner.read(buf);
        }
        let nread = {
            let mut rem = self.fill_buf()?;
            rem.read(buf)?
        };
        self.pos = cmp::min(self.pos + nread, self.end_pos);
        Ok(nread)
    }
}
