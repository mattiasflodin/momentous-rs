use crate::duration::DurationS64;
use crate::gregorian_normalized_date::GregorianNormalizedDate;
use crate::instant::InstantS64;
use crate::iso8601::chronology::Chronology;
use crate::iso8601::precision::Precision;
use crate::iso8601::{
    DateTimeBuilder, HOURS_PER_DAY, MINUTES_PER_HOUR, SECONDS_PER_DAY, SECONDS_PER_HOUR,
    SECONDS_PER_MINUTE,
};
use crate::zoneinfo::{get_leap_seconds, SegmentLookupResult};
use num_integer::Integer;
use std::cmp::min;

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
        let carry = self.1.clone();
        let DateTimeWithCarry(result, carry2) = self.0.add_days(carry.days as i32);
        assert_eq!(carry2.days, 0);
        result.add_seconds((carry.seconds + carry2.seconds) as i64)
    }
}

/// An ISO 8601 date and time. The range is from 0000-01-01 to 9999-12-31.
#[derive(Debug, Clone)]
pub struct DateTime {
    chronology: Chronology,
    precision: Precision,
    gnd: GregorianNormalizedDate,
    second: u32,
    nanosecond: u32,
}

impl DateTime {
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
        DateTime {
            chronology,
            precision,
            gnd,
            second,
            nanosecond,
        }
    }

    pub fn year(&self) -> u16 {
        self.gnd.unnormalized_year()
    }

    pub fn month(&self) -> u8 {
        self.gnd.unnormalized_month()
    }

    pub fn day(&self) -> u8 {
        self.gnd.unnormalized_day()
    }

    pub fn hour(&self) -> u8 {
        let hour = (self.second / SECONDS_PER_HOUR as u32) as u8;
        min(hour, HOURS_PER_DAY - 1)
    }

    pub fn minute(&self) -> u8 {
        // The last minute of the day can have more than MINUTES_PER_HOUR seconds, so
        // we need to treat that case separately.
        let boundary = SECONDS_PER_DAY - SECONDS_PER_MINUTE as u32;
        if self.second < boundary {
            let seconds_into_hour = (self.second % SECONDS_PER_HOUR as u32) as u16;
            (seconds_into_hour / MINUTES_PER_HOUR as u16) as u8
        } else {
            MINUTES_PER_HOUR - 1
        }
    }

    pub fn second(&self) -> u8 {
        let boundary = SECONDS_PER_DAY - SECONDS_PER_MINUTE as u32;
        if self.second < boundary {
            (self.second % SECONDS_PER_MINUTE as u32) as u8
        } else {
            (self.second - boundary) as u8
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
        let mut result = (*self).clone();
        let day_carry = result.gnd.add_years(years);
        let days_since_epoch = result.gnd.to_day();
        let seconds_carry = result.spill_seconds_overflow(days_since_epoch);
        DateTimeWithCarry(
            result,
            Carry {
                days: day_carry as u32,
                seconds: seconds_carry as u64,
            },
        )
    }

    pub fn add_months(&self, months: i32) -> DateTimeWithCarry {
        let mut result = (*self).clone();
        let day_carry = result.gnd.add_months(months);
        let days_from_epoch = result.gnd.to_day();
        let seconds_carry = result.spill_seconds_overflow(days_from_epoch);
        DateTimeWithCarry(
            result,
            Carry {
                days: day_carry as u32,
                seconds: seconds_carry as u64,
            },
        )
    }

    pub fn add_days(&self, days: i32) -> DateTimeWithCarry {
        let mut result = (*self).clone();
        result.gnd.add_days(days);
        let days_from_epoch = result.gnd.to_day();
        let seconds_carry = result.spill_seconds_overflow(days_from_epoch);
        DateTimeWithCarry(
            result,
            Carry {
                days: 0,
                seconds: seconds_carry as u64,
            },
        )
    }

    pub fn add_seconds(&self, seconds: i64) -> Self {
        let instant = self.into_second_instant();
        let (gnd, second) = Self::from_second_instant(instant + DurationS64::new(seconds));
        DateTime {
            chronology: self.chronology.clone(),
            precision: self.precision,
            gnd,
            second,
            nanosecond: self.nanosecond,
        }
    }

    pub fn add_minutes(&self, minutes: i128) -> Self {
        todo!()
    }

    #[allow(clippy::wrong_self_convention)]
    fn into_second_instant(&self) -> InstantS64 {
        // TODO store leap seconds reference in chronology object so we don't have to take
        // a lock each time we fetch it, and don't get unpredictable handling of leap seconds.
        // If the leap second table is updated, it should be incorporated into the chronology
        // at a deterministic point, not whenever the table is fetched.

        let leap_second_chronology = get_leap_seconds();
        let day = self.gnd.to_day();
        let seconds_since_epoch = match leap_second_chronology.by_day(day) {
            SegmentLookupResult::AfterLast(last_segment) => {
                let days_since_last_segment = day as u32 - last_segment.end_day();
                let seconds_since_last_segment =
                    days_since_last_segment as u64 * SECONDS_PER_DAY as u64 + self.second as u64;
                let last_segment_end = last_segment.end_instant().ticks_since_epoch() as u64;
                (last_segment_end + seconds_since_last_segment) as i64
            }
            SegmentLookupResult::In(segment) => {
                let days_into_segment = (day - segment.start_day as i32) as u32;
                segment.start_instant.ticks_since_epoch() as i64
                    + days_into_segment as i64 * SECONDS_PER_DAY as i64
                    + self.second as i64
            }
            SegmentLookupResult::BeforeFirst(first_segment) => {
                let days_to_first_segment = (first_segment.start_day as i32 - (day + 1)) as u32;
                let seconds_to_first_segment = days_to_first_segment as u64
                    * SECONDS_PER_DAY as u64
                    + ((SECONDS_PER_DAY - self.second) as u64);
                first_segment.start_instant.ticks_since_epoch() as i64
                    - seconds_to_first_segment as i64
            }
        };
        InstantS64::from_ticks_since_epoch(seconds_since_epoch)
    }

    fn from_second_instant(instant: InstantS64) -> (GregorianNormalizedDate, u32) {
        // TODO handle leap-second overshoot on the last day of the segment; see add_seconds code.
        // TODO leap-second smearing
        let leap_second_chronology = get_leap_seconds();
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
                let (days, second) = (days as u32, second as u32);
                let gnd = GregorianNormalizedDate::from_day((last_segment.end_day() + days) as i32);
                (gnd, second)
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

                let gnd = GregorianNormalizedDate::from_day((segment.start_day + days) as i32);
                (gnd, second)
            }
            SegmentLookupResult::BeforeFirst(first_segment) => {
                let seconds_until_first_segment =
                    (first_segment.start_instant.into() - instant).ticks() as u64;
                // When seconds_until_first_segment is 1, we want to end up subtracting 1 day and
                // setting the second to 86399. Simply dividing by SECONDS_PER_DAY will give us 0 days.
                // It's essentially a division that rounds up.
                let (days, second) = seconds_until_first_segment.div_rem(&(SECONDS_PER_DAY as u64));
                let (mut days, mut second) = (days as u32, second as u32);
                if second != 0 {
                    days += 1;
                    second = SECONDS_PER_DAY - second;
                }
                let gnd =
                    GregorianNormalizedDate::from_day(first_segment.start_day as i32 - days as i32);
                (gnd, second)
            }
        }
    }

    fn spill_seconds_overflow(&mut self, days_from_epoch: i32) -> u32 {
        // TODO can the seconds overflow become extremely large, like thousands of years? If so
        // we need a larger return type here, and probably some kind of fix to the logic.
        let leap_second_chronology = get_leap_seconds();
        if let SegmentLookupResult::In(segment) = leap_second_chronology.by_day(days_from_epoch) {
            let day_offset = (days_from_epoch - segment.start_day as i32) as u32;
            let leap_seconds = if day_offset == segment.duration_days {
                segment.leap_seconds
            } else {
                0
            };
            let day_length_s = (SECONDS_PER_DAY as i32 + leap_seconds as i32) as u32;
            if self.second >= day_length_s {
                let second_carry = self.second - day_length_s;
                self.second = day_length_s;
                second_carry
            } else {
                0
            }
        } else {
            0
        }
    }
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
    }
}
