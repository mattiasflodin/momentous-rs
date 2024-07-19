use crate::div_rem::ClampedDivRem;
use crate::duration::DurationS64;
use crate::gregorian_normalized_date::GregorianNormalizedDate;
use crate::instant::InstantS64;
use crate::iso8601::chronology::Chronology;
use crate::iso8601::precision::Precision;
use crate::iso8601::{
    DateTimeBuilder, HOURS_PER_DAY, MINUTES_PER_DAY, MINUTES_PER_HOUR, SECONDS_PER_DAY,
    SECONDS_PER_HOUR, SECONDS_PER_MINUTE,
};
use crate::zoneinfo::SegmentLookupResult;
use num_integer::Integer;
use std::cmp::min;
use std::fmt::{Debug, Formatter};

#[derive(Debug, Clone)]
pub struct Carry {
    // TODO what's the maximum theoretical carry that you can get? Probably a lot less than these
    // types allow for.
    days: u32,
    seconds: u64,
}

impl Carry {
    pub fn is_zero(&self) -> bool {
        self.days == 0 && self.seconds == 0
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeWithCarry(DateTime, Carry);

impl DateTimeWithCarry {
    pub(super) fn with_days(date_time: DateTime, days: u32) -> Self {
        DateTimeWithCarry(date_time, Carry { days, seconds: 0 })
    }

    pub(super) fn with_seconds(date_time: DateTime, seconds: u64) -> Self {
        DateTimeWithCarry(date_time, Carry { days: 0, seconds })
    }

    pub fn has_carry(&self) -> bool {
        !self.1.is_zero()
    }

    pub fn days_carry(&self) -> u32 {
        self.1.days
    }

    pub fn seconds_carry(&self) -> u64 {
        self.1.seconds
    }

    pub fn unwrap(self) -> DateTime {
        if !self.has_carry() {
            self.drop_carry()
        } else {
            panic!("Trying to unwrap DateTimeWithCarry that has a carry")
        }
    }

    pub fn drop_carry(self) -> DateTime {
        self.0
    }

    pub fn apply_carry(&self) -> DateTime {
        self.checked_apply_carry()
            .expect("applying carry to a DateTime resulted in an out-of-bounds value")
    }
    pub fn checked_apply_carry(&self) -> Option<DateTime> {
        let carry = self.1.clone();
        let DateTimeWithCarry(result, carry2) = self.0.add_days(carry.days as i32);
        assert_eq!(carry2.days, 0);
        result.checked_add_seconds((carry.seconds + carry2.seconds) as i64)
    }
}

/// An ISO 8601 date and time. The range is from 0000-01-01 to 9999-12-31.
#[derive(Clone, Eq, PartialEq)]
pub struct DateTime {
    // - P Precision: 13, 4 bits
    // - E cycle: 25 [-1-23], 5 bits
    // - C century: 3, 2 bits
    // - Q quadrennium: 24, 5 bits
    // - Y year: 4, 2 bits
    // - D day: 366, 9 bits
    // - S second: 86400+, 17 bits
    // - N nanosecond: 1_000_000_000, 30 bits
    //
    // | w0 ------------------------------------------------------------------------------------|| w1 -----------------|
    // | Byte 7   | Byte 6   | Byte 5   | Byte 4   || Byte 3   | Byte 2   | Byte 1   | Byte 0   || Byte 1   | Byte 0   |
    // | ..NNNNNN | NNNNNNNN | ...PPPPS | EEEEECCQ || QQQQQYYD | DDDDDDDD | SSSSSSSS | SSSSSSSS || NNNNNNNN | NNNNNNNN |
    w0: u64,
    w1: u16,
    chronology: Chronology,
}

impl Debug for DateTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let (precision, gnd, second, nanosecond) = Self::unpack(self.w0, self.w1);
        write!(
            f,
            "DateTime {{ precision: {:?}, gnd: {:?}, second: {}, nanosecond: {} }}",
            precision, gnd, second, nanosecond
        )
    }
}

// The maximum fixed day in the range of DateTime. The maximum fixed day occurs on 9999-12-31
// so this is the number of days between 1970-01-01 and 9999-12-31.
const MAX_FIXED_DAY: u32 = 2932896;

// The minimum fixed day in the range of DateTime. The minimum fixed day occurs on 0000-01-01
// so this is the number of days between 1970-01-01 and 0000-01-01.
const MIN_FIXED_DAY: i32 = -719528;

const MIN_YEAR: u16 = 0;
const MAX_YEAR: u16 = 9999;

// The minimum of the DateTime represented as a GregorianNormalizedDate. The
// date 0000-03-01 is exactly 2000/400 = 5 cycles before the GND epoch. However,
// we want to represent the date 0000-01-01 which is two months before, so we
// need to go back to the preceding cycle/century etc.
const MIN_GND: GregorianNormalizedDate = GregorianNormalizedDate {
    cycle: -6,
    century: 3,
    quadrennium: 24,
    year: 3,
    day: 306,
};

