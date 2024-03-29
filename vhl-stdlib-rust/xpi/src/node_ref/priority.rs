use crate::priority::XpiGenericPriority;
use core::fmt::{Display, Formatter};
use vhl_stdlib::discrete::U2;
use vhl_stdlib::serdes::bit_buf::BitBufMut;
use vhl_stdlib::serdes::traits::SerializeBits;
use vhl_stdlib::serdes::{bit_buf, BitBuf, DeserializeBits};

pub type Priority = XpiGenericPriority<U2>;

impl<'i> DeserializeBits<'i> for Priority {
    type Error = bit_buf::Error;

    fn des_bits<'di>(rdr: &'di mut BitBuf<'i>) -> Result<Self, Self::Error> {
        let is_lossless = rdr.get_bit()?;
        if is_lossless {
            Ok(Priority::Lossless(rdr.des_bits()?))
        } else {
            Ok(Priority::Lossy(rdr.des_bits()?))
        }
    }
}

impl SerializeBits for Priority {
    type Error = bit_buf::Error;

    fn ser_bits(&self, wgr: &mut BitBufMut) -> Result<(), Self::Error> {
        let (is_lossless, level) = match self {
            Priority::Lossy(level) => (false, level),
            Priority::Lossless(level) => (true, level),
        };
        wgr.put_bit(is_lossless)?;
        wgr.put(level)
    }
}

impl Display for Priority {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Priority::Lossy(level) => write!(f, "L{}", level.inner()),
            Priority::Lossless(level) => write!(f, "R{}", level.inner()),
        }
    }
}
