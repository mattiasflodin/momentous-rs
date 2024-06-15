use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Add, Sub};
use std::time::SystemTime;

use num_integer::Integer;
use num_traits::{Bounded, PrimInt};
use numcmp::NumCmp;
use thiserror::Error;

use crate::scale::Seconds;
use crate::widen::Widen;
use crate::zoneinfo::get_leap_second_adjustment_for_unix_timestamp;
use crate::Nanoseconds;
use crate::{Duration, Scale};

pub trait Tick: PrimInt + Bounded + Hash + Eq + Copy + Ord + PartialOrd + Integer {}

impl<T: PrimInt + Hash + Eq + Copy + Ord + PartialOrd + Integer> Tick for T {}

// TODO rename to "moment"? Would make sense given the name of the library.
#[derive(Debug, Clone, Copy, Hash)]
pub struct Instant<T: Tick, S: Scale> {
    // Ticks since the Unix epoch. Note that this is not the same as Unix time, since
    // Unix time skips leap seconds (i.e. it considers every day to have exactly 86400 seconds).
    // This is the actual number of ticks including leap seconds.
    // TODO make this private
    pub(crate) ticks: T,
    phantom: PhantomData<S>,
}

#[derive(Error, Debug)]
#[error("instant is out of range")]
pub struct InstantOutOfRange;

pub type InstantS32 = Instant<i32, Seconds>;
pub type InstantS64 = Instant<i64, Seconds>;

pub type InstantS128 = Instant<i128, Seconds>;
pub type InstantNs128 = Instant<i128, Nanoseconds>;

impl<T: Tick, S: Scale> Instant<T, S> {
    pub fn min_value() -> Self {
        Instant {
            ticks: T::min_value(),
            phantom: PhantomData,
        }
    }

    pub fn max_value() -> Self {
        Instant {
            ticks: T::max_value(),
            phantom: PhantomData,
        }
    }

    pub fn from_ticks_since_epoch(ticks: T) -> Self {
        Self {
            ticks,
            phantom: PhantomData,
        }
    }

    pub fn epoch() -> Self {
        Instant::from_ticks_since_epoch(T::zero())
    }

    // Can't implement TryFrom trait because of the blanket specialization in core.
    // https://github.com/rust-lang/rust/issues/50133
    pub fn try_from<T2>(value: Instant<T2, S>) -> Result<Self, InstantOutOfRange>
    where
        T2: Tick,
        T2: TryInto<T>,
    {
        let ticks = value.ticks.try_into().map_err(|_| InstantOutOfRange)?;
        Ok(Instant::from_ticks_since_epoch(ticks))
    }

    // TODO maybe use the error type from TryInto instead
    pub fn try_into<T2>(self) -> Result<Instant<T2, S>, InstantOutOfRange>
    where
        T2: Tick,
        T: TryInto<T2>,
    {
        let ticks = self.ticks.try_into().map_err(|_| InstantOutOfRange)?;
        Ok(Instant::from_ticks_since_epoch(ticks))
    }

    pub fn into<T2>(self) -> Instant<T2, S>
    where
        T2: Tick,
        T: Into<T2>,
    {
        Instant::from_ticks_since_epoch(self.ticks.into())
    }

    pub fn widen<T2: Tick>(self) -> Instant<<T as Widen<T2>>::Output, S>
    where
        T: Widen<T2>,
    {
        Instant::from_ticks_since_epoch(self.ticks.widen())
    }

    pub fn floor<S2: Scale>(&self) -> Instant<T, S2> {
        assert!(
            S2::TICKS_PER_SECOND <= S::TICKS_PER_SECOND,
            "Cannot floor scale to a higher scale"
        );
        let factor = T::from(S::TICKS_PER_SECOND).unwrap() / T::from(S2::TICKS_PER_SECOND).unwrap();
        Instant::from_ticks_since_epoch(self.ticks.div_floor(&factor))
    }