// The maximum of the DateTime represented as a GregorianNormalizedDate. The
// date 10000-03-01 is 8000 years ahead of the GND epoch, which is exactly 20 cycles.
// However, we want to represent the date 9999-12-31 which is two months before
// (minus one day).
const MAX_GND: GregorianNormalizedDate = GregorianNormalizedDate {
    cycle: 19,
    century: 3,
    quadrennium: 24,
    year: 3,
    day: 305,
};

impl DateTime {
    fn pack(
        precision: Precision,
        gnd: GregorianNormalizedDate,
        second: u32,
        nanosecond: u32,
    ) -> (u64, u16) {
        // Cycle is based at 2000-03-01, we need to rebase it to -400-03-01 which is
        // 6 centuries earlier.
        let w0 = Self::pack0(precision, gnd, second, nanosecond);
        let w1 = (nanosecond & 0xFFFF) as u16;
        (w0, w1)
    }

    fn unpack(w0: u64, w1: u16) -> (Precision, GregorianNormalizedDate, u32, u32) {
        let (precision, gnd, second, nanosecond) = Self::unpack0(w0);
        let nanosecond = nanosecond | w1 as u32;
        (precision, gnd, second, nanosecond)
    }

    fn pack0(
        precision: Precision,
        gnd: GregorianNormalizedDate,
        second: u32,
        nanosecond: u32,
    ) -> u64 {
        let p = Self::encode_precision(precision);
        // Cycle is based at 2000-03-01, we need to rebase it to -400-03-01 which is
        // 6 centuries earlier.
        let cycle = gnd.cycle;
        let century = gnd.century;
        let quadrennium = gnd.quadrennium;
        let year = gnd.year;
        let day = gnd.day;
        let rebased_cycle = cycle as i32 + 6;
        ((nanosecond & 0x7FFF0000) as u64) << 48
            | (p as u64) << 41
            | ((second & 0x10000) as u64) << 40
            | ((rebased_cycle & 0x1F) as u64) << 35
            | ((century & 0x3) as u64) << 33
            | ((quadrennium & 0x1F) as u64) << 27
            | ((year & 0x3) as u64) << 25
            | ((day & 0x1FF) as u64) << 16
            | (second & 0xFFFF) as u64
    }

    fn unpack0(w0: u64) -> (Precision, GregorianNormalizedDate, u32, u32) {
        let lower_second = (w0 & 0xFFFF) as u32;
        let day = ((w0 >> 16) & 0x1FF) as u16;
        let year = ((w0 >> 25) & 0x3) as u8;
        let quadrennium = ((w0 >> 27) & 0x1F) as u8;
        let century = ((w0 >> 33) & 0x3) as u8;
        let cycle = ((w0 >> 35) & 0x1F) as i8 - 6;
        let second = ((w0 >> 40) & 0x10000) as u32 | lower_second;
        let precision = Self::decode_precision((w0 >> 41) as u8);
        let nanosecond = ((w0 >> 48) & 0x7FFF0000) as u32;
        let gnd = GregorianNormalizedDate {
            cycle,
            century,
            quadrennium,
            year,
            day,
        };
        (precision, gnd, second, nanosecond)
    }

    fn encode_precision(precision: Precision) -> u8 {
        match precision {
            Precision::Millennia => 0,
            Precision::Centuries => 1,
            Precision::Decades => 2,
            Precision::Years => 3,
            Precision::Months => 4,
            Precision::Weeks => 5,
            Precision::Days => 6,
            Precision::Hours => 7,
            Precision::Minutes => 8,
            Precision::Seconds => 9,
            Precision::Milliseconds => 10,
            Precision::Microseconds => 11,
            Precision::Nanoseconds => 12,
        }
    }

    fn decode_precision(precision: u8) -> Precision {
        match precision {
            0 => Precision::Millennia,
            1 => Precision::Centuries,
            2 => Precision::Decades,
            3 => Precision::Years,
            4 => Precision::Months,
            5 => Precision::Weeks,
            6 => Precision::Days,
            7 => Precision::Hours,
            8 => Precision::Minutes,
            9 => Precision::Seconds,
            10 => Precision::Milliseconds,
            11 => Precision::Microseconds,
            12 => Precision::Nanoseconds,
            // Should never happen but we don't need to confuse the optimizer with a panic here.
            _ => Precision::Nanoseconds,
        }
    }

    pub fn builder() -> DateTimeBuilder {
        DateTimeBuilder::new()
    }

    pub(super) fn new(
        chronology: Chronology,
        precision: Precision,
        gnd: GregorianNormalizedDate,
        second: u32,
        nanosecond: u32,
    ) -> Self {
        let (w0, w1) = Self::pack(precision, gnd, second, nanosecond);
        DateTime { w0, w1, chronology }
    }

    pub fn year(&self) -> u16 {
        let (_, gnd, _, _) = Self::unpack0(self.w0);
        gnd.unnormalized_year()
            .try_into()
            .expect("year is within the range of u16")
    }

