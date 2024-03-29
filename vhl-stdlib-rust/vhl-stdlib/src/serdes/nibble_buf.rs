use crate::discrete::U4;
use crate::serdes::{
    bit_buf,
    vlu4::{Vlu32N, Vlu4VecBuilder},
    BitBuf, BitBufMut, DeserializeVlu4, SerDesSize, SerializeVlu4,
};
use core::fmt::{Debug, Display, Formatter};
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::ptr::copy_nonoverlapping;

/// Buffer reader that treats input as a stream of nibbles.
///
/// Use `nrd` as short name: let mut nrd = NibbleBuf::new(..);
#[derive(Copy, Clone)]
pub struct NibbleBuf<'i> {
    pub(crate) buf: &'i [u8],
    // Maximum number of nibbles to read (not whole buf might be available)
    pub(crate) len_nibbles: usize,
    // Next byte to read
    pub(crate) idx: usize,
    // Next nibble to read
    pub(crate) is_at_byte_boundary: bool,
}

#[cfg(not(feature = "no_std"))]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct NibbleBufOwned {
    pub buf: Vec<u8>,
    pub len_nibbles: usize,
    pub is_at_byte_boundary: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    // #[error("Out of bounds access")]
    OutOfBounds,
    // #[error("Wrong vlu4 number")]
    MalformedVlu32N,
    // #[error("Unaligned access for slice")]
    UnalignedAccess,

    #[cfg(feature = "buf-strict")]
    InvalidSizedEstimate,
    #[cfg(feature = "buf-strict")]
    InvalidSizedAlignedEstimate,
    #[cfg(feature = "buf-strict")]
    InvalidUnsizedBoundEstimate,

    Vlu4Vec,
    InvalidErrorCode,
    BitBuf,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(not(feature = "no_std"))]
impl std::error::Error for Error {}

impl From<bit_buf::Error> for Error {
    fn from(_: bit_buf::Error) -> Self {
        Error::BitBuf
    }
}

impl<'i> NibbleBuf<'i> {
    pub fn new(buf: &'i [u8], len_nibbles: usize) -> Result<Self, Error> {
        if len_nibbles > buf.len() * 2 {
            Err(Error::OutOfBounds)
        } else {
            Ok(NibbleBuf {
                buf,
                len_nibbles,
                idx: 0,
                is_at_byte_boundary: true,
            })
        }
    }

    pub fn new_all(buf: &'i [u8]) -> Self {
        NibbleBuf {
            buf,
            len_nibbles: buf.len() * 2,
            idx: 0,
            is_at_byte_boundary: true,
        }
    }

    pub fn get_bit_buf(&mut self, nibble_count: usize) -> Result<BitBuf<'i>, Error> {
        if self.nibbles_left() < nibble_count {
            return Err(Error::OutOfBounds);
        }
        let buf_before_consuming = &self.buf[self.idx..];
        let offset = if self.is_at_byte_boundary { 0 } else { 4 };
        self.skip(nibble_count)?;

