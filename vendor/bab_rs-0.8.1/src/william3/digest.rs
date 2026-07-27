use order_theory::{
    GreatestElement, LeastElement, LowerSemilattice, PredecessorExceptForLeast,
    SuccessorExceptForGreatest, TryPredecessor, TrySuccessor, UpperSemilattice,
};

use crate::{WIDTH, generic::BabDigest};

/// A WILLIAM3 digest of a string.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug, Hash)]
#[cfg_attr(feature = "dev", derive(arbitrary::Arbitrary))]
#[repr(transparent)]
pub struct William3Digest(pub(crate) BabDigest<WIDTH>);

impl William3Digest {
    /// Converts `self` into the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak values secrets.
    pub fn into_bytes(self) -> [u8; WIDTH] {
        self.0.into_bytes()
    }

    /// Returns a reference to the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak values secrets.
    pub fn as_bytes(&self) -> &[u8; WIDTH] {
        self.0.as_bytes()
    }

    /// Returns a mutable reference to the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak values secrets.
    pub fn as_mut_bytes(&mut self) -> &mut [u8; WIDTH] {
        self.0.as_mut_bytes()
    }
}

impl From<BabDigest<WIDTH>> for William3Digest {
    fn from(value: BabDigest<WIDTH>) -> Self {
        Self(value)
    }
}

impl From<[u8; WIDTH]> for William3Digest {
    fn from(value: [u8; WIDTH]) -> Self {
        Self(value.into())
    }
}

impl From<William3Digest> for BabDigest<WIDTH> {
    fn from(value: William3Digest) -> Self {
        value.0
    }
}

impl AsRef<BabDigest<WIDTH>> for William3Digest {
    fn as_ref(&self) -> &BabDigest<WIDTH> {
        &self.0
    }
}

impl AsMut<BabDigest<WIDTH>> for William3Digest {
    fn as_mut(&mut self) -> &mut BabDigest<WIDTH> {
        &mut self.0
    }
}

impl Default for William3Digest {
    fn default() -> Self {
        <BabDigest<WIDTH>>::default().into()
    }
}

impl LeastElement for William3Digest {
    fn least() -> Self {
        <BabDigest<WIDTH>>::least().into()
    }
}

impl GreatestElement for William3Digest {
    fn greatest() -> Self {
        <BabDigest<WIDTH>>::greatest().into()
    }
}

impl LowerSemilattice for William3Digest {
    fn greatest_lower_bound(&self, other: &Self) -> Self {
        self.0.greatest_lower_bound(other.as_ref()).into()
    }
}

impl UpperSemilattice for William3Digest {
    fn least_upper_bound(&self, other: &Self) -> Self {
        self.0.least_upper_bound(other.as_ref()).into()
    }
}

impl TryPredecessor for William3Digest {
    fn try_predecessor(&self) -> Option<Self> {
        self.0.try_predecessor().map(Self)
    }
}

impl TrySuccessor for William3Digest {
    fn try_successor(&self) -> Option<Self> {
        self.0.try_successor().map(Self)
    }
}

impl PredecessorExceptForLeast for William3Digest {}

impl SuccessorExceptForGreatest for William3Digest {}