    pub fn month(&self) -> u8 {
        let (_, gnd, _, _) = Self::unpack0(self.w0);
        gnd.unnormalized_month()
    }

    pub fn day(&self) -> u8 {
        let (_, gnd, _, _) = Self::unpack0(self.w0);
        gnd.unnormalized_day()
    }

    pub fn hour(&self) -> u8 {
        let (_, _, second, _) = Self::unpack0(self.w0);
        let hour = (second / SECONDS_PER_HOUR as u32) as u8;
        min(hour, HOURS_PER_DAY - 1)
    }

    pub fn minute(&self) -> u8 {
        let (_, _, second, _) = Self::unpack0(self.w0);
        // The last minute of the day can have more than MINUTES_PER_HOUR seconds, so
        // we need to treat that case separately.
        let boundary = SECONDS_PER_DAY - SECONDS_PER_MINUTE as u32;
        if second < boundary {
            let seconds_into_hour = (second % SECONDS_PER_HOUR as u32) as u16;
            (seconds_into_hour / MINUTES_PER_HOUR as u16) as u8
        } else {
            MINUTES_PER_HOUR - 1
        }
    }

    pub fn second(&self) -> u8 {
        let (_, _, second, _) = Self::unpack0(self.w0);
        let boundary = SECONDS_PER_DAY - SECONDS_PER_MINUTE as u32;
        if second < boundary {
            (second % SECONDS_PER_MINUTE as u32) as u8
        } else {
            (second - boundary) as u8
        }
    }

    // TODO function to transfer as much carry as possible to datetime without
    // overflowing to the next component. E.g. with a second of 58 and a carry of
    // 2, the carry should be reduced to 1 and the second increased to 59.
    // I *think* there's a use case for this but that would need to be figured
    // out as well. Something about chaining multiple operations without
    // "falling behind" the actual time. It might matter when you're e.g.
    // adding a month to something that has fallen behind? I'm not sure.

    pub fn add_years(&self, years: i16) -> DateTimeWithCarry {
        self.checked_add_years(years)
            .expect("adding years to a DateTime resulted in an out-of-bounds value")
    }

    pub fn checked_add_years(&self, years: i16) -> Option<DateTimeWithCarry> {
        let (precision, mut gnd, second, nanosecond) = Self::unpack0(self.w0);
        let day_carry = gnd.add_years(years);
        if !is_in_range(&gnd) {
            return None;
        }
        let (second, seconds_carry) = self.spill_eod_second_overflow(&gnd, second);
        let w0 = Self::pack0(precision, gnd, second, nanosecond);
        let result = DateTime {
            w0,
            w1: self.w1,
            chronology: self.chronology.clone(),
        };
        Some(DateTimeWithCarry(
            result,
            Carry {
                days: day_carry as u32,
                seconds: seconds_carry as u64,
            },
        ))
    }

    pub fn add_months(&self, months: i32) -> DateTimeWithCarry {
        self.checked_add_months(months)
            .expect("adding months to a DateTime resulted in an out-of-bounds value")
    }

    pub fn checked_add_months(&self, months: i32) -> Option<DateTimeWithCarry> {
        let (precision, mut gnd, second, nanosecond) = Self::unpack0(self.w0);
        let day_carry = gnd.add_months(months);
        if !is_in_range(&gnd) {
            return None;
        }
        let (second, seconds_carry) = self.spill_eod_second_overflow(&gnd, second);
        let result = DateTime {
            w0: Self::pack0(precision, gnd, second, nanosecond),
            w1: self.w1,
            chronology: self.chronology.clone(),
        };
        Some(DateTimeWithCarry(
            result,
            Carry {
                days: day_carry as u32,
                seconds: seconds_carry as u64,
            },
        ))
    }

    pub fn add_days(&self, days: i32) -> DateTimeWithCarry {
        self.checked_add_days(days)
            .expect("adding days to a DateTime resulted in an out-of-bounds value")
    }

    pub fn checked_add_days(&self, days: i32) -> Option<DateTimeWithCarry> {
        let (precision, mut gnd, second, nanosecond) = Self::unpack0(self.w0);
        gnd.add_days(days).ok()?;
        if !is_in_range(&gnd) {
            return None;
        }
        let (second, seconds_carry) = self.spill_eod_second_overflow(&gnd, second);
        let result = DateTime {
            w0: Self::pack0(precision, gnd, second, nanosecond),
            w1: self.w1,
            chronology: self.chronology.clone(),
        };
        Some(DateTimeWithCarry(
            result,
            Carry {
                days: 0,
                seconds: seconds_carry as u64,
            },
        ))
    }

    pub fn add_hours(&self, hours: i32) -> DateTimeWithCarry {
        self.checked_add_hours(hours)
            .expect("adding hours to a DateTime resulted in an out-of-bounds value")
    }

