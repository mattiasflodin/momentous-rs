use std::cmp::max;
use std::hash::Hash;
use std::ops::{Div, Mul, Rem};
use divrem::{DivFloor, DivRem, DivRemFloor};
use momentous::iso8601::chronology;
use num_traits::{PrimInt, Signed};
use zoneinfo_compiled::TZData;
use crate::{Chronology, Instant, Nanoseconds, zoneinfo};
use crate::clamped_div_rem::ClampedDivRem;
use crate::instant::InstantOutOfRange;
use crate::zoneinfo::get_leap_second_segment;

struct WeekDate {
    instant: Instant<i128, Nanoseconds>,
    year: i32,
    week: i32,
    weekday: i32,
}

struct OrdinalDate {
    instant: Instant<i128, Nanoseconds>,
    year: i32,
    day: i32,
}

enum LeapSecondMode {
    IGNORE,
    INSERT,
    SMEAR
}

