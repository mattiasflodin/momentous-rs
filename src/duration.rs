use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Add, Mul, Sub};

use num_rational::Ratio;
use num_traits::PrimInt;

use crate::instant::Tick;
use crate::scale::{Milliseconds, Scale, Seconds};
use crate::widen::Widen;
use crate::{Instant, InstantOutOfRange, Nanoseconds};

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Duration<T: PrimInt, S: Scale> {
    ticks: T,
    phantom: PhantomData<S>,
}

/*impl<T: PrimInt, S: Scale> Duration<T, S> {
    pub(crate) fn as_seconds(&self) -> T {
        self.t / T::from(S::TICKS_PER_SECOND).unwrap()
    }
    pub(crate) fn as_seconds(&self) -> T {
        self.t / T::from(S::TICKS_PER_SECOND).unwrap()
    }
}*/

pub type DurationS32 = Duration<i32, Seconds>;

pub type DurationS64 = Duration<i64, Seconds>;

pub type DurationS128 = Duration<i128, Seconds>;
pub type DurationMs128 = Duration<i128, Milliseconds>;
pub type DurationNs128 = Duration<i128, Nanoseconds>;

impl<T: Tick, S: Scale> Duration<T, S> {
    pub(crate) fn new(t: T) -> Self {
        Self {
            ticks: t,
            phantom: PhantomData,
        }
    }

    /*    pub fn from_seconds(seconds: T) -> Self {
        // TODO check for overflow and generate error
        Self::new(seconds * T::from(S::TICKS_PER_SECOND).unwrap())
    }

    pub fn from_milliseconds(milliseconds: T) -> Self {
        // TODO check for overflow and generate error
        let factor = T::from(S::TICKS_PER_SECOND).unwrap() / T::from(1000).unwrap();
        Self::new(milliseconds * factor)
    }

    pub fn from_microseconds(microseconds: T) -> Self {
        // TODO check for overflow and generate error
        let factor = T::from(S::TICKS_PER_SECOND).unwrap() / T::from(1_000_000).unwrap();
        Self::new(microseconds * factor)
    }

    pub fn from_nanoseconds(nanoseconds: T) -> Self {
        // TODO check for overflow and generate error
        let factor = T::from(S::TICKS_PER_SECOND).unwrap() / T::from(1_000_000_000).unwrap();
        Self::new(nanoseconds * factor)
    }*/

    // Can't implement TryFrom trait because of the blanket specialization in core.
    // https://github.com/rust-lang/rust/issues/50133
    pub fn try_from<T2>(value: Instant<T2, S>) -> Result<Self, InstantOutOfRange>
    where
        T2: Tick,
        T2: TryInto<T>,
    {
        let ticks = value.ticks.try_into().map_err(|_| InstantOutOfRange)?;
        Ok(Duration::new(ticks))
    }

    // TODO maybe use the error type from TryInto instead
    pub fn try_into<T2>(self) -> Result<Duration<T2, S>, InstantOutOfRange>
    where
        T2: Tick,
        T: TryInto<T2>,
    {
        let ticks = self.ticks.try_into().map_err(|_| InstantOutOfRange)?;
        Ok(Duration::new(ticks))
    }

    pub(crate) fn from<T2>(value: Duration<T2, S>) -> Self
    where
        T2: Tick,
        T2: Into<T>,
    {
        Duration::new(value.ticks.into())
    }

    pub fn into<T2>(self) -> Duration<T2, S>
    where
        T2: Tick,
        T: Into<T2>,
    {
        Duration::new(self.ticks.into())
    }

    pub fn widen<T2: Tick>(self) -> Duration<<T as Widen<T2>>::Output, S>
    where
        T: Widen<T2>,
    {
        Duration::new(self.ticks.widen())
    }

    pub fn floor<S2: Scale>(&self) -> Duration<T, S2> {
        assert!(
            S2::TICKS_PER_SECOND <= S::TICKS_PER_SECOND,
            "Cannot floor to a higher scale"
        );
        let factor = T::from(S::TICKS_PER_SECOND).unwrap() / T::from(S2::TICKS_PER_SECOND).unwrap();
        Duration::new(self.ticks / factor)
    }

    pub fn extend<S2: Scale>(&self) -> Option<Duration<T, S2>> {
        assert!(
            S2::TICKS_PER_SECOND >= S::TICKS_PER_SECOND,
            "Cannot extend scale to a lower scale"
        );
        let factor = T::from(S2::TICKS_PER_SECOND).unwrap() / T::from(S::TICKS_PER_SECOND).unwrap();
        Some(Duration::new(self.ticks.checked_mul(&factor)?))
    }

    pub fn div_rem_floor<S2: Scale>(&self, other: Duration<T, S2>) -> (T, Duration<T, S>) {
        // We have two numbers t1/s1 and t2/s2. We want to compute
        //
        // (t1/s1) / (t2/s2) = (t1*s2) / (t2*s1)
        //
        // precisely, i.e. both a quotient and a remainder. The quotient will be returned as
        // is, and the remainder will be scaled up to the scale of the Duration type.
        let t1 = Ratio::new(self.ticks, T::from(S::TICKS_PER_SECOND).unwrap());
        let t2 = Ratio::new(other.ticks, T::from(S2::TICKS_PER_SECOND).unwrap());
        let quotient = (t1 / t2).floor();
        let remainder = t1 - t2 * quotient;
        // Scale the remainder up to the scale parameter of the Duration type.
        let remainder = remainder * T::from(S::TICKS_PER_SECOND).unwrap();
        (quotient.to_integer(), Duration::new(remainder.to_integer()))
    }

    // TODO document that this is not an ordinary ceiling division, as that should
    // generate a negative remainder. But we want the duration to be positive so we negate
    // it. The remainder is actually what you need to subtract from the quotient to get the
    // precise value, i.e. a/b = q - r/b.
    pub fn div_rem_ceil<S2: Scale>(&self, other: Duration<T, S2>) -> (T, Duration<T, S>) {
        // This is the same as with div_rem_floor, but we use a ceiling operation
        // instead of a flooring operation to get the quotient.
        let t1 = Ratio::new(self.ticks, T::from(S::TICKS_PER_SECOND).unwrap());
        let t2 = Ratio::new(other.ticks, T::from(S2::TICKS_PER_SECOND).unwrap());
        let quotient = (t1 / t2).ceil();
        let remainder = t2 * quotient - t1;
        let remainder = remainder * T::from(S::TICKS_PER_SECOND).unwrap();
        (quotient.to_integer(), Duration::new(remainder.to_integer()))
    }

    pub(crate) fn ticks(&self) -> T {
        self.ticks
    }
}

impl<T: Tick, S: Scale> Add for Duration<T, S> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.ticks + rhs.ticks)
    }
}

impl<T: Tick, S: Scale> Sub for Duration<T, S> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.ticks - rhs.ticks)
    }
}

impl<T: Tick, S: Scale> Mul<T> for Duration<T, S> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self::new(self.ticks * rhs)
    }
}