    pub fn checked_add_hours(&self, hours: i32) -> Option<DateTimeWithCarry> {
        self.checked_add_minutes(hours as i64 * MINUTES_PER_HOUR as i64)
    }

    pub fn add_minutes(&self, minutes: i64) -> DateTimeWithCarry {
        self.checked_add_minutes(minutes)
            .expect("adding minutes to a DateTime resulted in an out-of-bounds value")
    }

    pub fn checked_add_minutes(&self, minutes: i64) -> Option<DateTimeWithCarry> {
        let (precision, mut gnd, second_of_day, nanosecond) = Self::unpack0(self.w0);

        // We can't easily determine the number of seconds to add since some minutes are
        // irregularly longer or shorter than 60 seconds. But we know that every day has
        // exactly 60*24 minutes. So we can start by splitting into days, minutes and seconds then
        // "seeking" to the right day.
        let (minute, second_of_minute) =
            second_of_day.clamped_div_rem(MINUTES_PER_HOUR as u32, MINUTES_PER_DAY - 1);
        let second_of_minute = second_of_minute as u8;
        let (day_delta, minute) =
            (minute as i64 + minutes).div_mod_floor(&(MINUTES_PER_DAY as i64));
        let (day_delta, minute) = (day_delta as i32, minute as u16);
        gnd.add_days(day_delta).ok()?;
        if !is_in_range(&gnd) {
            return None;
        }

        let (seconds_into_day, seconds_carry) = if minute < MINUTES_PER_DAY - 1 {
            // Not the last minute of the day. We know that it's 60 seconds long.
            if second_of_minute < SECONDS_PER_MINUTE {
                (
                    minute as u32 * SECONDS_PER_MINUTE as u32 + second_of_minute as u32,
                    0,
                )
            } else {
                let max_second = SECONDS_PER_MINUTE - 1;
                let carry = second_of_minute - max_second;
                (
                    minute as u32 * SECONDS_PER_MINUTE as u32 + max_second as u32,
                    carry as u32,
                )
            }
        } else {
            let seconds_into_day =
                minute as u32 * SECONDS_PER_MINUTE as u32 + second_of_minute as u32;
            self.spill_eod_second_overflow(&gnd, seconds_into_day)
        };

        let result = DateTime {
            w0: Self::pack0(precision, gnd, seconds_into_day, nanosecond),
            w1: self.w1,
            chronology: self.chronology.clone(),
        };
        Some(DateTimeWithCarry(
            result,
            Carry {
                days: 0,
                seconds: seconds_carry as u64,
            },
        ))
    }

    pub fn add_seconds(&self, seconds: i64) -> Self {
        self.checked_add_seconds(seconds)
            .expect("adding seconds to a DateTime resulted in overflow")
    }

