use crate::cursor::Cursor;
use crate::div_rem::ClampedDivRem;
use crate::gregorian_normalized_date::GregorianNormalizedDate;
use crate::instant::InstantS128;
use crate::iso8601::chronology::Chronology;
use crate::iso8601::precision::Precision;
use crate::iso8601::{
    DateTimeBuilder, HOURS_PER_DAY, MINUTES_PER_HOUR, SECONDS_PER_DAY, SECONDS_PER_HOUR,
    SECONDS_PER_MINUTE,
};
use crate::shared_vec_cursor::SharedVecCursor;
use crate::zoneinfo::{get_leap_seconds, ContinuousTimeSegment};
use num_integer::Integer;
use std::cmp::min;

#[derive(Debug, Clone)]
pub struct Carry {
    days: u128,
    seconds: u128,
}

impl Carry {
    pub fn is_zero(&self) -> bool {
        self.days == 0 && self.seconds == 0
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeWithCarry(DateTime, Carry);

impl DateTimeWithCarry {
    pub(super) fn with_days(date_time: DateTime, days: u128) -> Self {
        DateTimeWithCarry(date_time, Carry { days, seconds: 0 })
    }

    pub(super) fn with_seconds(date_time: DateTime, seconds: u128) -> Self {
        DateTimeWithCarry(date_time, Carry { days: 0, seconds })
    }

    pub fn has_carry(&self) -> bool {
        !self.1.is_zero()
    }

    pub fn days_carry(&self) -> u128 {
        self.1.days
    }

    pub fn seconds_carry(&self) -> u128 {
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
        let DateTimeWithCarry(result, carry2) = self.0.add_days(carry.days as i128);
        assert_eq!(carry2.days, 0);
        result.add_seconds(carry.seconds as i128 + carry2.seconds as i128)
    }
}

#[derive(Debug, Clone)]
pub struct DateTime {
    chronology: Chronology,
    precision: Precision,
    gnd: GregorianNormalizedDate,
    second: u32,
    nanosecond: u32,
    segment_cursor: SharedVecCursor<ContinuousTimeSegment>,
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
        segment_cursor: SharedVecCursor<ContinuousTimeSegment>,
    ) -> Self {
        DateTime {
            chronology,
            precision,
            gnd,
            second,
            nanosecond,
            segment_cursor,
        }
    }

    pub fn year(&self) -> i128 {
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

    pub fn add_days(&self, days: i128) -> DateTimeWithCarry {
        let mut result = (*self).clone();
        result.gnd.add_days(days);
        let days_from_epoch = result.gnd.to_day();
        result.adjust_segment(days_from_epoch);
        let seconds_carry = result.spill_seconds_overflow(days_from_epoch);
        DateTimeWithCarry(
            result,
            Carry {
                days: 0,
                seconds: seconds_carry,
            },
        )
    }

    pub fn add_seconds(&self, seconds: i128) -> Self {
        let mut result = (*self).clone();
        if seconds > 0 {
            result.add_seconds_mut(seconds as u128);
        } else {
            result.subtract_seconds_mut(-seconds as u128);
        }
        result
    }

    pub fn add_minutes(&self, minutes: i128) -> Self {
        todo!()
    }

    fn add_seconds_mut(&mut self, seconds: u128) {
        let mut seconds_remaining = seconds;
        if self.segment_cursor.at_end() {
            // We are beyond the last leap-second segment so adding seconds is trivial.
            let (days, second) =
                (self.second as u128 + seconds_remaining).div_mod_floor(&(SECONDS_PER_DAY as u128));
            self.gnd.add_days(days as i128);
            self.second = second as u32;
            return;
        }

        let mut segment_cursor = self.segment_cursor.clone();
        let mut day = self.gnd.to_day();
        let current_segment = if let Some(segment) = segment_cursor.current() {
            segment
        } else {
            // We are before the first leap-second segment, so we can trivially
            // add seconds until we reach the first segment.
            let first_segment = segment_cursor.next().unwrap();
            let days_to_first_segment = (first_segment.start_day as i128 - (day + 1)) as u128;
            let seconds_to_first_segment = days_to_first_segment * SECONDS_PER_DAY as u128
                + ((SECONDS_PER_DAY - self.second) as u128);
            let seconds_to_add = seconds_remaining.min(seconds_to_first_segment);
            let (days_to_add, second) =
                (self.second as u128 + seconds_to_add).div_mod_floor(&(SECONDS_PER_DAY as u128));
            self.gnd.add_days(days_to_add as i128);
            self.second = second as u32;
            seconds_remaining -= seconds_to_add;
            day += days_to_add as i128;
            first_segment
        };
        if seconds_remaining == 0 {
            // In case we used up all the remaining seconds while trying to reach
            // the first segment above, we can return early and avoid weird corner
            // cases.
            return;
        }

        let leap_second_chronology = get_leap_seconds();

        let days_into_segment = day - current_segment.start_day as i128;
        let ticks = current_segment.start_instant.ticks_since_epoch() as i128
            + days_into_segment * SECONDS_PER_DAY as i128
            + self.second as i128;
        let new_ticks = ticks + seconds_remaining as i128;
        let new_segment_cursor = leap_second_chronology.by_instant_with_hint(
            InstantS128::from_ticks_since_epoch(new_ticks),
            &self.segment_cursor,
        );

        if let Some(new_segment) = new_segment_cursor.current() {
            let seconds_into_segment =
                new_ticks - new_segment.start_instant.ticks_since_epoch() as i128;

            let (mut new_days_into_segment, mut new_second) =
                seconds_into_segment.div_rem(&(SECONDS_PER_DAY as i128));

            let max_day = new_segment.duration_days - 1;
            if new_days_into_segment > max_day as i128 {
                // Leap seconds at the end of the day caused us to overshoot the last day of the
                // segment. We need to spill the extra day into the second component.
                let overshoot_days = new_days_into_segment - max_day as i128;
                assert_eq!(
                    overshoot_days, 1,
                    "More than 86400 leap seconds in one day?"
                );
                new_days_into_segment = max_day as i128;
                new_second += SECONDS_PER_DAY as i128;
            }

            if new_days_into_segment != days_into_segment
                || new_segment_cursor != self.segment_cursor
            {
                self.gnd = GregorianNormalizedDate::from_day(
                    new_segment.start_day as i128 + new_days_into_segment,
                );
            }
            self.second = new_second as u32;
            self.segment_cursor = new_segment_cursor;
        } else {
            // We have gone beyond the last leap-second segment. We only end up here if
            // we were previously in a valid segment, since we would have returned early
            // otherwise.
            let last_segment = new_segment_cursor.peek_prev().unwrap();
            let last_segment_end_tick = last_segment.start_instant.ticks_since_epoch() as i128
                + last_segment.duration_days as i128 * SECONDS_PER_DAY as i128
                + last_segment.leap_seconds as i128;
            let seconds_since_last_segment = new_ticks - last_segment_end_tick;
            let (days, second) = seconds_since_last_segment.div_rem(&(SECONDS_PER_DAY as i128));

            self.gnd = GregorianNormalizedDate::from_day(
                last_segment.start_day as i128 + last_segment.duration_days as i128 + days,
            );
            self.second = second as u32;
            self.segment_cursor = new_segment_cursor;
        }
    }

    fn subtract_seconds_mut(&mut self, seconds: u128) {
        let mut seconds_remaining = seconds;
        if self.segment_cursor.at_start() {
            // We are before the first leap-second segment so subtracting seconds is trivial.
            let (days, second) = self.second.clamped_div_rem(SECONDS_PER_DAY, 0_u32);
            self.gnd.add_days(-(days as i128));
            self.second = second;
            return;
        }

        let mut segment_cursor = self.segment_cursor.clone();
        let mut day = self.gnd.to_day();
        let current_segment = if let Some(segment) = segment_cursor.current() {
            segment
        } else {
            // We are beyond the last leap-second segment, so we can trivially
            // subtract seconds until we reach the last segment.
            let last_segment = segment_cursor.prev().unwrap();
            let last_segment_end_day =
                (last_segment.start_day + last_segment.duration_days) as i128;
            let days_to_last_segment = (day - last_segment_end_day) as u128;
            let seconds_to_last_segment =
                days_to_last_segment * SECONDS_PER_DAY as u128 + self.second as u128;
            let seconds_to_subtract = seconds_remaining.min(seconds_to_last_segment);
            let (days_to_subtract, second) = (self.second as i128 - seconds_to_subtract as i128)
                .div_mod_floor(&(SECONDS_PER_DAY as i128));
            self.gnd.add_days(days_to_subtract);
            self.second = second as u32;
            seconds_remaining -= seconds_to_subtract;
            day -= days_to_subtract;
            last_segment
        };
        if seconds_remaining == 0 {
            // In case we used up all the remaining seconds while trying to reach
            // the last segment above, we can return early and avoid weird corner
            // cases.
            return;
        }

        let leap_second_chronology = get_leap_seconds();

        let days_into_segment = day - current_segment.start_day as i128;
        let ticks = current_segment.start_instant.ticks_since_epoch() as i128
            + days_into_segment * SECONDS_PER_DAY as i128
            + self.second as i128;
        let new_ticks = ticks - seconds_remaining as i128;
        let new_segment_cursor = leap_second_chronology.by_instant_with_hint(
            InstantS128::from_ticks_since_epoch(new_ticks),
            &self.segment_cursor,
        );

        if let Some(new_segment) = new_segment_cursor.current() {
            let seconds_into_segment =
                new_ticks - new_segment.start_instant.ticks_since_epoch() as i128;

            let (mut new_days_into_segment, mut new_second) =
                seconds_into_segment.div_rem(&(SECONDS_PER_DAY as i128));

            let max_day = new_segment.duration_days - 1;
            if new_days_into_segment > max_day as i128 {
                // Leap seconds at the end of the day caused us to overshoot the last day of the
                // segment. We need to spill the extra day into the second component.
                let overshoot_days = new_days_into_segment - max_day as i128;
                assert_eq!(
                    overshoot_days, 1,
                    "More than 86400 leap seconds in one day?"
                );
                new_days_into_segment = max_day as i128;
                new_second += SECONDS_PER_DAY as i128;
            }

            if new_days_into_segment != days_into_segment
                || new_segment_cursor != self.segment_cursor
            {
                self.gnd = GregorianNormalizedDate::from_day(
                    new_segment.start_day as i128 + new_days_into_segment,
                );
            }
            self.second = new_second as u32;
            self.segment_cursor = new_segment_cursor;
        } else {
            // We have gone beyond the first leap-second segment. We only end up here if
            // we were previously in a valid segment, since we would have returned early
            // otherwise.
            let first_segment = new_segment_cursor.peek_next().unwrap();
            let first_segment_start_tick = first_segment.start_instant.ticks_since_epoch() as i128;
            let seconds_until_first_segment = first_segment_start_tick - new_ticks;
            let (mut days, mut second) =
                seconds_until_first_segment.div_rem(&(SECONDS_PER_DAY as i128));

            if second != 0 {
                // We have the distance to the upcoming segment in days and
                // seconds, but this needs to be converted into a "rounded down"
                // (toward negative) day and the seconds into the day.
                days += 1;
                second = SECONDS_PER_DAY as i128 - second;
            }

            self.gnd = GregorianNormalizedDate::from_day(first_segment.start_day as i128 - days);
            self.second = second as u32;
            self.segment_cursor = new_segment_cursor;
        }
    }

    fn adjust_segment(&mut self, day: i128) {
        if let Some(segment) = self.segment_cursor.current() {
            if day >= segment.start_day as i128
                && day < (segment.start_day + segment.duration_days) as i128
            {
                return;
            }
        } else if self.segment_cursor.at_start() {
            let next_segment = self.segment_cursor.peek_next().unwrap();
            if day < next_segment.start_day as i128 {
                return;
            }
        } else {
            assert!(self.segment_cursor.at_end());
            let prev_segment = self.segment_cursor.peek_prev().unwrap();
            if day >= (prev_segment.start_day + prev_segment.duration_days) as i128 {
                return;
            }
        }

        // We are no longer in the same segment, so we need to find the new current one.
        self.segment_cursor = get_leap_seconds().by_day(day);
    }

    fn spill_seconds_overflow(&mut self, days_from_epoch: i128) -> u128 {
        if let Some(segment) = self.segment_cursor.current() {
            let day_offset = (days_from_epoch - segment.start_day as i128) as u32;
            let leap_seconds = if day_offset == segment.duration_days {
                segment.leap_seconds
            } else {
                0
            };
            let day_length_s = (86_400i32 + leap_seconds as i32) as u32;
            if self.second >= day_length_s {
                let second_carry = self.second - day_length_s;
                self.second = day_length_s;
                second_carry as u128
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
    fn add_seconds() {
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
        let date_time = date_time.add_seconds(SECONDS_PER_DAY as i128 * 365);
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
        let date_time = date_time.add_seconds(SECONDS_PER_DAY as i128 * 365);
        assert_eq!(date_time.year(), 2018);
        assert_eq!(date_time.month(), 1);
        assert_eq!(date_time.day(), 1);
        assert_eq!(date_time.hour(), 0);
        assert_eq!(date_time.minute(), 0);
        assert_eq!(date_time.second(), 0);
    }
}