        BitBuf::new_with_offset(buf_before_consuming, offset, nibble_count * 4)
            .map_err(|_| Error::OutOfBounds)
    }

    // pub fn new_with_offset(buf: &'i [u8], offset_nibbles: usize) -> Result<Self, Error> {
    //     if offset_nibbles > buf.len() * 2 {
    //         Err(Error::OutOfBounds)
    //     } else {
    //         Ok(
    //             NibbleBuf {
    //                 buf,
    //                 idx: offset_nibbles / 2,
    //                 is_at_byte_boundary: offset_nibbles % 2 == 0,
    //             }
    //         )
    //     }
    // }

    pub fn nibbles_pos(&self) -> usize {
        if self.is_at_byte_boundary {
            self.idx * 2
        } else {
            self.idx * 2 + 1
        }
    }

    pub fn nibbles_left(&self) -> usize {
        if !self.is_at_end() {
            self.len_nibbles - self.nibbles_pos()
        } else {
            0
        }
    }

    pub fn is_at_end(&self) -> bool {
        self.nibbles_pos() >= self.len_nibbles
    }

    pub fn is_at_byte_boundary(&self) -> bool {
        self.is_at_byte_boundary
    }

    /// Limit size of data for reading at the current position of another instance of self
    pub fn shrink_to_pos_of(&mut self, advanced_self: &NibbleBuf) -> Result<(), Error> {
        if self.buf != advanced_self.buf {
            return Err(Error::OutOfBounds);
        }
        self.len_nibbles = advanced_self.nibbles_pos();
        Ok(())
    }

    /// Get one nibble or return an OutOfBounds error otherwise.
    pub fn get_nibble(&mut self) -> Result<u8, Error> {
        if self.is_at_end() {
            return Err(Error::OutOfBounds);
        }
        Ok(unsafe { self.get_nibble_unchecked() })
    }

    unsafe fn get_nibble_unchecked(&mut self) -> u8 {
        if self.is_at_byte_boundary {
            let val = *self.buf.get_unchecked(self.idx);
            self.is_at_byte_boundary = false;
            (val & 0xf0) >> 4
        } else {
            let val = self.buf.get_unchecked(self.idx);
            self.is_at_byte_boundary = true;
            self.idx += 1;
            val & 0xf
        }
    }

    pub fn get_vlu32n(&mut self) -> Result<u32, Error> {
        let val: Vlu32N = Vlu32N::des_vlu4(self)?;
        Ok(val.0)
    }

    pub fn skip_vlu32n(&mut self) -> Result<(), Error> {
        while self.get_nibble()? & 0b1000 != 0 {}
        Ok(())
    }

    pub fn skip(&mut self, nibble_count: usize) -> Result<(), Error> {
        if self.nibbles_left() < nibble_count {
            return Err(Error::OutOfBounds);
        }
        unsafe {
            self.skip_unchecked(nibble_count);
        }
        Ok(())
    }

    unsafe fn skip_unchecked(&mut self, nibble_count: usize) {
        if self.is_at_byte_boundary {
            if nibble_count % 2 != 0 {
                self.is_at_byte_boundary = false;
            }
            self.idx += nibble_count / 2;
        } else if nibble_count % 2 != 0 {
            self.is_at_byte_boundary = true;
            self.idx += nibble_count / 2 + 1;
        } else {
            self.idx += nibble_count / 2;
        }
    }

    pub fn align_to_byte(&mut self) -> Result<(), Error> {
        if !self.is_at_byte_boundary {
            let _padding = self.get_nibble()?;
        }
        Ok(())
    }

    pub fn get_u8(&mut self) -> Result<u8, Error> {
        if self.nibbles_left() < 2 {
            return Err(Error::OutOfBounds);
        }
        if self.is_at_byte_boundary {
            let val = unsafe { *self.buf.get_unchecked(self.idx) };
            self.idx += 1;
            Ok(val)
        } else {
            let msn = unsafe { *self.buf.get_unchecked(self.idx) };
            self.idx += 1;
            let lsn = unsafe { *self.buf.get_unchecked(self.idx) };
            Ok((msn << 4) | (lsn >> 4))
        }
    }

    pub fn get_u16_be(&mut self) -> Result<u16, Error> {
        Ok(((self.get_u8()? as u16) << 8) | self.get_u8()? as u16)
    }

    pub fn get_u32_be(&mut self) -> Result<u32, Error> {
        Ok(((self.get_u8()? as u32) << 24)
            | ((self.get_u8()? as u32) << 16)
            | ((self.get_u8()? as u32) << 8)
            | self.get_u8()? as u32)
    }

    pub fn get_slice(&mut self, len: usize) -> Result<&'i [u8], Error> {
        if !self.is_at_byte_boundary {
            return Err(Error::UnalignedAccess);
        }
        if self.nibbles_left() < len * 2 {
            return Err(Error::OutOfBounds);
        }
        let slice = &self.buf[self.idx..self.idx + len];
        self.idx += len;
        Ok(slice)
    }

    pub fn get_buf_slice(&mut self, len_nibbles: usize) -> Result<Self, Error> {
        if self.nibbles_left() < len_nibbles {
            return Err(Error::OutOfBounds);
        }

        let idx_before = self.idx;
        let is_at_byte_boundary_before = self.is_at_byte_boundary;
        unsafe {
            self.skip_unchecked(len_nibbles);
        }

        let len_nibbles = if is_at_byte_boundary_before {
            len_nibbles
        } else {
            len_nibbles + 1
        };
        Ok(NibbleBuf {
            buf: &self.buf[idx_before..],
            len_nibbles,
            idx: 0,
            is_at_byte_boundary: is_at_byte_boundary_before,
        })
    }

    pub fn des_vlu4<'di, T: DeserializeVlu4<'i>>(&'di mut self) -> Result<T, T::Error> {
        T::des_vlu4(self)
    }

    /// Read result code as vlu4, if 0 => deserialize T and return Ok(Ok(T)),
    /// otherwise return Ok(Err(f(code))).
    /// If deserialization of T fails, return Err(T::Error)
    pub fn des_vlu4_if_ok<'di, T, F, E>(&'di mut self, f: F) -> Result<Result<T, E>, T::Error>
    where
        T: DeserializeVlu4<'i>, // type to deserialize
        F: Fn(u32) -> E,        // result error mapping closure
        T::Error: From<Error>,  // deserialization error of T
    {
        let code = self.get_vlu32n()?;
        if code == 0 {
            T::des_vlu4(self).map(|t| Ok(t))
        } else {
            Ok(Err(f(code)))
        }
    }

    pub fn iter(&self) -> NibbleBufIter {
        NibbleBufIter { buf: *self }
    }

    #[cfg(not(feature = "no_std"))]
    pub fn to_nibble_buf_owned(&self) -> NibbleBufOwned {
        let len_nibbles = if self.is_at_byte_boundary {
            self.nibbles_left()
        } else {
            self.nibbles_left() + 1
        };
        NibbleBufOwned {
            buf: self.buf[self.idx..].to_vec(),
            len_nibbles,
            is_at_byte_boundary: self.is_at_byte_boundary,
        }
    }
}

#[cfg(not(feature = "no_std"))]
impl NibbleBufOwned {
    pub fn new() -> NibbleBufOwned {
        NibbleBufOwned {
            buf: vec![],
            len_nibbles: 0,
            is_at_byte_boundary: true,
        }
    }

    pub fn from_vec(buf: Vec<u8>) -> Self {
        let len_nibbles = buf.len() * 2;
        NibbleBufOwned {
            buf,
            len_nibbles,
            is_at_byte_boundary: true,
        }
    }

    pub fn to_nibble_buf_ref(&self) -> NibbleBuf {
        NibbleBuf {
            buf: &self.buf,
            len_nibbles: self.len_nibbles,
            idx: 0,
            is_at_byte_boundary: self.is_at_byte_boundary,
        }
    }

    pub fn inner(self) -> Vec<u8> {
        self.buf
    }
}

impl Default for NibbleBufOwned {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "no_std"))]
impl Debug for NibbleBufOwned {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.to_nibble_buf_ref(), f)
    }
}