    pub fn checked_add_seconds(&self, seconds: i64) -> Option<Self> {
        let (precision, gnd, second, nanosecond) = Self::unpack0(self.w0);
        let instant = self.to_second_instant(gnd, second);
        let instant = instant.checked_add(DurationS64::new(seconds))?;
        let (gnd, second) = Self::from_second_instant(instant, &self.chronology)?;
        let w0 = Self::pack0(precision, gnd, second, nanosecond);
        Some(DateTime {
            w0,
            w1: self.w1,
            chronology: self.chronology.clone(),
        })
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_second_instant(&self, gnd: GregorianNormalizedDate, second: u32) -> InstantS64 {
        let leap_second_chronology = self.chronology.leap_seconds();
        let day = gnd.to_day();
        let seconds_since_epoch = match leap_second_chronology.by_day(day) {
            SegmentLookupResult::AfterLast(last_segment) => {
                let days_since_last_segment = day as u32 - last_segment.end_day();
                let seconds_since_last_segment =
                    days_since_last_segment as u64 * SECONDS_PER_DAY as u64 + second as u64;
                let last_segment_end = last_segment.end_instant().ticks_since_epoch() as u64;
                (last_segment_end + seconds_since_last_segment) as i64
            }
            SegmentLookupResult::In(segment) => {
                let days_into_segment = (day - segment.start_day as i32) as u32;
                segment.start_instant.ticks_since_epoch() as i64
                    + days_into_segment as i64 * SECONDS_PER_DAY as i64
                    + second as i64
            }
            SegmentLookupResult::BeforeFirst(first_segment) => {
                let days_to_first_segment = (first_segment.start_day as i32 - (day + 1)) as u32;
                let seconds_to_first_segment = days_to_first_segment as u64
                    * SECONDS_PER_DAY as u64
                    + ((SECONDS_PER_DAY - second) as u64);
                first_segment.start_instant.ticks_since_epoch() as i64
                    - seconds_to_first_segment as i64
            }
        };
        InstantS64::from_ticks_since_epoch(seconds_since_epoch)
    }

    // Returns the GregorianNormalizedDate and second component of the DateTime that corresponds to
    // the given Instant. The second component is the number of seconds since midnight.
    // If date cannot be represented within the range of DateTime, None is returned.
    fn from_second_instant(
        instant: InstantS64,
        chronology: &Chronology,
    ) -> Option<(GregorianNormalizedDate, u32)> {
        // TODO handle leap-second overshoot on the last day of the segment; see add_seconds code.
        // TODO leap-second smearing
        let leap_second_chronology = chronology.leap_seconds();
        match leap_second_chronology.by_instant(instant) {
            SegmentLookupResult::AfterLast(last_segment) => {
                // The instant is past the last known leap-second segment, so we calculate the number
                // of days after the last segment, with each day having exactly 86,400 seconds.
                // TODO this behavior should be parameterized. It's not clear what the correct behavior is.
                // We could
                // - Fail entirely, since it's impossible to do accurately.
                // - Assume no leap seconds after the last known segment, and use 86400 seconds per day.
                // - Calculate the earth's rotation rate and use that to predict decisions by the
                //   IERS.
                // - Use a simple quadratic model to predict leap seconds. Apparently the earth rotation
                //    slows by about 1.8 milliseconds per day
                //    (rspa.royalsocietypublishing.org/content/472/2196/20160404).
                //
                // However, it is likely that the International Telecommunication Union (ITU) will
                // decide to abolish leap seconds in the future. In fact no leap second have been added
                // since 2016. So we might not need to worry about this.
                let seconds_past_segment =
                    (instant - last_segment.end_instant().into()).ticks() as u64;
                let (days, second) = seconds_past_segment.div_rem(&(SECONDS_PER_DAY as u64));
                let second = second as u32;
                let fixed_day = (last_segment.end_day() as u64).checked_add(days)?;
                if fixed_day > MAX_FIXED_DAY as u64 {
                    return None;
                }
                let gnd = GregorianNormalizedDate::from_day(fixed_day as i32).expect(
                    "The number of days since the epoch is within the range of GregorianNormalizedDate",
                );
                Some((gnd, second))
            }
            SegmentLookupResult::In(segment) => {
                let seconds_into_segment = (instant - segment.start_instant.into()).ticks() as u64;
                let (days, second) = seconds_into_segment.div_rem(&(SECONDS_PER_DAY as u64));
                let (mut days, mut second) = (days as u32, second as u32);
                let max_day = segment.duration_days - 1;
                if days > max_day {
                    // Leap seconds at the end of the day caused us to overshoot the last day of the
                    // segment. We need to spill the extra day into the second component.
                    let overshoot_days = days - max_day;
                    assert_eq!(
                        overshoot_days, 1,
                        "More than 86400 leap seconds in one day?"
                    );
                    days = max_day;
                    second += SECONDS_PER_DAY;
                }

                let fixed_day = segment
                    .start_day
                    .checked_add(days)
                    .expect("No leap-second segment is outside of the range of an u32 day");
                let gnd = GregorianNormalizedDate::from_day(fixed_day as i32)
                    .expect("The number of days since the epoch is within the range of GregorianNormalizedDate");
                Some((gnd, second))
            }
            SegmentLookupResult::BeforeFirst(first_segment) => {
                let seconds_until_first_segment =
                    (first_segment.start_instant.into() - instant).ticks() as u64;
                // When seconds_until_first_segment is 1, we want to end up subtracting 1 day and
                // setting the second to 86399. Simply dividing by SECONDS_PER_DAY will give us 0 days.
                // It's essentially a division that rounds up.
                let (mut days, second) =
                    seconds_until_first_segment.div_rem(&(SECONDS_PER_DAY as u64));
                let mut second = second as u32;
                if second != 0 {
                    days += 1;
                    second = SECONDS_PER_DAY - second;
                }
                let fixed_day = first_segment.start_day as i64 - days as i64;
                if fixed_day < MIN_FIXED_DAY as i64 {
                    return None;
                }
                let gnd = GregorianNormalizedDate::from_day(fixed_day as i32)
                    .expect("The number of days since the epoch is within the range of GregorianNormalizedDate");
                Some((gnd, second))
            }
        }
    }

    fn spill_eod_second_overflow(&self, gnd: &GregorianNormalizedDate, second: u32) -> (u32, u32) {
        // TODO can the seconds overflow become extremely large, like thousands of years? If so
        // we need a larger return type here, and probably some kind of fix to the logic.
        let leap_second_chronology = self.chronology.leap_seconds();
        let days_from_epoch = gnd.to_day();
        if let SegmentLookupResult::In(segment) = leap_second_chronology.by_day(days_from_epoch) {
            let day_offset = (days_from_epoch - segment.start_day as i32) as u32;
            let leap_seconds = if day_offset == segment.duration_days {
                segment.leap_seconds
            } else {
                0
            };
            let day_length_s = (SECONDS_PER_DAY as i32 + leap_seconds as i32) as u32;
            if second >= day_length_s {
                let second_carry = second - day_length_s;
                (day_length_s, second_carry)
            } else {
                (second, 0)
            }
        } else {
            (second, 0)
        }
    }
}

fn is_in_range(gnd: &GregorianNormalizedDate) -> bool {
    (MIN_GND..=MAX_GND).contains(gnd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_minute() {
        // Epoch
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert_eq!(date_time.minute(), 0);

        // Minute 59 after the epoch
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(59)
            .second(0)
            .build();
        assert_eq!(date_time.minute(), 59);

        // Last minute of the day during a leap second
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(60)
            .build();
        assert_eq!(date_time.minute(), 59);
    }

    #[test]
    fn test_get_second() {
        // Epoch
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert_eq!(date_time.minute(), 0);

        // Second 59 after the epoch
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(59)
            .build();
        assert_eq!(date_time.second(), 59);

        // Last second of the day during a leap second
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(60)
            .build();
        assert_eq!(date_time.second(), 60);
    }

    #[test]
    fn add_years_to_epoch() {
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        let date_time = date_time.add_years(1).unwrap();
        assert_eq!(date_time.year(), 2001);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        let date_time = date_time.add_years(3).unwrap();
        assert_eq!(date_time.year(), 2003);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        let date_time = date_time.add_years(4).unwrap();
        assert_eq!(date_time.year(), 2004);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);
    }

    #[test]
    fn add_years_to_leap_year() {
        let date_time = DateTime::builder()
            .year(2000)
            .month(2)
            .day(29)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_years(1);
        assert_eq!(date_time.days_carry(), 1);
        assert_eq!(date_time.seconds_carry(), 0);
        let date_time = date_time.drop_carry();
        assert_eq!(date_time.year(), 2001);
        assert_eq!(date_time.month(), 2);
        assert_eq!(date_time.day(), 28);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        let date_time = DateTime::builder()
            .year(2000)
            .month(2)
            .day(29)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_years(1);
        assert_eq!(date_time.days_carry(), 1);
        assert_eq!(date_time.seconds_carry(), 0);

        let date_time = date_time.apply_carry();
        assert_eq!(date_time.year(), 2001);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        let date_time = DateTime::builder()
            .year(2000)
            .month(2)
            .day(29)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_years(4).unwrap();
        assert_eq!(date_time.year(), 2004);
        assert_eq!(date_time.month(), 2);
        assert_eq!(date_time.day(), 29);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);
    }

