use crate::vlu16n::Vlu16N;
use crate::{Error, SerializeShrinkWrap};

/// no_std buffer writer that supports 1 bit, 4 bit, variable length integer and other operations.
/// No alignment requirements are imposed on the byte buffer provided.
/// Allocator is not required for no_std use. See BufWriterOwned for std use.
///
/// # Example
/// ```
/// let mut buf = [0u8; 256];
/// let mut wr = shrink_wrap::BufWriter::new(&mut buf);
/// wr.write_bool(true).unwrap();
/// wr.write_u8(0xaa).unwrap();
/// assert_eq!(wr.finish(), &[0x80, 0xaa]);
/// ```
pub struct BufWriter<'i> {
    buf: &'i mut [u8],
    // Next byte to write to
    byte_idx: usize,
    // Next bit to write to
    bit_idx: u8,
    // Buffer length from the front, shrinks when write_u16_rev() is used.
    len_bytes: usize,
}

impl<'i> BufWriter<'i> {
    pub fn new(buf: &'i mut [u8]) -> Self {
        let len_bytes = buf.len();
        Self {
            buf,
            len_bytes,
            byte_idx: 0,
            bit_idx: 7,
        }
    }

    pub fn write_bool(&mut self, val: bool) -> Result<(), Error> {
        if self.byte_idx >= self.len_bytes {
            return Err(Error::OutOfBounds);
        }
        self.buf[self.byte_idx] &= !(1 << self.bit_idx);
        self.buf[self.byte_idx] |= (val as u8) << self.bit_idx;
        if self.bit_idx == 0 {
            self.bit_idx = 7;
            self.byte_idx += 1;
        } else {
            self.bit_idx -= 1;
        }
        Ok(())
    }

    pub fn write_u4(&mut self, val: u8) -> Result<(), Error> {
        self.align_nibble();
        if self.byte_idx >= self.len_bytes {
            return Err(Error::OutOfBounds);
        }
        if self.bit_idx == 7 {
            self.buf[self.byte_idx] &= 0b0000_1111;
            self.buf[self.byte_idx] |= val << 4;
            self.bit_idx = 3;
        } else {
            self.buf[self.byte_idx] &= 0b1111_0000;
            self.buf[self.byte_idx] |= val & 0b0000_1111;
            self.bit_idx = 7;
            self.byte_idx += 1;
        }
        Ok(())
    }

    pub fn write_u8(&mut self, val: u8) -> Result<(), Error> {
        self.align_byte();
        if self.byte_idx >= self.len_bytes {
            return Err(Error::OutOfBounds);
        }
        self.buf[self.byte_idx] = val;
        self.byte_idx += 1;
        Ok(())
    }

    pub fn write_vlu16n(&mut self, val: u16) -> Result<(), Error> {
        Vlu16N(val).write_forward(self)
    }

    pub fn write_f32(&mut self, val: f32) -> Result<(), Error> {
        for b in val.to_bits().to_be_bytes() {
            self.write_u8(b)?;
        }
        Ok(())
    }

    pub fn write_u16_rev(&mut self, val: u16) -> Result<(), Error> {
        if self.bytes_left() < 2 {
            return Err(Error::OutOfBoundsRev);
        }
        let val_be = val.to_be_bytes();
        self.buf[self.len_bytes - 2] = val_be[0];
        self.buf[self.len_bytes - 1] = val_be[1];
        self.len_bytes -= 2;
        Ok(())
    }

    pub fn write<T: SerializeShrinkWrap>(&mut self, val: &T) -> Result<(), Error> {
        val.ser_shrink_wrap(self)
    }