pub struct NibbleBufIter<'i> {
    buf: NibbleBuf<'i>,
}

impl<'i> Iterator for NibbleBufIter<'i> {
    type Item = U4;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buf.is_at_end() {
            None
        } else {
            Some(unsafe { U4::new_unchecked(self.buf.get_nibble_unchecked()) })
        }
    }
}

impl<'i> FusedIterator for NibbleBufIter<'i> {}

impl<'i> PartialEq for NibbleBuf<'i> {
    fn eq(&self, other: &Self) -> bool {
        if self.nibbles_left() != other.nibbles_left() {
            return false;
        }
        // can be optimized to compare bytes vs bytes, but now comparison is only used in unit tests
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}

impl<'i> Eq for NibbleBuf<'i> {}

impl<'i> DeserializeVlu4<'i> for NibbleBuf<'i> {
    type Error = Error;

    fn des_vlu4<'di>(nrd: &'di mut NibbleBuf<'i>) -> Result<Self, Self::Error> {
        let len = nrd.get_vlu32n()?;
        nrd.get_buf_slice(len as usize)
    }
}

impl<'i> SerializeVlu4 for NibbleBuf<'i> {
    type Error = Error;

    fn ser_vlu4(&self, nwr: &mut NibbleBufMut) -> Result<(), Self::Error> {
        nwr.put(&Vlu32N(self.nibbles_left() as u32))?;
        nwr.put_nibble_buf(self)
    }

    fn len_nibbles(&self) -> SerDesSize {
        let len_len = Vlu32N(self.nibbles_left() as u32).len_nibbles_known_to_be_sized();
        SerDesSize::Sized(len_len + self.nibbles_left())
    }
}

#[cfg(not(feature = "no_std"))]
impl SerializeVlu4 for NibbleBufOwned {
    type Error = Error;

    fn ser_vlu4(&self, nwr: &mut NibbleBufMut) -> Result<(), Self::Error> {
        self.to_nibble_buf_ref().ser_vlu4(nwr)
    }

    fn len_nibbles(&self) -> SerDesSize {
        self.to_nibble_buf_ref().len_nibbles()
    }
}

impl<'i> Display for NibbleBuf<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "NibbleBuf(")?;
        let mut buf = *self;
        if buf.nibbles_pos() > 0 {
            write!(f, "<{}< ", buf.nibbles_pos())?;
        }
        while !buf.is_at_end() {
            write!(f, "{:01x}", buf.get_nibble().unwrap_or(0))?;
            if buf.nibbles_left() >= 1 {
                write!(f, " ")?;
            }
        }
        write!(f, ")")
    }
}

impl<'i> Debug for NibbleBuf<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

/// Buffer writer that supports 4 bit (nibble) level operations
///
/// Use `nwr` as short name: let mut nwr = NibbleBufMut::new(..);
pub struct NibbleBufMut<'i> {
    pub(crate) buf: &'i mut [u8],
    // Maximum number of nibbles available (not whole slice might be available)
    pub(crate) len_nibbles: usize,
    // Next byte to write to
    pub(crate) idx: usize,
    // Next nibble to write to
    pub(crate) is_at_byte_boundary: bool,
}

impl<'i> NibbleBufMut<'i> {
    /// Create a new nibble writer covering len_nibbles from the provided array only.
    /// If less than len_nibbles is actually written, remaining portion of the buf will contain original data.
    pub fn new(buf: &'i mut [u8], len_nibbles: usize) -> Result<Self, Error> {
        if len_nibbles > buf.len() * 2 {
            Err(Error::OutOfBounds)
        } else {
            Ok(NibbleBufMut {
                buf,
                len_nibbles,
                idx: 0,
                is_at_byte_boundary: true,
            })
        }
    }

    /// Create a new nibble writer covering whole provided array.
    /// If less than buf.len() * 2 nibbles is actually written, remaining portion of the buf
    /// will contain original data.
    pub fn new_all(buf: &'i mut [u8]) -> Self {
        let len_nibbles = buf.len() * 2;
        NibbleBufMut {
            buf,
            len_nibbles,
            idx: 0,
            is_at_byte_boundary: true,
        }
    }

