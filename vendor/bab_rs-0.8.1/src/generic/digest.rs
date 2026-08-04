use zeroize::Zeroize;

use order_theory::{
    GreatestElement, LeastElement, LowerSemilattice, PredecessorExceptForLeast,
    SuccessorExceptForGreatest, TryPredecessor, TrySuccessor, UpperSemilattice,
};

/// A Bab digest of a string.
#[derive(Eq, PartialOrd, Ord, Clone, Debug, Hash)]
#[cfg_attr(feature = "dev", derive(arbitrary::Arbitrary))]
#[repr(transparent)]
pub struct BabDigest<const WIDTH: usize>(pub(crate) [u8; WIDTH]);

impl<const WIDTH: usize> BabDigest<WIDTH> {
    /// Converts `self` into the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak values secrets.
    pub fn into_bytes(self) -> [u8; WIDTH] {
        self.0
    }

    /// Returns a reference to the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak values secrets.
    pub fn as_bytes(&self) -> &[u8; WIDTH] {
        &self.0
    }

    /// Returns a mutable reference to the underlying byte array.
    ///
    /// This type deliberately does not provide this functionality through a trait, in order to make it less likely to leak values secrets.
    pub fn as_mut_bytes(&mut self) -> &mut [u8; WIDTH] {
        &mut self.0
    }
}

impl<const WIDTH: usize> From<[u8; WIDTH]> for BabDigest<WIDTH> {
    fn from(value: [u8; WIDTH]) -> Self {
        Self(value)
    }
}

impl<const WIDTH: usize> PartialEq for BabDigest<WIDTH> {
    fn eq(&self, other: &Self) -> bool {
        constant_time_eq::constant_time_eq_n(&self.0, &other.0)
    }
}

impl<const WIDTH: usize> Default for BabDigest<WIDTH> {
    fn default() -> Self {
        [0; WIDTH].into()
    }
}

impl<const WIDTH: usize> Drop for BabDigest<WIDTH> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<const WIDTH: usize> LeastElement for BabDigest<WIDTH> {
    fn least() -> Self {
        <[u8; WIDTH]>::least().into()
    }
}

impl<const WIDTH: usize> GreatestElement for BabDigest<WIDTH> {
    fn greatest() -> Self {
        <[u8; WIDTH]>::greatest().into()
    }
}

impl<const WIDTH: usize> LowerSemilattice for BabDigest<WIDTH> {
    fn greatest_lower_bound(&self, other: &Self) -> Self {
        self.0.greatest_lower_bound(other.as_bytes()).into()
    }
}

impl<const WIDTH: usize> UpperSemilattice for BabDigest<WIDTH> {
    fn least_upper_bound(&self, other: &Self) -> Self {
        self.0.least_upper_bound(other.as_bytes()).into()
    }
}

impl<const WIDTH: usize> TryPredecessor for BabDigest<WIDTH> {
    fn try_predecessor(&self) -> Option<Self> {
        self.0.try_predecessor().map(Self)
    }
}

impl<const WIDTH: usize> TrySuccessor for BabDigest<WIDTH> {
    fn try_successor(&self) -> Option<Self> {
        self.0.try_successor().map(Self)
    }
}

impl<const WIDTH: usize> PredecessorExceptForLeast for BabDigest<WIDTH> {}

impl<const WIDTH: usize> SuccessorExceptForGreatest for BabDigest<WIDTH> {}