    pub fn split<S2: Scale>(&self) -> (Instant<T, S2>, Duration<T, S>) {
        assert!(
            S2::TICKS_PER_SECOND <= S::TICKS_PER_SECOND,
            "Cannot split scale to a higher scale"
        );
        let factor = T::from(S::TICKS_PER_SECOND).unwrap() / T::from(S2::TICKS_PER_SECOND).unwrap();
        let (ticks, remainder) = self.ticks.div_mod_floor(&factor);
        (
            Instant::from_ticks_since_epoch(ticks),
            Duration::new(remainder),
        )
    }

    pub fn extend<T2: Tick, S2: Scale>(&self) -> Option<Instant<T2, S2>> {
        assert!(
            S2::TICKS_PER_SECOND >= S::TICKS_PER_SECOND,
            "Cannot extend scale to a lower scale"
        );
        let ticks = T2::from(self.ticks)?;
        let factor =
            T2::from(S2::TICKS_PER_SECOND).unwrap() / T2::from(S::TICKS_PER_SECOND).unwrap();
        let ticks = ticks.checked_mul(&factor)?;
        Some(Instant::from_ticks_since_epoch(ticks))
    }

    pub fn duration_since_epoch(&self) -> Duration<T, S> {
        Duration::new(self.ticks)
    }

    pub fn ticks_since_epoch(&self) -> T {
        self.ticks
    }
}

impl<T: Tick, S: Scale> Sub for Instant<T, S> {
    type Output = Duration<T, S>;

    fn sub(self, rhs: Self) -> Self::Output {
        Duration::new(
            self.ticks
                .checked_sub(&rhs.ticks)
                .expect("instant subtraction underflow"),
        )
    }
}

impl<T: Tick, S: Scale> Add<Duration<T, S>> for Instant<T, S> {
    type Output = Self;

    fn add(self, rhs: Duration<T, S>) -> Self::Output {
        Self::from_ticks_since_epoch(
            self.ticks
                .checked_add(&rhs.ticks())
                .expect("instant addition overflow"),
        )
    }
}

/*impl<T: Tick, S1: Scale, S2: Scale> PartialEq<Instant<T, S2>> for Instant<T, S1> {
    fn eq(&self, other: &Instant<T, S2>) -> bool {
        // If
        //
        // t1/s1 == t2/s2
        //
        // then
        //
        // t1*s2 == t2*s1
        //
        // If s1 < s2 then this can be rewritten as
        //
        // t1 == t2*s2/s1.
        //
        // If s1 > s2, it can be rewritten as
        //
        // t1*s1/s2 == t2.
        //
        // If s1 == s2, then it simplifies to
        //
        // t1 == t2.
        let t1 = self.ticks;
        let s1 = S1::TICKS_PER_SECOND;
        let t2 = other.ticks;
        let s2 = S2::TICKS_PER_SECOND;
        if s1 < s2 {
            let factor = T::from(s2/s1).expect("scale conversion factor is too large for type T");
            t1 == t2*factor
        } else if s1 > s2 {
            let factor = T::from(s1/s2).expect("scale conversion factor is too large for type T");
            t1*factor == t2
        } else {
            t1 == t2
        }
    }
}*/

impl<T1: Tick, S1: Scale, T2: Tick, S2: Scale> PartialEq<Instant<T2, S2>> for Instant<T1, S1>
where
    T1: NumCmp<T2>,
{
    fn eq(&self, other: &Instant<T2, S2>) -> bool {
        // If
        //
        // t1/s1 == t2/s2
        //
        // then
        //
        // t1*s2 == t2*s1
        //
        // If s1 < s2 then this can be rewritten as
        //
        // t1 == t2*s2/s1.
        //
        // If s1 > s2, it can be rewritten as
        //
        // t1*s1/s2 == t2.
        //
        // If s1 == s2, then it simplifies to
        //
        // t1 == t2.
        let t1 = self.ticks;
        let s1 = S1::TICKS_PER_SECOND;
        let t2 = other.ticks;
        let s2 = S2::TICKS_PER_SECOND;
        match s1.cmp(&s2) {
            std::cmp::Ordering::Less => {
                let factor =
                    T2::from(s2 / s1).expect("scale conversion factor is too large for type T2");
                t1.num_eq(t2 * factor)
            }
            std::cmp::Ordering::Greater => {
                let factor =
                    T1::from(s1 / s2).expect("scale conversion factor is too large for type T1");
                (t1 * factor).num_eq(t2)
            }
            std::cmp::Ordering::Equal => t1.num_eq(t2),
        }
    }
}