    /// Convert self to BitBufMut from the current position to continue writing in bits
    pub fn to_bit_buf(self) -> BitBufMut<'i> {
        let bit_idx = if self.is_at_byte_boundary { 0 } else { 4 };
        BitBufMut {
            buf: self.buf,
            len_bits: self.len_nibbles * 4,
            idx: self.idx,
            bit_idx,
        }
    }

    pub fn to_nibble_buf(self) -> NibbleBuf<'i> {
        NibbleBuf {
            buf: self.buf,
            len_nibbles: self.idx,
            idx: 0,
            is_at_byte_boundary: true,
        }
    }

    /// Convert to BitBufMut and call f closure with it.
    /// Closure must leave BitBufMut at 4 bit boundary, otherwise UnalignedAccess error is returned.
    /// If closure fails, it's error is returned. Since there are 2 kind of errors being used,
    /// user error must implement From<bit_buf::Error>, see example.
    ///
    /// # Example
    /// ```
    /// use vhl_stdlib::serdes::NibbleBufMut;
    /// use vhl_stdlib::serdes::bit_buf::Error as BitBufError;
    ///
    /// #[derive(Debug)]
    /// enum MyError {
    ///     BitBufError(BitBufError),
    /// }
    /// impl From<BitBufError> for MyError {
    /// fn from(e: BitBufError) -> Self {
    ///         MyError::BitBufError(e)
    ///     }
    /// }
    ///
    /// let mut buf = [0u8; 1];
    /// let mut wgr = NibbleBufMut::new_all(&mut buf);
    /// wgr.put_nibble(0b1010).unwrap();
    ///
    /// wgr.as_bit_buf::<_, MyError>(|bit_wgr| {
    ///     bit_wgr.put_bit(true)?;
    ///     bit_wgr.put_bit(false)?;
    ///     bit_wgr.put_up_to_8(2, 0b11)?;
    ///     Ok(())
    /// }).unwrap();
    ///
    /// let (buf, _, _) = wgr.finish();
    /// assert_eq!(buf[0], 0b1010_1011);
    /// ```
    pub fn as_bit_buf<F, E>(&mut self, mut f: F) -> Result<(), E>
    where
        F: FnMut(&mut BitBufMut) -> Result<(), E>,
        E: From<bit_buf::Error>,
    {
        let bit_idx = if self.is_at_byte_boundary { 0 } else { 4 };
        let mut bit_buf = BitBufMut {
            buf: self.buf,
            len_bits: self.len_nibbles * 4,
            idx: self.idx,
            bit_idx,
        };
        f(&mut bit_buf)?;
        if bit_buf.bit_idx != 0 && bit_buf.bit_idx != 4 {
            return Err(E::from(bit_buf::Error::UnalignedAccess));
        }
        self.idx = bit_buf.idx;
        self.is_at_byte_boundary = bit_buf.bit_idx == 0;
        Ok(())
    }

    pub fn nibbles_pos(&self) -> usize {
        if self.is_at_byte_boundary {
            self.idx * 2
        } else {
            self.idx * 2 + 1
        }
    }

    pub fn nibbles_left(&self) -> usize {
        if !self.is_at_end() {
            self.len_nibbles - self.nibbles_pos()
        } else {
            0
        }
    }

    pub fn is_at_end(&self) -> bool {
        self.nibbles_pos() >= self.len_nibbles
    }

    pub fn is_at_byte_boundary(&self) -> bool {
        self.is_at_byte_boundary
    }

    pub fn finish(self) -> (&'i mut [u8], usize, bool) {
        (self.buf, self.idx, self.is_at_byte_boundary)
    }

    pub fn skip(&mut self, nibble_count: usize) -> Result<(), Error> {
        if self.nibbles_left() < nibble_count {
            return Err(Error::OutOfBounds);
        }
        if self.is_at_byte_boundary {
            if nibble_count % 2 != 0 {
                self.is_at_byte_boundary = false;
            }
            self.idx += nibble_count / 2;
        } else if nibble_count % 2 != 0 {
            self.is_at_byte_boundary = true;
            self.idx += nibble_count / 2 + 1;
        } else {
            self.idx += nibble_count / 2;
        }
        Ok(())
    }

    pub fn rewind<F, E>(&mut self, to_nibbles_pos: usize, f: F) -> Result<(), E>
    where
        F: Fn(&mut Self) -> Result<(), E>,
        E: From<Error>,
    {
        if to_nibbles_pos >= self.len_nibbles {
            return Err(Error::OutOfBounds.into());
        }
        let idx_before = self.idx;
        let is_at_byte_boundary_before = self.is_at_byte_boundary;
        self.idx = to_nibbles_pos / 2;
        self.is_at_byte_boundary = to_nibbles_pos % 2 == 0;
        f(self)?;
        self.idx = idx_before;
        self.is_at_byte_boundary = is_at_byte_boundary_before;
        Ok(())
    }

    pub fn save_state(&self) -> NibbleBufMutState {
        NibbleBufMutState {
            buf_ptr: self.buf.as_ptr(),
            idx: self.idx,
            is_at_byte_boundary: self.is_at_byte_boundary,
        }
    }

    pub fn restore_state(&mut self, state: NibbleBufMutState) -> Result<(), Error> {
        // do not restore state from another buffer (still possible, but harder to trick this check)
        if self.buf.as_ptr() != state.buf_ptr {
            return Err(Error::OutOfBounds);
        }
        self.idx = state.idx;
        self.is_at_byte_boundary = state.is_at_byte_boundary;
        Ok(())
    }

    pub fn put_nibble(&mut self, nib: u8) -> Result<(), Error> {
        if self.nibbles_left() == 0 {
            return Err(Error::OutOfBounds);
        }
        unsafe {
            self.put_nibble_unchecked(nib);
        }
        Ok(())
    }

    unsafe fn put_nibble_unchecked(&mut self, nib: u8) {
        if self.is_at_byte_boundary {
            let b = self.buf.get_unchecked_mut(self.idx);
            *b &= 0b0000_1111;
            *b |= nib << 4;
            self.is_at_byte_boundary = false;
        } else {
            let b = self.buf.get_unchecked_mut(self.idx);
            *b &= 0b1111_0000;
            *b |= nib & 0b0000_1111;
            self.is_at_byte_boundary = true;
            self.idx += 1;
        }
    }

    pub fn put_vlu32n(&mut self, val: u32) -> Result<(), Error> {
        Vlu32N(val).ser_vlu4(self)
    }

    pub fn align_to_byte(&mut self) -> Result<(), Error> {
        if !self.is_at_byte_boundary {
            self.put_nibble(0)?;
        }
        Ok(())
    }

    pub fn put_u8(&mut self, val: u8) -> Result<(), Error> {
        if self.nibbles_left() < 2 {
            return Err(Error::OutOfBounds);
        }
        if self.is_at_byte_boundary {
            unsafe {
                *self.buf.get_unchecked_mut(self.idx) = val;
            }
            self.idx += 1;
        } else {
            unsafe {
                *self.buf.get_unchecked_mut(self.idx) |= val >> 4;
            }
            self.idx += 1;
            unsafe {
                *self.buf.get_unchecked_mut(self.idx) = val << 4;
            }
        }
        Ok(())
    }

    pub fn put_u16_be(&mut self, val: u16) -> Result<(), Error> {
        self.put_u8((val >> 8) as u8)?;
        self.put_u8((val & 0xff) as u8)
    }

    pub fn put_u32_be(&mut self, val: u32) -> Result<(), Error> {
        self.put_u8((val >> 24) as u8)?;
        self.put_u8((val >> 16) as u8)?;
        self.put_u8((val >> 8) as u8)?;
        self.put_u8((val & 0xff) as u8)
    }

    pub fn put_slice(&mut self, slice: &[u8]) -> Result<(), Error> {
        if self.nibbles_left() < slice.len() * 2 {
            return Err(Error::OutOfBounds);
        }
        if !self.is_at_byte_boundary() {
            return Err(Error::UnalignedAccess);
        }
        self.buf[self.idx..self.idx + slice.len()].copy_from_slice(slice);
        self.idx += slice.len();
        Ok(())
    }

    pub fn replace_nibble(&mut self, nibble_pos: usize, nib: u8) -> Result<(), Error> {
        if nibble_pos >= self.len_nibbles {
            return Err(Error::OutOfBounds);
        }
        let is_at_byte_boundary = nibble_pos % 2 == 0;
        let idx = nibble_pos / 2;
        if is_at_byte_boundary {
            unsafe {
                *self.buf.get_unchecked_mut(idx) &= 0x0f;
                *self.buf.get_unchecked_mut(idx) |= nib << 4;
            }
        } else {
            unsafe {
                *self.buf.get_unchecked_mut(idx) &= 0xf0;
                *self.buf.get_unchecked_mut(idx) |= nib & 0xf;
            }
        }
        Ok(())
    }

    /// Create a Vlu4VecBuilder from the rest of this buffer.
    /// Consumes self by value, to ensure that Vlu4Vec is properly finalized.
    /// NibbleBufMut can be obtained back by calling finish().
    ///
    /// Example:
    /// ```
    /// use vhl_stdlib::serdes::NibbleBufMut;
    /// use vhl_stdlib::serdes::vlu4::Vlu4VecBuilder;
    /// use hex_literal::hex;
    ///
    /// let mut buf = [0u8; 128];
    /// let wgr = NibbleBufMut::new_all(&mut buf);
    /// let mut wgr = wgr.put_vec::<&[u8]>();
    ///
    /// wgr.put_aligned(&[1, 2, 3]).unwrap();
    /// wgr.put_aligned(&[4, 5]).unwrap();
    /// let wgr: NibbleBufMut = wgr.finish().unwrap();
    /// let (buf, _, _) = wgr.finish();
    /// assert_eq!(&buf[0..=6], hex!("23 01 02 03 20 04 05"));
    /// ```
    pub fn put_vec<T>(self) -> Vlu4VecBuilder<'i, T> {
        let idx_before = self.idx;
        let is_at_byte_boundary_before = self.is_at_byte_boundary;
        Vlu4VecBuilder {
            nwr: self,
            idx_before,
            is_at_byte_boundary_before,
            stride_len: 0,
            stride_len_idx_nibbles: 0,
            slices_written: 0,
            _phantom: PhantomData,
        }
    }

    /// Put Vlu4Vec into this buffer taking elements from the provided closure, while it returns Some.
    /// Takes self by mutable reference instead of by value as [put_vec](Self::put_vec);
    pub fn unfold_as_vec<T, F, SE>(&mut self, mut f: F) -> Result<(), SE>
    where
        F: FnMut() -> Option<T>,
        T: SerializeVlu4<Error = SE>,
        SE: From<Error>,
    {
        let mut builder = Vlu4VecBuilder {
            nwr: NibbleBufMut {
                buf: self.buf,
                len_nibbles: self.len_nibbles,
                idx: self.idx,
                is_at_byte_boundary: self.is_at_byte_boundary,
            },
            idx_before: self.idx,
            is_at_byte_boundary_before: self.is_at_byte_boundary,
            stride_len: 0,
            stride_len_idx_nibbles: 0,
            slices_written: 0,
            _phantom: PhantomData,
        };
        while let Some(t) = f() {
            builder.put(&t)?;
        }
        builder.finish_internal()?;
        self.idx = builder.nwr.idx;
        self.is_at_byte_boundary = builder.nwr.is_at_byte_boundary;
        Ok(())
    }

    /// Create Vlu4VecBuilder and call provided closure with it without consuming self.
    pub fn put_vec_with<T, F, SE>(&mut self, f: F) -> Result<(), SE>
    where
        F: FnOnce(&mut Vlu4VecBuilder<T>) -> Result<(), SE>,
        T: SerializeVlu4<Error = SE>,
        SE: From<Error>,
    {
        let mut builder = Vlu4VecBuilder {
            nwr: NibbleBufMut {
                buf: self.buf,
                len_nibbles: self.len_nibbles,
                idx: self.idx,
                is_at_byte_boundary: self.is_at_byte_boundary,
            },
            idx_before: self.idx,
            is_at_byte_boundary_before: self.is_at_byte_boundary,
            stride_len: 0,
            stride_len_idx_nibbles: 0,
            slices_written: 0,
            _phantom: PhantomData,
        };
        f(&mut builder)?;
        builder.finish_internal()?;
        self.idx = builder.nwr.idx;
        self.is_at_byte_boundary = builder.nwr.is_at_byte_boundary;
        Ok(())
    }

    pub fn put_nibble_buf(&mut self, other: &NibbleBuf) -> Result<(), Error> {
        if self.nibbles_left() < other.nibbles_left() {
            return Err(Error::OutOfBounds);
        }
        if self.is_at_byte_boundary && other.is_at_byte_boundary {
            let bytes_to_copy = if other.nibbles_left() % 2 == 0 {
                other.nibbles_left() / 2
            } else {
                self.is_at_byte_boundary = false;
                other.nibbles_left() / 2 + 1
            };
            unsafe {
                copy_nonoverlapping(
                    other.buf.as_ptr().add(other.idx),
                    self.buf.as_mut_ptr().add(self.idx),
                    bytes_to_copy,
                );
            }
            self.idx += bytes_to_copy;
        } else if !self.is_at_byte_boundary && !other.is_at_byte_boundary {
            unsafe {
                self.put_nibble_unchecked(*other.buf.get_unchecked(other.idx) & 0x0f);
            }
            let other_nibbles_left = other.nibbles_left() - 1;
            let bytes_to_copy = if other_nibbles_left % 2 == 0 {
                other_nibbles_left / 2
            } else {
                self.is_at_byte_boundary = false;
                other_nibbles_left / 2 + 1
            };
            unsafe {
                copy_nonoverlapping(
                    other.buf.as_ptr().add(other.idx + 1),
                    self.buf.as_mut_ptr().add(self.idx),
                    bytes_to_copy,
                );
            }
            self.idx += bytes_to_copy;
        } else {
            let mut other_clone = *other;
            while !other_clone.is_at_end() {
                unsafe { self.put_nibble_unchecked(other_clone.get_nibble_unchecked()) }
            }
        }

        Ok(())
    }

    /// Put any type that implements SerializeVlu4 into this buffer.
    pub fn put<E, T: SerializeVlu4<Error = E>>(&mut self, t: &T) -> Result<(), E> {
        t.ser_vlu4(self)
    }

    #[cfg(not(feature = "no_std"))]
    pub fn to_nibble_buf_owned(&self) -> NibbleBufOwned {
        NibbleBufOwned {
            buf: self.buf[..self.idx].to_vec(),
            len_nibbles: self.nibbles_pos(),
            is_at_byte_boundary: true,
        }
    }
}

