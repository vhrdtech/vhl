use core::fmt::{Debug, Display, Formatter};
use core::iter::FusedIterator;
use crate::discrete::{U3, U4, U6};
use crate::serdes::{DeserializeVlu4, NibbleBuf};
use crate::serdes::vlu4::{Vlu4U32Array, Vlu4U32ArrayIter};
use crate::serdes::xpi_vlu4::error::XpiVlu4Error;

/// Sequence of numbers uniquely identifying one of the resources.
/// If there is a group in the uri with not numerical index - it must be mapped into numbers.
#[derive(Copy, Clone)]
pub enum Uri<'i> {
    /// Points to one of the root resources /i, i <= 15; takes 1 nibble
    OnePart4(U4),

    /// Points to one of the root child resources /i/j, i,j <= 15; takes 2 nibbles
    TwoPart44(U4, U4),

    /// Points into third level of the resource tree /i/j/k, i,j,k <= 15; takes 3 nibbles
    ThreePart444(U4, U4, U4),

    /// Points into third level of the resource tree /i/j/k, i <= 63, j,k <= 7; takes 3 nibbles
    ThreePart633(U6, U3, U3),

    /// Points into third level of the resource tree /i/j/k, i,j <= 63, k <= 15; takes 4 nibbles
    ThreePart664(U6, U6, U4),

    /// Point to any resource in the resources tree, any numbers up to u32::MAX; variable size
    MultiPart(Vlu4U32Array<'i>)
}

impl<'i> Uri<'i> {
    pub fn iter(&self) -> UriIter<'i> {
        match self {
            Uri::OnePart4(a) => UriIter::UpToThree {
                parts: [a.inner(), 0, 0],
                len: 1,
                pos: 0
            },
            Uri::TwoPart44(a, b) => UriIter::UpToThree {
                parts: [a.inner(), b.inner(), 0],
                len: 2,
                pos: 0
            },
            Uri::ThreePart444(a, b, c) => UriIter::UpToThree {
                parts: [a.inner(), b.inner(), c.inner()],
                len: 3,
                pos: 0
            },
            Uri::ThreePart633(a, b, c) => UriIter::UpToThree {
                parts: [a.inner(), b.inner(), c.inner()],
                len: 3,
                pos: 0
            },
            Uri::ThreePart664(a, b, c) => UriIter::UpToThree {
                parts: [a.inner(), b.inner(), c.inner()],
                len: 3,
                pos: 0
            },
            Uri::MultiPart(arr) => {
                UriIter::ArrIter(arr.iter())
            }
        }
    }
}

impl<'i> DeserializeVlu4<'i> for Uri<'i> {
    type Error = XpiVlu4Error;

    fn des_vlu4<'di>(rdr: &'di mut NibbleBuf<'i>) -> Result<Self, Self::Error> {
        Ok(Uri::MultiPart(rdr.des_vlu4()?))
    }
}

impl<'i> IntoIterator for Uri<'i> {
    type Item = u32;
    type IntoIter = UriIter<'i>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub enum UriIter<'i> {
    UpToThree {
        parts: [u8; 3],
        len: u8,
        pos: u8,
    },
    ArrIter(Vlu4U32ArrayIter<'i>)
}

impl<'i> Iterator for UriIter<'i> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            UriIter::UpToThree { parts, len, pos } => {
                if pos < len {
                    *pos += 1;
                    Some(parts[(*pos - 1) as usize] as u32)
                } else {
                    None
                }
            }
            UriIter::ArrIter(arr_iter) => {
                arr_iter.next()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            UriIter::UpToThree { len, .. } => (*len as usize, Some(*len as usize)),
            UriIter::ArrIter(arr_iter) => arr_iter.size_hint(),
        }
    }
}

impl<'i> FusedIterator for UriIter<'i> {}

impl<'i> Display for Uri<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut uri_iter = self.iter().peekable();
        if f.alternate() {
            write!(f, "Uri(/")?;
        } else {
            write!(f, "/")?;
        }
        while let Some(uri_part) = uri_iter.next() {
            write!(f, "{}", uri_part)?;
            if uri_iter.peek().is_some() {
                write!(f, "/")?;
            }
        }
        if f.alternate() {
            write!(f, ")")
        } else {
            write!(f, "")
        }
    }
}

impl<'i> Debug for Uri<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use std::format;

    use crate::serdes::NibbleBuf;
    use super::Uri;
    use crate::serdes::vlu4::Vlu4U32Array;

    #[test]
    fn one_part_uri_iter() {
        let uri = Uri::OnePart(1);
        let mut uri_iter = uri.iter();
        assert_eq!(uri_iter.next(), Some(1));
        assert_eq!(uri_iter.next(), None);
    }

    #[test]
    fn two_part_uri_iter() {
        let uri = Uri::TwoPart(1, 2);
        let mut uri_iter = uri.iter();
        assert_eq!(uri_iter.next(), Some(1));
        assert_eq!(uri_iter.next(), Some(2));
        assert_eq!(uri_iter.next(), None);
    }

    #[test]
    fn three_part_uri_iter() {
        let uri = Uri::ThreePart(1, 2, 3);
        let mut uri_iter = uri.iter();
        assert_eq!(uri_iter.next(), Some(1));
        assert_eq!(uri_iter.next(), Some(2));
        assert_eq!(uri_iter.next(), Some(3));
        assert_eq!(uri_iter.next(), None);
    }

    #[test]
    fn multi_part_uri_iter() {
        let buf = [0x51, 0x23, 0x45];
        let mut buf = NibbleBuf::new_all(&buf);
        let arr: Vlu4U32Array = buf.des_vlu4().unwrap();
        let uri = Uri::MultiPart(arr);
        let mut uri_iter = uri.iter();
        assert_eq!(uri_iter.next(), Some(1));
        assert_eq!(uri_iter.next(), Some(2));
        assert_eq!(uri_iter.next(), Some(3));
        assert_eq!(uri_iter.next(), Some(4));
        assert_eq!(uri_iter.next(), Some(5));
        assert_eq!(uri_iter.next(), None);
    }

    #[test]
    fn uri_display() {
        let buf = [0x51, 0x23, 0x45];
        let mut buf = NibbleBuf::new_all(&buf);
        let arr: Vlu4U32Array = buf.des_vlu4().unwrap();
        let uri = Uri::MultiPart(arr);
        assert_eq!(format!("{:#}", uri), "Uri(/1/2/3/4/5)");
        assert_eq!(format!("{}", uri), "/1/2/3/4/5");
    }
}