    pub fn finish(mut self) -> Result<&'i [u8], Error> {
        let reverse_u16_written = (self.buf.len() - self.len_bytes) / 2;
        if reverse_u16_written != 0 {
            let mut total_nibbles = 0;
            let mut idx = self.len_bytes;
            for _ in 0..reverse_u16_written {
                let val = u16::from_be_bytes([self.buf[idx], self.buf[idx + 1]]);
                total_nibbles += Vlu16N(val).len_nibbles();
                idx += 2;
            }
            self.align_nibble();
            let not_at_byte_boundary = self.bit_idx != 7;
            if not_at_byte_boundary {
                total_nibbles += 1;
            }
            if total_nibbles % 2 != 0 {
                // ensure that reading from the back always starts from a valid Vlu16N
                self.write_u4(0).map_err(|_| Error::OutOfBoundsRevCompact)?;
            }

            let mut idx = self.len_bytes;
            for _ in 0..reverse_u16_written {
                let val = u16::from_be_bytes([self.buf[idx], self.buf[idx + 1]]);
                Vlu16N(val).write_reversed(&mut self)?;
                idx += 2;
            }
            debug_assert!(self.bit_idx == 7);
            Ok(&self.buf[0..self.byte_idx])
        } else {
            self.align_byte();
            Ok(&self.buf[0..self.byte_idx])
        }
    }

    fn align_nibble(&mut self) {
        if self.bit_idx == 7 || self.bit_idx == 3 {
            return;
        }
        if self.bit_idx > 3 {
            self.buf[self.byte_idx] &= !(0xFF >> (7 - self.bit_idx));
            self.bit_idx = 3;
        } else {
            self.bit_idx = 7;
            self.byte_idx += 1;
        }
    }

    fn align_byte(&mut self) {
        if self.bit_idx == 7 {
            return;
        }
        self.buf[self.byte_idx] &= !(0xFF >> (7 - self.bit_idx));
        self.bit_idx = 7;
        self.byte_idx += 1;
    }

    pub fn bytes_left(&mut self) -> usize {
        if self.byte_idx <= self.len_bytes {
            if self.bit_idx == 7 {
                self.len_bytes - self.byte_idx
            } else if self.byte_idx < self.len_bytes {
                self.len_bytes - self.byte_idx - 1
            } else {
                0
            }
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::BufWriter;

    #[test]
    fn finish_zeroes_reserved_bits() {
        let mut buf = [0xFF; 64];
        let mut wr = BufWriter::new(&mut buf);
        wr.write_bool(true).unwrap();
        wr.write_bool(false).unwrap();
        assert_eq!(wr.finish().unwrap(), &[0b1000_0000]);
    }

    #[test]
    fn write_u8_zeroes_reserved_bits() {
        let mut buf = [0xFF; 64];
        let mut wr = BufWriter::new(&mut buf);
        wr.write_bool(true).unwrap();
        wr.write_bool(false).unwrap();
        wr.write_u8(0xAA).unwrap();
        assert_eq!(wr.finish().unwrap(), &[0b1000_0000, 0xAA]);
    }

    #[test]
    fn align_nibble_zeroes_reserved_bits() {
        let mut buf = [0xFF; 64];
        let mut wr = BufWriter::new(&mut buf);
        wr.write_bool(true).unwrap();
        wr.write_bool(false).unwrap();
        wr.write_u4(0b1010).unwrap();
        assert_eq!(wr.finish().unwrap(), &[0b1000_1010]);
    }

    #[test]
    fn booleans() {
        let mut buf = [0xFF; 64];
        let mut wr = BufWriter::new(&mut buf);
        for b in [true, false, true, false, true, true, false, false] {
            wr.write_bool(b).unwrap();
        }
        assert_eq!(wr.bytes_left(), 63);
        assert_eq!(wr.finish().unwrap(), &[0b10101100]);
    }

    #[test]
    fn rev_u16_aligned() {
        let mut buf = [0; 8];
        let mut wr = BufWriter::new(&mut buf);
        wr.write_u8(0xAA).unwrap();
        wr.write_u8(0xCC).unwrap();
        wr.write_u16_rev(3).unwrap();
        wr.write_u16_rev(5).unwrap();
        assert_eq!(wr.bytes_left(), 2);
        assert_eq!(&wr.buf, &[0xAA, 0xCC, 0, 0, 0, 5, 0, 3]);
        assert_eq!(wr.finish().unwrap(), &[0xAA, 0xCC, 0b0101_0011]);
    }

    #[test]
    fn rev_u16_unaligned() {
        let mut buf = [0; 10];
        let mut wr = BufWriter::new(&mut buf);
        wr.write_u8(0xAA).unwrap();
        wr.write_u8(0xCC).unwrap();
        wr.write_u16_rev(3).unwrap();
        wr.write_u16_rev(5).unwrap();
        wr.write_u16_rev(7).unwrap();
        assert_eq!(wr.bytes_left(), 2);
        assert_eq!(&wr.buf, &[0xAA, 0xCC, 0, 0, 0, 7, 0, 5, 0, 3]);
        assert_eq!(
            wr.finish().unwrap(),
            &[0xAA, 0xCC, 0b0000_0111, 0b0101_0011]
        );
    }
}