impl<'i> Display for NibbleBufMut<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "NibbleBufMut(")?;
        let mut nrd = NibbleBuf {
            buf: self.buf,
            len_nibbles: self.nibbles_pos(),
            idx: 0,
            is_at_byte_boundary: true,
        };
        let mut i = 0;
        while !nrd.is_at_end() {
            write!(f, "{:01x}", nrd.get_nibble().unwrap_or(0))?;
            if i == 7 {
                write!(f, " ")?;
                i = 0;
            } else {
                i += 1;
            }
        }
        if self.nibbles_left() > 0 {
            write!(f, ">{}>", self.nibbles_left())?;
        }
        write!(f, ")")
    }
}

impl<'i> Debug for NibbleBufMut<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

pub struct NibbleBufMutState {
    buf_ptr: *const u8,
    idx: usize,
    is_at_byte_boundary: bool,
}

#[cfg(test)]
mod test {
    extern crate std;
    use hex_literal::hex;
    use std::format;

    use super::{NibbleBuf, NibbleBufMut};
    use crate::serdes::nibble_buf::Error;

    #[test]
    fn read_nibbles() {
        let buf = [0xab, 0xcd, 0xef];
        let mut rdr = NibbleBuf::new_all(&buf);
        assert_eq!(rdr.get_nibble(), Ok(0xa));
        assert_eq!(rdr.get_nibble(), Ok(0xb));
        assert_eq!(rdr.get_nibble(), Ok(0xc));
        assert_eq!(rdr.get_nibble(), Ok(0xd));
        assert_eq!(rdr.get_nibble(), Ok(0xe));
        assert_eq!(rdr.get_nibble(), Ok(0xf));
        assert!(rdr.is_at_end());
    }