    #[test]
    fn add_years() {
        let date_time = DateTime::builder()
            .year(2021)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        date_time.add_years(1);
    }

    #[test]
    fn add_years_bounds_checking() {
        // Go to the last year of the range.
        let date_time = DateTime::builder()
            .year(9998)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_years(1).unwrap();
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Try to go beyond the last year of the range.
        let date_time = DateTime::builder()
            .year(9999)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.checked_add_years(1);
        assert!(date_time.is_none());

        // Go to the first year of the range.
        let date_time = DateTime::builder()
            .year(1)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_years(-1).unwrap();
        assert_eq!(date_time.year(), 0);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Try to go beyond the first year of the range.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.checked_add_years(-1);
        assert!(date_time.is_none());
    }

    #[test]
    fn add_months() {
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        // Add a month to epoch time.
        let date_time = date_time.add_months(1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 4);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go back.
        let date_time = date_time.add_months(-1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Add from a month with 31 days to a month with 30 days.
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(31)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_months(1);
        assert_eq!(date_time.days_carry(), 1);
        assert_eq!(date_time.seconds_carry(), 0);
        let date_time = date_time.apply_carry();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 5);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Add from a leap day to a non-leap day
        let date_time = DateTime::builder()
            .year(2000)
            .month(2)
            .day(29)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_months(12);
        assert_eq!(date_time.days_carry(), 1);
        assert_eq!(date_time.seconds_carry(), 0);
        let date_time = date_time.apply_carry();
        assert_eq!(date_time.year(), 2001);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go to the last month of the range of DateTime.
        let date_time = DateTime::builder()
            .year(9999)
            .month(11)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_months(1).unwrap();
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Try to go beyond the last month of the range of DateTime.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_months(1).is_none());