/*impl<T: Tick, S1: Scale, S2: Scale> PartialOrd<Instant<T, S2>> for Instant<T, S1> {
    fn partial_cmp(&self, other: &Instant<T, S2>) -> Option<std::cmp::Ordering> {
        // Similar to PartialEq, if
        //
        // t1/s1 < t2/s2
        //
        // then
        //
        // t1*s2 < t2*s1
        //
        let t1 = self.ticks;
        let s1 = S1::TICKS_PER_SECOND;
        let t2 = other.ticks;
        let s2 = S2::TICKS_PER_SECOND;
        if s1 < s2 {
            let factor = T::from(s2/s1).expect("scale conversion factor is too large for type T");
            t1.partial_cmp(&(t2*factor))
        } else if s1 > s2 {
            let factor = T::from(s1/s2).expect("scale conversion factor is too large for type T");
            (t1*factor).partial_cmp(&t2)
        } else {
            t1.partial_cmp(&t2)
        }
    }
}*/

impl<T1: Tick, S1: Scale, T2: Tick, S2: Scale> PartialOrd<Instant<T2, S2>> for Instant<T1, S1>
where
    T1: NumCmp<T2>,
{
    fn partial_cmp(&self, other: &Instant<T2, S2>) -> Option<std::cmp::Ordering> {
        // Similar to PartialEq, if
        //
        // t1/s1 < t2/s2
        //
        // then
        //
        // t1*s2 < t2*s1
        //
        let t1 = self.ticks;
        let s1 = S1::TICKS_PER_SECOND;
        let t2 = other.ticks;
        let s2 = S2::TICKS_PER_SECOND;
        match s1.cmp(&s2) {
            std::cmp::Ordering::Less => {
                let factor =
                    T2::from(s2 / s1).expect("scale conversion factor is too large for type T2");
                t1.num_cmp(t2 * factor)
            }
            std::cmp::Ordering::Greater => {
                let factor =
                    T1::from(s1 / s2).expect("scale conversion factor is too large for type T1");
                (t1 * factor).num_cmp(t2)
            }
            std::cmp::Ordering::Equal => t1.num_cmp(t2),
        }
    }
}

/*impl<T1: PrimInt, P1: Precision, T2: PrimInt, S2: PrimInt> TryFrom<Instant<T2, S2>> for Instant<T1, S2> {
    type Error = std::num::TryFromIntError;

    fn try_from(value: Instant<T2, S2>) -> Result<Self, Self::Error> {
        todo!()
        /*if P1::TICKS_PER_SECOND < S2::TICKS_PER_SECOND {
            return Err(std::num::TryFromIntError { kind: std::num::TryFromIntErrorKind::PosOverflow });
        }*/

    }
}*/

impl<T: Tick, S: Scale> TryFrom<SystemTime> for Instant<T, S> {
    type Error = std::time::SystemTimeError;

    fn try_from(value: SystemTime) -> Result<Self, Self::Error> {
        let ticks_per_second =
            T::from(S::TICKS_PER_SECOND).expect("ticks per second is too large for type T)");
        let nanoseconds_per_second =
            T::from(1_000_000_000).expect("nanoseconds per second is too large for type T)");
        let nanoseconds_per_tick = nanoseconds_per_second / ticks_per_second;

        let (mut seconds, subsecond_ns) = system_time_to_time_t(value);
        if !system_time_includes_leap_seconds() {
            seconds += get_leap_second_adjustment_for_unix_timestamp(seconds) as i64;
        }

        // FIXME report proper error here
        let seconds = T::from(seconds).unwrap();
        // FIXME and here
        let subsecond_ns = T::from(subsecond_ns).unwrap();

        let subsec_ticks = subsecond_ns / nanoseconds_per_tick;
        let total_ticks = seconds * ticks_per_second + subsec_ticks;

        Ok(Instant::<T, S> {
            ticks: total_ticks,
            phantom: PhantomData,
        })
    }
}