    #[test]
    fn out_of_bounds() {
        let buf = [0xa0];
        let mut rdr = NibbleBuf::new(&buf, 1).unwrap();
        assert_eq!(rdr.get_nibble(), Ok(0xa));
        assert_eq!(rdr.get_nibble(), Err(Error::OutOfBounds));
    }

    #[test]
    fn read_u8() {
        let buf = [0x12, 0x34, 0x56];
        let mut rdr = NibbleBuf::new_all(&buf);
        assert_eq!(rdr.get_nibble(), Ok(0x1));
        assert_eq!(rdr.get_u8(), Ok(0x23));
        assert_eq!(rdr.get_nibble(), Ok(0x4));
        assert_eq!(rdr.get_u8(), Ok(0x56));
        assert!(rdr.is_at_end());
    }

    #[test]
    fn read_past_end() {
        let buf = [0xaa, 0xbb, 0xcc];
        let mut rdr = NibbleBuf::new_all(&buf[0..=1]);
        rdr.get_u8().unwrap();
        rdr.get_u8().unwrap();
        assert!(rdr.is_at_end());
        assert_eq!(rdr.get_u8(), Err(Error::OutOfBounds));
    }

    #[test]
    fn write_nibbles() {
        let mut buf = [0u8; 2];
        let mut wgr = NibbleBufMut::new_all(&mut buf);
        wgr.put_nibble(1).unwrap();
        wgr.put_nibble(2).unwrap();
        wgr.put_nibble(3).unwrap();
        wgr.put_nibble(4).unwrap();
        assert!(wgr.is_at_end());
        assert_eq!(wgr.put_nibble(0), Err(Error::OutOfBounds));
        let (buf, byte_pos, is_at_byte_boundary) = wgr.finish();
        assert_eq!(buf[0], 0x12);
        assert_eq!(buf[1], 0x34);
        assert_eq!(byte_pos, 2);
        assert!(is_at_byte_boundary);
    }