        // Go to the first month of the range of DateTime.
        let date_time = DateTime::builder()
            .year(0)
            .month(2)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_months(-1).unwrap();
        assert_eq!(date_time.year(), 0);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Try to go beyond the first month of the range of DateTime.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_months(-1).is_none());

        // Go from the beginning of the range to the end of the range.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_months(9999 * 12 + 11).unwrap();
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go from the end of the range to the beginning of the range.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_months(-9999 * 12 - 11).unwrap();
        assert_eq!(date_time.year(), 0);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);
    }

    #[test]
    fn add_days() {
        // Add 0 days.
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_days(0).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Add a day to epoch time.
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        let date_time = date_time.add_days(1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 2);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Reverse.
        let date_time = date_time.add_days(-1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Reverse to before epoch time.
        let date_time = date_time.add_days(-1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 2);
        assert_eq!(date_time.day(), 29);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go back forward to epoch time.
        let date_time = date_time.add_days(1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go to the last day of the range of DateTime.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(30)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_days(1).unwrap();
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Try to go beyond the last day of the range of DateTime.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_days(1).is_none());

        // Go to the first day of the range of DateTime.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(2)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_days(-1).unwrap();
        assert_eq!(date_time.year(), 0);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Try to go beyond the first day of the range of DateTime.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_days(-1).is_none());

        // Go from the beginning of the range to the end of the range.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_days(25 * 146097 - 1).unwrap(); // 10000 years = 25 cycles
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Add the maximum number of days to the end of the range.
        let date = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date.checked_add_days(i32::MAX).is_none());

        // Go to the last in-bounds date of the backing GregorianNormalizedDate.
        // This will be out of bounds for the DateTime but should generate a valid
        // GregorianNormalizedDate internally.
        let date = DateTime::builder()
            .year(1970)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date
            .checked_add_days(GregorianNormalizedDate::MAX_FIXED_DAY)
            .is_none());

        // Go out of bounds on the high end of the backing GregorianNormalizedDate.
        let date = DateTime::builder()
            .year(1970)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date
            .checked_add_days(GregorianNormalizedDate::MAX_FIXED_DAY + 1)
            .is_none());
    }

    #[test]
    fn add_hours() {
        // Add 0 hours.
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_hours(0).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        // Add an hour to epoch time.
        let date_time = date_time.add_hours(1).drop_carry();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 1);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go back.
        let date_time = date_time.add_hours(-1).drop_carry();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);

        // Subtract an hour from epoch time.
        let date_time = date_time.add_hours(-1).drop_carry();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 2);
        assert_eq!(date_time.day(), 29);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Add through a leap second.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_hours(1).unwrap();
        assert_eq!(date_time.year(), 1999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Add from a leap second to generate carry.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(60)
            .build();
        let date_time = date_time.add_hours(1);
        assert_eq!(date_time.days_carry(), 0);
        assert_eq!(date_time.seconds_carry(), 1);
        let date_time = date_time.apply_carry();
        assert_eq!(date_time.year(), 1999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 1);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Add hours across multiple days.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_hours(2 * 24).unwrap();
        assert_eq!(date_time.year(), 1999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 2);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Subtract hours across multiple days.
        let date_time = DateTime::builder()
            .year(1999)
            .month(1)
            .day(2)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_hours(-2 * 24).unwrap();
        assert_eq!(date_time.year(), 1998);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Add hours spanning 100 years.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_hours(36525 * 24).unwrap();
        assert_eq!(date_time.year(), 2098);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Go to the last hour of the DateTime range.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(22)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_hours(1).unwrap();
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go out of bounds at the end of the DateTime range.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(23)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_hours(1).is_none());

        // Go to the first hour of the DateTime range.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(1)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_hours(-1).unwrap();
        assert_eq!(date_time.year(), 0);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go out of bounds at the beginning of the DateTime range.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_hours(-1).is_none());
    }

    #[test]
    fn add_minutes() {
        // Add 0 minutes.
        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_minutes(0).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        // Add a minute to epoch time.
        let date_time = date_time.add_minutes(1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 1);
        assert_eq!(date_time.second(), 0);

        // Go back.
        let date_time = date_time.add_minutes(-1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Subtract a minute from epoch time.
        let date_time = date_time.add_minutes(-1).unwrap();
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 2);
        assert_eq!(date_time.day(), 29);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 0);

        // Add through a leap second.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_minutes(1).unwrap();
        assert_eq!(date_time.year(), 1999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 59);

        // Add from a leap second to generate carry.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(60)
            .build();
        let date_time = date_time.add_minutes(1);
        assert_eq!(date_time.days_carry(), 0);
        assert_eq!(date_time.seconds_carry(), 1);
        let date_time = date_time.apply_carry();
        assert_eq!(date_time.year(), 1999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 1);
        assert_eq!(date_time.second(), 0);

        // Add minutes across multiple days.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_minutes(2 * 24 * 60).unwrap();
        assert_eq!(date_time.year(), 1999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 2);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Subtract minutes across multiple days.
        let date_time = DateTime::builder()
            .year(1999)
            .month(1)
            .day(2)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_minutes(-2 * 24 * 60).unwrap();
        assert_eq!(date_time.year(), 1998);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Add minutes spanning 100 years.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_minutes(36525 * 24 * 60).unwrap();
        assert_eq!(date_time.year(), 2098);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Go to the last in-bounds minute at the end of the range of the datetime.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(23)
            .minute(58)
            .second(0)
            .build();
        let date_time = date_time.add_minutes(1).unwrap();
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 0);

        // Go out of bounds at the end of the DateTime range.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(0)
            .build();
        assert!(date_time.checked_add_minutes(1).is_none());

        // Go to the first in-bounds minute at the start of the range of the datetime.
        let date_time = DateTime::builder()
            .year(1)
            .month(1)
            .day(1)
            .hour(0)
            .minute(1)
            .second(0)
            .build();
        let date_time = date_time.add_minutes(-1).unwrap();
        assert_eq!(date_time.year(), 1);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go out of bounds at the start of the DateTime range.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_minutes(-1).is_none());

        // Go to the last minute of the backing GregorianNormalizedDate. This
        // will be out of bounds for the DateTime but should generate a valid
        // GregorianNormalizedDate internally.
        let date_time = DateTime::builder()
            .year(1970)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_minutes(
            (GregorianNormalizedDate::MAX_FIXED_DAY as i64 + 1)*24*60-1).is_none());

        // Go out of bounds on the high end of the backing GregorianNormalizedDate.
        let date_time = DateTime::builder()
            .year(1970)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_minutes(
            (GregorianNormalizedDate::MAX_FIXED_DAY as i64 + 1)*24*60).is_none());
    }

    #[test]
    fn add_seconds() {
        // TODO proptest tests?

        let date_time = DateTime::builder()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        // Add a second to epoch time.
        let date_time = date_time.add_seconds(1);
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 1);

        // Go back.
        let date_time = date_time.add_seconds(-1);
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 3);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Subtract a second from epoch time.
        let date_time = date_time.add_seconds(-1);
        assert_eq!(date_time.year(), 2000);
        assert_eq!(date_time.month(), 2);
        assert_eq!(date_time.day(), 29);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Add into a leap second.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_seconds(1);
        assert_eq!(date_time.year(), 1998);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 60);

        // Add past the leap second.
        let date_time = DateTime::builder()
            .year(1998)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(0)
            .build();
        let date_time = date_time.add_seconds(62);
        assert_eq!(date_time.year(), 1999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 1);

        // Go back to the segment start.
        let date_time = date_time.add_seconds(-1);
        assert_eq!(date_time.year(), 1999);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go back across the leap second.
        let date_time = date_time.add_seconds(-1);
        assert_eq!(date_time.year(), 1998);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 60);

        // Take a greater step backwards across the leap second.
        let date_time = DateTime::builder()
            .year(1999)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(1)
            .build();
        let date_time = date_time.add_seconds(-62);
        assert_eq!(date_time.year(), 1998);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 0);

        // Before first leap segment.
        let date_time = DateTime::builder()
            .year(1969)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        let date_time = date_time.add_seconds(1);
        assert_eq!(date_time.year(), 1969);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 1);

        // Go to the first leap segment.
        let date_time = date_time.add_seconds(SECONDS_PER_DAY as i64 * 365);
        assert_eq!(date_time.year(), 1970);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 1);

        // Go back to before the first leap segment.
        let date_time = date_time.add_seconds(-2);
        assert_eq!(date_time.year(), 1969);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // In the last leap segment
        let date_time = DateTime::builder()
            .year(2016)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(0)
            .build();
        let date_time = date_time.add_seconds(60);
        assert_eq!(date_time.year(), 2016);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 60);

        // Go past the last leap segment.
        let date_time = date_time.add_seconds(1);
        assert_eq!(date_time.year(), 2017);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // From a point past the last leap segment and further.
        let date_time = date_time.add_seconds(SECONDS_PER_DAY as i64 * 365);
        assert_eq!(date_time.year(), 2018);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go to the last in-bounds second at the end of the range of the datetime.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(58)
            .build();
        let date_time = date_time.add_seconds(1);
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Go out of bounds at the end of the range of the datetime.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        assert!(date_time.checked_add_seconds(1).is_none());

        // Go to the first in-bounds second at the start of the range of the datetime.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(1)
            .build();
        let date_time = date_time.add_seconds(-1);
        assert_eq!(date_time.year(), 0);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go out of bounds at the start of the range of the datetime.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_seconds(-1).is_none());

        // Go from the low bound to the high.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        // 3652424 days
        // = 3652424*86400 + 27 = 315569433627 seconds.
        //
        // 315569433627 + 23*3600 + 59*60 + 59 = 315569520026
        let date_time = date_time.add_seconds(315_569_520_026);
        assert_eq!(date_time.year(), 9999);
        assert_eq!(date_time.month(), 12);
        assert_eq!(date_time.day(), 31);
        assert_eq!(date_time.hour(), 23);
        assert_eq!(date_time.minute(), 59);
        assert_eq!(date_time.second(), 59);

        // Go to from the low bound to one second past the high bound.
        let date_time = DateTime::builder()
            .year(0)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert!(date_time.checked_add_seconds(315_569_520_027).is_none());

        // Go from the high bound to the low.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        let date_time = date_time.add_seconds(-315_569_520_026);
        assert_eq!(date_time.year(), 0);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);

        // Go from the high bound to one second before the low bound.
        let date_time = DateTime::builder()
            .year(9999)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(59)
            .build();
        assert!(date_time.checked_add_seconds(-315_569_520_027).is_none());
    }
}
