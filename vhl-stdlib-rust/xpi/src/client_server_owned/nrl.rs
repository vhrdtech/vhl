use core::ops::Add;
use core::slice::Iter;
use core::{fmt::Display, ops::Deref};
use std::borrow::Borrow;

use smallvec::{IntoIter, SmallVec};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Nrl(pub SmallVec<[u32; 3]>);
impl Default for Nrl {
    fn default() -> Self {
        Nrl(SmallVec::new())
    }
}

impl Nrl {
    pub fn new(parts: &[u32]) -> Self {
        Nrl(parts.into())
    }

    pub fn push(&mut self, segment: u32) {
        self.0.push(segment);
    }

    pub fn iter(&self) -> Iter<u32> {
        self.0.iter()
    }

    pub fn into_iter(self) -> IntoIter<[u32; 3]> {
        self.0.into_iter()
    }
}

impl PartialEq<[u32]> for Nrl {
    fn eq(&self, other: &[u32]) -> bool {
        self.0.deref() == other
    }
}

macro_rules! impl_for_array_of {
    ($len:literal) => {
        impl PartialEq<[u32; $len]> for Nrl {
            fn eq(&self, other: &[u32; $len]) -> bool {
                self.0.deref() == other
            }
        }

        impl From<[u32; $len]> for Nrl {
            fn from(value: [u32; $len]) -> Self {
                Nrl(value.into_iter().collect())
            }
        }
    };
}
impl_for_array_of!(1);
impl_for_array_of!(2);
impl_for_array_of!(3);
impl_for_array_of!(4);

impl From<Iter<'_, u32>> for Nrl {
    fn from(value: Iter<u32>) -> Self {
        Nrl(value.copied().collect())
    }
}

impl<'i> From<&'i Nrl> for Nrl {
    fn from(value: &'i Nrl) -> Self {
        Nrl(value.0.clone())
    }
}

impl<'i> Add<u32> for &'i Nrl {
    type Output = Nrl;

    fn add(self, rhs: u32) -> Self::Output {
        let mut nrl = self.clone();
        nrl.push(rhs);
        nrl
    }
}
impl<'i> Add<u32> for Nrl {
    type Output = Nrl;

    fn add(mut self, rhs: u32) -> Self::Output {
        self.0.push(rhs);
        self
    }
}

impl Add<Nrl> for Nrl {
    type Output = Nrl;

    fn add(mut self, rhs: Nrl) -> Self::Output {
        self.0.extend(rhs.0);
        self
    }
}

impl Display for Nrl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "/")?;
        let mut it = self.0.iter().peekable();
        while let Some(p) = it.next() {
            write!(f, "{p}")?;
            if it.peek().is_some() {
                write!(f, "/")?;
            }
        }
        Ok(())
    }
}

pub trait NrlIndexable {
    fn try_from_nrl(nrl_iter: &mut impl Iterator<Item = impl Borrow<u32>>) -> Option<Self>
    where
        Self: Sized;
    fn into_nrl(&self) -> Nrl;
}