    #[test]
    fn buf_display() {
        let buf = [0x12, 0x34, 0x56];
        let buf = NibbleBuf::new_all(&buf);
        assert_eq!(format!("{}", buf), "NibbleBuf(1 2 3 4 5 6)")
    }

    #[test]
    fn buf_display_partly_consumed() {
        let buf = [0x12, 0x43, 0x21];
        let mut buf = NibbleBuf::new_all(&buf);
        let _ = buf.get_nibble();
        let _ = buf.get_nibble();
        assert_eq!(format!("{}", buf), "NibbleBuf(<2< 4 3 2 1)")
    }

    #[test]
    fn get_bit_buf() {
        let buf = [0x12, 0x34, 0x56];
        let mut rgr = NibbleBuf::new_all(&buf);

        let mut bits_7_0_rgr = rgr.get_bit_buf(2).unwrap();
        assert_eq!(bits_7_0_rgr.get_up_to_8(4), Ok(0x1));
        assert_eq!(bits_7_0_rgr.get_up_to_8(4), Ok(0x2));
        assert!(bits_7_0_rgr.get_bit().is_err());

        assert_eq!(rgr.get_nibble(), Ok(0x3));

        let mut bits_11_0_rgr = rgr.get_bit_buf(3).unwrap();
        assert!(rgr.is_at_end());
        assert_eq!(bits_11_0_rgr.get_up_to_8(4), Ok(0x4));
        assert_eq!(bits_11_0_rgr.get_up_to_16(8), Ok(0x56));
        assert!(bits_11_0_rgr.get_bit().is_err());
    }

    #[test]
    fn get_bit_buf_in_the_middle() {
        let buf = [0x12, 0x34, 0x56, 0x78];
        let mut rgr = NibbleBuf::new_all(&buf);
        let _ = rgr.get_nibble().unwrap();
        let mut bits_3_0_rgr = rgr.get_bit_buf(1).unwrap();
        assert_eq!(bits_3_0_rgr.get_up_to_8(4), Ok(0x2));
        assert!(bits_3_0_rgr.get_bit().is_err());

        let mut bits_3_0_rgr = rgr.get_bit_buf(1).unwrap();
        assert_eq!(bits_3_0_rgr.get_up_to_8(4), Ok(0x3));
        assert!(bits_3_0_rgr.get_bit().is_err());

        let mut bits_11_0_rgr = rgr.get_bit_buf(3).unwrap();
        assert_eq!(bits_11_0_rgr.get_up_to_16(12), Ok(0x456));
        assert!(bits_11_0_rgr.get_bit().is_err());

        assert_eq!(rgr.get_u8(), Ok(0x78));
        assert!(rgr.is_at_end());
    }

    #[test]
    fn to_bit_buf() {
        let mut buf = [0u8; 2];
        let mut wgr = NibbleBufMut::new_all(&mut buf);
        wgr.put_nibble(0b1010).unwrap();

        let mut wgr = wgr.to_bit_buf();
        wgr.put_up_to_8(8, 0b1111_1010).unwrap();
        wgr.put_up_to_8(4, 0b0011).unwrap();

        let (buf, byte_pos, bit_pos) = wgr.finish();
        assert_eq!(buf[0], 0b1010_1111);
        assert_eq!(buf[1], 0b1010_0011);
        assert_eq!(byte_pos, 2);
        assert_eq!(bit_pos, 0);
    }

    #[test]
    fn put_nibble_buf_both_byte_aligned() {
        let rgr_buf = [1, 2, 3];
        let rgr = NibbleBuf::new_all(&rgr_buf);

        let mut wgr_buf = [0u8; 5];
        let mut wgr = NibbleBufMut::new_all(&mut wgr_buf);
        wgr.put_u8(9).unwrap();
        wgr.put_u8(8).unwrap();

        wgr.put_nibble_buf(&rgr).unwrap();
        assert_eq!(wgr.nibbles_pos(), 10);
        let (wgr_buf, len, _) = wgr.finish();
        assert_eq!(len, 5);
        assert_eq!(wgr_buf, &[9, 8, 1, 2, 3]);
    }

    #[test]
    fn put_nibble_buf_both_nibble_aligned() {
        let rgr_buf = hex!("ab cd ef");
        let mut rgr = NibbleBuf::new_all(&rgr_buf);
        let _ = rgr.get_nibble().unwrap();

        let mut wgr_buf = [0u8; 4];
        let mut wgr = NibbleBufMut::new_all(&mut wgr_buf);
        wgr.put_u8(0xff).unwrap();
        wgr.put_nibble(1).unwrap();

        wgr.put_nibble_buf(&rgr).unwrap();
        assert_eq!(wgr.nibbles_pos(), 8);
        let (wgr_buf, len, _) = wgr.finish();
        assert_eq!(len, 4);
        assert_eq!(wgr_buf, &[0xff, 0x1b, 0xcd, 0xef]);
    }