/// Return the number of seconds and nanoseconds since the Unix epoch, as
/// defined by time_t (i.e. without caring about leap seconds).
fn system_time_to_time_t(value: SystemTime) -> (libc::time_t, u32) {
    let sign = value < SystemTime::UNIX_EPOCH;
    let duration = if sign {
        SystemTime::UNIX_EPOCH.duration_since(value)
    } else {
        value.duration_since(SystemTime::UNIX_EPOCH)
    }
    .unwrap();
    let mut seconds = duration.as_secs() as i64;
    if sign {
        seconds = -seconds;
    }
    (seconds, duration.subsec_nanos())
}

/// Return true if the system time includes leap seconds.
///
/// The Rust documentation for SystemTime specifically says "A SystemTime does not count leap seconds."
/// But it also says that it uses clock_gettime() with CLOCK_REALTIME, which is documented to say
/// that it _does_ include leap seconds on Unix (unlike CLOCK_TAI, which is like CLOCK_REALTIME but without
/// leap seconds). Even so, experimenting on my own Linux system shows that CLOCK_REALTIME and CLOCK_TIME
/// return exactly the same value and clock_adjtime claims there's no leap second offset.
///
/// So it's ambiguous, to say the least. OS leap-second handling is a mess in general. Instead of relying
/// on the OS to follow its own documentation, this function attempts to check for it by using the same
/// facilities to get a time_t value and a gregorian date, and then comparing the time_t value to what
/// it should be given the system's idea of the current date and time.
fn system_time_includes_leap_seconds() -> bool {
    // TODO cache the result of this, but refresh periodically in case things change.

    let now = SystemTime::now();
    let (seconds, _) = system_time_to_time_t(now);
    let mut georgian = libc::tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 1,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null_mut(),
    };
    unsafe {
        libc::gmtime_r(&seconds, &mut georgian);
    }
    // Compute what the time_t value *should* be if SystemTime returns Unix time (i.e. no leap seconds).
    // If the time_t value is different, then the system time includes leap seconds.
    let unix_time = utc_to_unix_time(
        1900 + georgian.tm_year,
        1 + georgian.tm_mon,
        georgian.tm_mday,
        georgian.tm_hour,
        georgian.tm_min,
        georgian.tm_sec,
    );

    unix_time != seconds
}

fn utc_to_unix_time(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: i32) -> i64 {
    // By the unix time definition, every day consists of exactly 86400 seconds. In other words,
    // it disregards leap seconds. We can use julian days calculate this.
    let julian_day = julian_day(year, month, day) as i64;
    let hour = hour as i64;
    let minute = minute as i64;
    let second = second as i64;
    let unix_time = (julian_day - 2440588) * 86400 + hour * 3600 + minute * 60 + second;
    unix_time as i64 // TODO increase precision of calculations above
}

fn julian_day(year: i32, month: i32, day: i32) -> i32 {
    // https://en.wikipedia.org/wiki/Julian_day
    // JDN = (1461 × (Y + 4800 + (M − 14)/12))/4
    // + (367 × (M − 2 − 12 × ((M − 14)/12)))/12
    // − (3 × ((Y + 4900 + (M - 14)/12)/100))/4 + D − 32075

    let y = year as i64;
    let m = month as i64;
    let d = day as i64;
    let jdn = (1461 * (y + 4800 + (m - 14) / 12)) / 4 + (367 * (m - 2 - 12 * ((m - 14) / 12))) / 12
        - (3 * ((y + 4900 + (m - 14) / 12) / 100)) / 4
        + d
        - 32075;
    jdn as i32
}

// TODO how do I implement into() for SystemTime?
/*impl<T: PrimInt, S: Precision> From<SystemTime> for Instant<T, S> {
    fn from(value: SystemTime) -> Self {
        match Self::try_from(value) {
            Ok(instant) => instant,
            Err(err) => panic!("failed to convert SystemTime to Instant: {}", err),
        }
    }
}*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(julian_day(2000, 1, 1), 2451545);
        assert_eq!(julian_day(1970, 1, 1), 2440588);

        let t1: InstantNs128 = SystemTime::now().try_into().unwrap();
        let t2: InstantNs128 = SystemTime::now().try_into().unwrap();
        assert!(t1 <= t2);
    }
}