    #[test]
    fn put_nibble_buf_wgr_unaligned() {
        let rgr_buf = hex!("ab cd ef");
        let rgr = NibbleBuf::new_all(&rgr_buf);

        let mut wgr_buf = [0u8; 4];
        let mut wgr = NibbleBufMut::new_all(&mut wgr_buf);
        wgr.put_nibble(1).unwrap();

        wgr.put_nibble_buf(&rgr).unwrap();
        assert_eq!(wgr.nibbles_pos(), 7);
        let (wgr_buf, pos, is_at_byte_boundary) = wgr.finish();
        assert_eq!(wgr_buf, &[0x1a, 0xbc, 0xde, 0xf0]);
        assert_eq!(pos, 3);
        assert!(!is_at_byte_boundary);
    }

    #[test]
    fn put_nibble_buf_rgr_unaligned() {
        let rgr_buf = hex!("ab cd ef");
        let mut rgr = NibbleBuf::new_all(&rgr_buf);
        let _ = rgr.get_nibble().unwrap();

        let mut wgr_buf = [0u8; 4];
        let mut wgr = NibbleBufMut::new_all(&mut wgr_buf);
        wgr.put_u8(0xff).unwrap();

        wgr.put_nibble_buf(&rgr).unwrap();
        let (wgr_buf, pos, is_at_byte_boundary) = wgr.finish();
        assert_eq!(wgr_buf, &[0xff, 0xbc, 0xde, 0xf0]);
        assert_eq!(pos, 3);
        assert!(!is_at_byte_boundary);
    }

    #[test]
    fn replace_nibble() {
        let mut buf = [0u8; 4];
        let mut wgr = NibbleBufMut::new_all(&mut buf);
        wgr.put_u8(0x00).unwrap();
        wgr.put_u8(0xab).unwrap();
        wgr.put_u8(0xcd).unwrap();
        wgr.replace_nibble(0, 0xe).unwrap();
        wgr.replace_nibble(2, 0xf).unwrap();
        wgr.replace_nibble(5, 1).unwrap();

        wgr.replace_nibble(6, 0xf).unwrap();
        wgr.put_u8(0x12).unwrap();
        let (buf, _, _) = wgr.finish();
        assert_eq!(buf, hex!("e0 fb c1 12"));
    }

    #[test]
    fn rewind() {
        let mut buf = [0u8; 2];
        let mut wrr = NibbleBufMut::new_all(&mut buf);
        wrr.skip(2).unwrap();
        wrr.put_u8(0xaa).unwrap();
        wrr.rewind::<_, Error>(0, |wrr| {
            wrr.put_nibble(1)?;
            wrr.put_nibble(2)?;
            Ok(())
        })
        .unwrap();
        let (buf, _, _) = wrr.finish();
        assert_eq!(buf[0], 0x12);
        assert_eq!(buf[1], 0xaa);
    }

    #[test]
    fn save_and_restore() {
        let mut buf = [0u8; 4];
        let mut wrr = NibbleBufMut::new_all(&mut buf);
        wrr.put_u8(0xaa).unwrap();
        wrr.put_u8(0xbb).unwrap();
        let state = wrr.save_state();
        wrr.put_u8(0x11).unwrap();
        wrr.put_u8(0x22).unwrap();
        wrr.restore_state(state).unwrap();
        assert_eq!(wrr.nibbles_pos(), 4);
    }

    #[test]
    fn get_buf_slice() {
        let input = hex!("aa bb cc dd ee ff 11");
        let mut nrd = NibbleBuf::new_all(&input);

        let mut nrd_aa = nrd.get_buf_slice(2).unwrap();
        assert_eq!(nrd_aa.nibbles_left(), 2);
        assert_eq!(nrd_aa.get_u8(), Ok(0xaa));

        assert_eq!(nrd.get_nibble(), Ok(0xb));

        let mut nrd_bcc = nrd.get_buf_slice(3).unwrap();
        assert_eq!(nrd_bcc.nibbles_left(), 3);
        assert_eq!(nrd_bcc.get_u8(), Ok(0xbc));
        assert_eq!(nrd_bcc.get_nibble(), Ok(0xc));

        assert_eq!(nrd.get_nibble(), Ok(0xd));

        let mut nrd_de = nrd.get_buf_slice(2).unwrap();
        assert_eq!(nrd_de.nibbles_left(), 2);
        assert_eq!(nrd_de.get_u8(), Ok(0xde));

        let mut nrd_e = nrd.get_buf_slice(1).unwrap();
        assert_eq!(nrd_e.nibbles_left(), 1);
        assert_eq!(nrd_e.get_nibble(), Ok(0xe));

        let mut nrd_ff1 = nrd.get_buf_slice(3).unwrap();
        assert_eq!(nrd_ff1.nibbles_left(), 3);
        assert_eq!(nrd_ff1.get_u8(), Ok(0xff));
        assert_eq!(nrd_ff1.get_nibble(), Ok(0x1));

        assert_eq!(nrd.get_nibble(), Ok(0x1));
        assert!(nrd.is_at_end());
    }

    #[test]
    fn non_zero_buf() {
        let mut buf = [0xab, 0xcd, 0xef];
        let mut nwr = NibbleBufMut::new_all(&mut buf);
        nwr.put_nibble(0).unwrap();
        nwr.put_nibble(0).unwrap();
        nwr.put_nibble(0).unwrap();
        nwr.put_nibble(0xa).unwrap();
        nwr.put_nibble(0xb).unwrap();
        let (buf, _, _) = nwr.finish();
        assert_eq!(buf, [0x00, 0x0a, 0xbf]);
    }
}
