use std::cmp::min;

use num_integer::Integer;
use numcmp::NumCmp;

use crate::cursor::Cursor;
use crate::gregorian_normalized_date::GregorianNormalizedDate;
use crate::instant::Tick;
use crate::iso8601::date_time_with_carry::DateTimeWithCarry;
use crate::iso8601::util::days_in_month;
use crate::iso8601::{Chronology, DateTimeBuilder, Precision, SECONDS_PER_DAY};
use crate::widen::Widen;
use crate::zoneinfo::get_leap_seconds;
use crate::InstantOutOfRange;
use crate::{Instant, Scale};

#[derive(Debug, Clone)]
pub struct DateTime {
    pub(super) chronology: Chronology,
    pub(super) precision: Precision,
    pub(super) year: i128,
    pub(super) month: u8,
    pub(super) day: u8,
    pub(super) hour: u8,
    pub(super) minute: u8,
    pub(super) second: u8,
    pub(super) millisecond: u16,
    pub(super) microsecond: u16,
    pub(super) nanosecond: u16,
    pub(super) offset_hour: u8,
    pub(super) offset_minute: u8,
}

impl DateTime {
    pub fn builder() -> DateTimeBuilder {
        DateTimeBuilder::new()
    }

    fn from_instant<T, S>(
        instant: Instant<T, S>,
        chronology: &Chronology,
    ) -> Result<Self, InstantOutOfRange>
    where
        T: Tick + NumCmp<i32> + TryInto<i64>,
        S: Scale,
        i32: Widen<T>,
        i64: Widen<T>,
    {
        let datetime = chronology.get_date_time(instant);
        Ok(datetime)
    }

    fn truncate(&self, precision: Precision) -> Self {
        let nanosecond = if precision < Precision::Nanoseconds {
            0
        } else {
            self.nanosecond
        };
        let microsecond = if precision < Precision::Microseconds {
            0
        } else {
            self.microsecond
        };
        let millisecond = if precision < Precision::Milliseconds {
            0
        } else {
            self.millisecond
        };
        let second = if precision < Precision::Seconds {
            0
        } else {
            self.second
        };
        let minute = if precision < Precision::Minutes {
            0
        } else {
            self.minute
        };
        let hour = if precision < Precision::Hours {
            0
        } else {
            self.hour
        };
        let day = if precision < Precision::Days {
            0
        } else {
            self.day
        };
        if precision == Precision::Weeks {
            todo!()
        }
        let month = if precision < Precision::Months {
            0
        } else {
            self.month
        };
        let year = if precision == Precision::Decades {
            Integer::div_floor(&self.year, &10) * 10
        } else if precision == Precision::Centuries {
            Integer::div_floor(&self.year, &100) * 100
        } else if precision == Precision::Millennia {
            Integer::div_floor(&self.year, &1000) * 1000
        } else {
            self.year
        };
        // TODO what do we do with the offset?
        DateTime {
            chronology: self.chronology.clone(),
            precision,
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
            microsecond,
            nanosecond,
            offset_hour: self.offset_hour,
            offset_minute: self.offset_minute,
        }
    }

    pub fn add_years(&self, years: i128) -> DateTimeWithCarry {
        assert!(
            self.precision >= Precision::Years,
            "Cannot add years to a datetime with precision less than years"
        );
        let mut result = self.clone();
        result.add_years_mut(years);

        todo!()
    }

    pub fn add_months(&self, months: i128) -> DateTimeWithCarry {
        assert!(
            self.precision >= Precision::Months,
            "Cannot add months to a datetime with precision less than months"
        );

        let mut result = self.clone();
        result.add_months_mut(months);

        let carry_days = if self.precision >= Precision::Days {
            let days_in_month = days_in_month(result.year, result.month);
            if result.day > days_in_month {
                let carry = result.day - days_in_month;
                result.day = days_in_month;
                carry
            } else {
                0
            }
        } else {
            0
        };

        DateTimeWithCarry::with_days(result, carry_days as u128)
    }

    pub fn add_days(&self, days: i128) -> DateTimeWithCarry {
        assert!(
            self.precision >= Precision::Days,
            "Cannot add days to a datetime with precision less than days"
        );
        let mut result = self.clone();
        result.add_days_mut(days);

        todo!("leap second carry")
    }

    fn add_hours_mut(&mut self, hours: i128) {
        let (days, hour) = (self.hour as i128 + hours).div_mod_floor(&24);
        self.hour = hour as u8;

        self.add_days_mut(days);
    }

    pub fn add_minutes(&self, minutes: i128) -> DateTimeWithCarry {
        assert!(
            self.precision >= Precision::Minutes,
            "Cannot add minutes to a datetime with precision less than minutes"
        );
        let mut result = self.clone();
        result.add_minutes_mut(minutes);

        let carry = if self.precision >= Precision::Seconds {
            let gnd = GregorianNormalizedDate::from_date(result.year, result.month, result.day);
            let day = gnd.to_day();
            let leap_second_chronology = get_leap_seconds();
            let segment_cursor = leap_second_chronology.by_day(day);
            let max_second = if let Some(segment) = segment_cursor.current() {
                let segment_end_day = segment.start_day + segment.duration_days;
                if day == segment_end_day as i128 && result.hour == 23 && result.minute == 59 {
                    (59 + segment.leap_seconds) as u8
                } else {
                    59
                }
            } else {
                59
            };

            if result.second > max_second {
                let carry = result.second - max_second;
                result.second = max_second;
                carry
            } else {
                0
            }
        } else {
            0
        };

        DateTimeWithCarry::with_seconds(result, carry as u128)
    }

    pub fn add_seconds(&self, seconds: i128) -> DateTime {
        assert!(
            self.precision >= Precision::Seconds,
            "Cannot add seconds to a datetime with precision less than seconds"
        );
        if self.chronology.leap_second_smearing() {
            todo!("Leap-second smearing not yet implemented")
        } else {
            let mut result = self.clone();
            if seconds >= 0 {
                result.add_seconds_mut(seconds as u128);
            } else {
                result.subtract_seconds_mut((-seconds) as u128);
            }
            result
        }
    }

    fn add_years_mut(&mut self, years: i128) {
        self.year += years;
    }

    fn add_months_mut(&mut self, months: i128) {
        let (years, month) = ((self.month - 1) as i128 + months).div_mod_floor(&12);
        self.month = month as u8 + 1;
        self.add_years_mut(years);
    }

    fn add_days_mut(&mut self, days: i128) {
        let gnd = GregorianNormalizedDate::from_date(self.year, self.month, self.day);
        let gnd = GregorianNormalizedDate::from_day(gnd.to_day() + days);
        let (year, month, day) = gnd.to_date();
        self.year = year;
        self.month = month;
        self.day = day;
    }

    fn add_minutes_mut(&mut self, minutes: i128) {
        // TODO there are two alternative implementation strategies for these functions:
        // 1. Perform arithmetic within the calendar system, taking into account every change
        //    in calendar system etc. This is essentially what the operation "means": when adding
        //    seconds, for example, we're not adding SI seconds but rather seconds as defined by
        //    the calendar system. This can also be simple because many relationships are relatively
        //    stable: an hour is always 60 minutes.
        // 2. Convert to nanoseconds, add to the instant, and convert back. This requires an
        //    understanding of how the length of every unit above a second changes due to leap
        //    seconds, and also requires taking the length of days into account etc.
        //
        // We're assuming that the first strategy is better (mainly because it captures semantics
        // more directly), but we should try the second strategy as well later to see how the they
        // compare in terms of code complexity and performance.

        let (hours, minute) = (self.minute as i128 + minutes).div_mod_floor(&60);
        self.minute = minute as u8;
        self.add_hours_mut(hours);
    }

    fn add_seconds_mut(&mut self, seconds: u128) {
        let gnd = GregorianNormalizedDate::from_date(self.year, self.month, self.day);
        let day = gnd.to_day();
        let leap_seconds = get_leap_seconds();
        let mut segment_cursor = leap_seconds.by_day(day);

        let mut seconds_remaining = seconds;

        // How far into the segment is the current time? Counting from the start of the segment.
        let mut segment_offset_seconds = if segment_cursor.at_start() {
            // We're before the first segment, so we can just add seconds without dealing with leap
            // seconds until we reach the first segment.
            let segment = segment_cursor.peek_next().unwrap();
            let days_before_first_segment = segment.start_day as i128 - day;
            let seconds_before_first_segment = days_before_first_segment * SECONDS_PER_DAY as i128
                - self.hour as i128 * 60 * 60
                - self.minute as i128 * 60
                - self.second as i128;
            let add_seconds = min(seconds_remaining, seconds_before_first_segment as u128);
            self.add_seconds_within_segment(add_seconds as i128);
            seconds_remaining -= add_seconds;
            if seconds_remaining == 0 {
                // Since we possibly didn't reach the first segment, we'll be in a bad state
                // if we continue with that segment below. So just finish here.
                return;
            }
            0
        } else if segment_cursor.at_end() {
            // We're after the last segment, so no need to compute a segment offset - we can perform
            // a simple addition of seconds without caring about leap seconds.
            self.add_seconds_within_segment(seconds as i128);
            return;
        } else {
            let segment = segment_cursor.current().unwrap();
            let days_into_segment = day - segment.start_day as i128;
            let seconds_into_segment = days_into_segment as u128 * SECONDS_PER_DAY as u128
                + self.hour as u128 * 60 * 60
                + self.minute as u128 * 60
                + self.second as u128;
            // Since the loop starts by advancing the cursor, we need to retreat it by one to get
            // the correct segment in the first iteration.
            let _ = segment_cursor.prev();
            seconds_into_segment
        };

        while let Some(segment) = segment_cursor.next() {
            let regular_seconds_in_segment =
                segment.duration_days as u128 * SECONDS_PER_DAY as u128;
            if segment_offset_seconds < regular_seconds_in_segment - 1 {
                // We're in the regular part of the segment. Add seconds using modular arithmetic.
                // We need -1 above because the last second would wrap around to 00:00:00 and we
                // don't want that to happen.
                let ordinary_seconds_remaining =
                    regular_seconds_in_segment - segment_offset_seconds - 1;
                let add_seconds = min(seconds_remaining, ordinary_seconds_remaining);
                self.add_seconds_within_segment(add_seconds as i128);
                seconds_remaining -= add_seconds;
                segment_offset_seconds += add_seconds;
            }

            if seconds_remaining == 0 {
                // We need to bail out because if we end up at a leap second above, even adding
                // zero seconds may cause a second value that is >= 60 to wrap around to 0.
                break;
            }

            // Add the remaining (leap) seconds in the segment using normal arithmetic.
            let total_seconds_in_segment =
                regular_seconds_in_segment + segment.leap_seconds as u128;
            let leap_seconds_remaining = total_seconds_in_segment - segment_offset_seconds;
            if seconds_remaining >= leap_seconds_remaining {
                // More seconds to add than there are leap seconds in the segment, i.e. we will
                // reach the next segment. Simply move to the next day and set the time to the
                // start of the day.
                self.hour = 0;
                self.minute = 0;
                self.second = 0;
                self.add_days_mut(1);
                seconds_remaining -= leap_seconds_remaining;
            } else {
                // Not enough seconds remaining to leave the segment. Add the remaining seconds
                // using regular arithmetic.
                self.second += seconds_remaining as u8;
                return;
            }
        }

        if seconds_remaining != 0 {
            // If there are still seconds remaining at this point then we passed the last segment,
            // so just add the remaining seconds as if there are no more leap seconds.
            self.add_seconds_within_segment(seconds_remaining as i128);
        }
    }

    fn subtract_seconds_mut(&mut self, seconds: u128) {
        let gnd = GregorianNormalizedDate::from_date(self.year, self.month, self.day);
        let mut day = gnd.to_day();
        let leap_seconds = get_leap_seconds();
        let mut segment_cursor = leap_seconds.by_day(day);

        let mut seconds_remaining = seconds;

        // We want to treat a time precisely at the boundary between two segments as being in the
        // segment that ends at that time. That way, if we're at 00:00:00 and subtracting one
        // second, we're in the segment that *ends* at 00:00:00 and can directly use the leap
        // seconds from that segment in the calculations. However, for the arithmetic
        // to work out later on we also need to transform 00:00:00 to 23:59:X where x represents
        // the second that never occurs in the segment. This will temporarily screw up
        // the validity of the time, so it needs to be corrected later on.
        if !segment_cursor.at_start() {
            let segment_start_day = if segment_cursor.at_end() {
                let prev_segment = segment_cursor.peek_prev().unwrap();
                prev_segment.start_day + prev_segment.duration_days
            } else {
                segment_cursor.current().unwrap().start_day
            };
            // Note that we don't care if sub-second parts are non-zero, since this entire operation
            // will leave those parts unaffected.
            if segment_start_day as i128 == day
                && self.hour == 0
                && self.minute == 0
                && self.second == 0
            {
                day -= 1;
                self.add_days_mut(-1);
                self.hour = 23;
                self.minute = 59;
                self.second = (60
                    + if let Some(prev_segment) = segment_cursor.prev() {
                        prev_segment.leap_seconds
                    } else {
                        0
                    }) as u8;
            }
        }

        // How far into the segment is the current time? Counting from the end of the segment.
        let mut segment_offset_seconds = if segment_cursor.at_end() {
            // We're after the end of the last segment, so we can just subtract seconds
            // without dealing with leap seconds until we reach the last segment.
            let segment = segment_cursor.peek_prev().unwrap();
            let last_segment_end = segment.start_day + segment.duration_days;
            let days_past_last_segment = day - last_segment_end as i128;
            // -1 because when we're at 00:00:00 we consider it to be in the previous segment,
            // so the "virtual segment" that is after the last segment starts at 00:00:01.
            // Subtracting the number of seconds calculated here will leave us at 00:00:00 which
            // is where we want to be.
            let seconds_past_last_segment = days_past_last_segment as u128
                * SECONDS_PER_DAY as u128
                + self.hour as u128 * 60 * 60
                + self.minute as u128 * 60
                + self.second as u128
                - 1;
            let subtract_seconds = min(seconds_remaining, seconds_past_last_segment);
            self.add_seconds_within_segment(-(subtract_seconds as i128));
            seconds_remaining -= subtract_seconds;
            0
        } else if segment_cursor.at_start() {
            // We're before the first segment, so no need to compute a segment offset as the segment
            // loop will be skipped. We'll just fall through to the trailing subtraction below
            // the segment loop.
            0
        } else {
            let segment = segment_cursor.current().unwrap();
            let days_into_segment = day - segment.start_day as i128;
            let seconds_into_segment = days_into_segment as u128 * SECONDS_PER_DAY as u128
                + self.hour as u128 * 60 * 60
                + self.minute as u128 * 60
                + self.second as u128;
            let segment_duration_seconds = segment.duration_days as u128 * SECONDS_PER_DAY as u128
                + segment.leap_seconds as u128;
            // Since the loop starts by retreating the cursor, we need to advance it to get
            // the correct segment in the first iteration.
            let _ = segment_cursor.next();
            segment_duration_seconds - seconds_into_segment
        };

        while let Some(segment) = segment_cursor.prev() {
            let leap_seconds_in_segment = segment.leap_seconds as u128;

            if leap_seconds_in_segment >= segment_offset_seconds {
                // We're in the leap second part of the segment. Subtract seconds using regular
                // arithmetic. We add +1 to the leap seconds remaining because (assuming a segment
                // that ends in one leap second) when the time is 23:59:60 the offset will be 1
                // which would lead to a difference of between the segment offset and the total
                // leap seconds of zero, and nothing would be subtracted here. But we want to use
                // regular arithmetic since add_seconds_within_segment (the modular addition)
                // is not meant to be used with a second value of 60.
                let leap_seconds_remaining = leap_seconds_in_segment - segment_offset_seconds;
                let subtract_seconds = min(seconds_remaining, leap_seconds_remaining + 1);
                self.second -= subtract_seconds as u8;
                seconds_remaining -= subtract_seconds;
                segment_offset_seconds += subtract_seconds;
            }

            if seconds_remaining == 0 {
                // We need to bail out because if we end up at a leap second above, even subtracting
                // zero seconds will cause a second value that is >= 60 to wrap around to 0.
                break;
            }

            // Subtract the remaining seconds in the segment using modular arithmetic.
            let non_leap_seconds_in_segment =
                segment.duration_days as u128 * SECONDS_PER_DAY as u128;
            let total_seconds_in_segment = non_leap_seconds_in_segment + leap_seconds_in_segment;
            let seconds_remaining_in_segment = total_seconds_in_segment - segment_offset_seconds;
            let subtract_seconds = min(seconds_remaining, seconds_remaining_in_segment);
            self.add_seconds_within_segment(-(subtract_seconds as i128));
            seconds_remaining -= subtract_seconds;
            if seconds_remaining == 0 {
                break;
            }
            segment_offset_seconds = 0;
            // Map DATE 00:00:00 to DATE-1 23:59:X where X is the first second that never occurs
            // in the segment, in order to ensure that the next iteration starts at the correct
            // position in the segment.
            debug_assert_eq!(self.hour, 0);
            debug_assert_eq!(self.minute, 0);
            debug_assert_eq!(self.second, 0);
            self.add_days_mut(-1);
            self.hour = 23;
            self.minute = 59;
            self.second = (60
                + if let Some(prev_segment) = segment_cursor.peek_prev() {
                    prev_segment.leap_seconds
                } else {
                    0
                }) as u8;
        }

        if seconds_remaining != 0 {
            // If there are still seconds remaining at this point then we passed the first segment,
            // so just subtract the remaining seconds. Note that we may be left with a second
            // part that is >= 60 due to treating segment ranges as inclusive above. Since
            // add_seconds_within_segment uses modular arithmetic it doesn't support that, so we
            // need to handle that case first.
            if self.second >= 60 {
                let subtract_seconds = min(seconds_remaining, self.second as u128 - 59);
                self.second -= subtract_seconds as u8;
                seconds_remaining -= subtract_seconds;
            }
            self.add_seconds_within_segment(-(seconds_remaining as i128));
        }
    }

    fn add_seconds_within_segment(&mut self, seconds: i128) {
        assert!(self.second < 60);
        let (minutes, second) = (self.second as i128 + seconds).div_mod_floor(&60);
        self.second = second as u8;

        self.add_minutes_mut(minutes);
    }
}

#[cfg(test)]
mod tests {
    use crate::iso8601::chronology::load_chronology;
    use crate::iso8601::precision::Precision;
    use crate::iso8601::DateTime;
    use crate::InstantNs128;

    #[test]
    fn from_instant() {
        let chronology = load_chronology("UTC");

        // 2000-03-01 is the first day of the first quadrennium of the first century of the first cycle,
        // i.e. the zero point used in the implementation of GregorianNormalizedDate which is used in
        // the implementation of from_instant.
        let date_time = DateTime::from_instant(
            InstantNs128::new((11017 * 24 * 60 * 60 + 22) * 1_000_000_000),
            &chronology,
        )
        .unwrap();
        assert_eq!(date_time.year, 2000);
        assert_eq!(date_time.month, 3);
        assert_eq!(date_time.day, 1);
        assert_eq!(date_time.hour, 0);
        assert_eq!(date_time.minute, 0);
        assert_eq!(date_time.second, 0);
        assert_eq!(date_time.millisecond, 0);
        assert_eq!(date_time.microsecond, 0);
        assert_eq!(date_time.nanosecond, 0);

        // One nanosecond earlier than above.
        let date_time = DateTime::from_instant(
            InstantNs128::new((11017 * 24 * 60 * 60 + 22) * 1_000_000_000 - 1),
            &chronology,
        )
        .unwrap();
        assert_eq!(date_time.year, 2000);
        assert_eq!(date_time.month, 2);
        assert_eq!(date_time.day, 29);
        assert_eq!(date_time.hour, 23);
        assert_eq!(date_time.minute, 59);
        assert_eq!(date_time.second, 59);
        assert_eq!(date_time.millisecond, 999);
        assert_eq!(date_time.microsecond, 999);
        assert_eq!(date_time.nanosecond, 999);

        // Unix epoch.
        let date_time = DateTime::from_instant(InstantNs128::new(0), &chronology).unwrap();
        assert_eq!(date_time.year, 1970);
        assert_eq!(date_time.month, 1);
        assert_eq!(date_time.day, 1);
        assert_eq!(date_time.hour, 0);
        assert_eq!(date_time.minute, 0);
        assert_eq!(date_time.second, 0);
        assert_eq!(date_time.millisecond, 0);
        assert_eq!(date_time.microsecond, 0);
        assert_eq!(date_time.nanosecond, 0);

        // One nanosecond before unix epoch.
        let date_time = DateTime::from_instant(InstantNs128::new(-1), &chronology).unwrap();
        assert_eq!(date_time.year, 1969);
        assert_eq!(date_time.month, 12);
        assert_eq!(date_time.day, 31);
        assert_eq!(date_time.hour, 23);
        assert_eq!(date_time.minute, 59);
        assert_eq!(date_time.second, 59);
        assert_eq!(date_time.millisecond, 999);
        assert_eq!(date_time.microsecond, 999);
        assert_eq!(date_time.nanosecond, 999);

        // Introduction of the Gregorian calendar. 141427 days before unix epoch.
        let date_time = DateTime::from_instant(
            InstantNs128::new(-141427 * 24 * 60 * 60 * 1_000_000_000),
            &chronology,
        )
        .unwrap();
        assert_eq!(date_time.year, 1582);
        assert_eq!(date_time.month, 10);
        assert_eq!(date_time.day, 15);
        assert_eq!(date_time.hour, 0);
        assert_eq!(date_time.minute, 0);
        assert_eq!(date_time.second, 0);
        assert_eq!(date_time.millisecond, 0);
        assert_eq!(date_time.microsecond, 0);
        assert_eq!(date_time.nanosecond, 0);

        // Second before 1990 leap second
        let date_time = DateTime::from_instant(
            InstantNs128::new((7670 * 24 * 60 * 60 + 14) * 1_000_000_000),
            &chronology,
        )
        .unwrap();
        assert_eq!(date_time.year, 1990);
        assert_eq!(date_time.month, 12);
        assert_eq!(date_time.day, 31);
        assert_eq!(date_time.hour, 23);
        assert_eq!(date_time.minute, 59);
        assert_eq!(date_time.second, 59);
        assert_eq!(date_time.millisecond, 0);
        assert_eq!(date_time.microsecond, 0);
        assert_eq!(date_time.nanosecond, 0);

        // 1990 leap second.
        let date_time = DateTime::from_instant(
            InstantNs128::new((7670 * 24 * 60 * 60 + 15) * 1_000_000_000),
            &chronology,
        )
        .unwrap();
        assert_eq!(date_time.year, 1990);
        assert_eq!(date_time.month, 12);
        assert_eq!(date_time.day, 31);
        assert_eq!(date_time.hour, 23);
        assert_eq!(date_time.minute, 59);
        assert_eq!(date_time.second, 60);
        assert_eq!(date_time.millisecond, 0);
        assert_eq!(date_time.microsecond, 0);
        assert_eq!(date_time.nanosecond, 0);

        // Second after 1990 leap second.
        let date_time = DateTime::from_instant(
            InstantNs128::new((7670 * 24 * 60 * 60 + 16) * 1_000_000_000),
            &chronology,
        )
        .unwrap();
        assert_eq!(date_time.year, 1991);
        assert_eq!(date_time.month, 1);
        assert_eq!(date_time.day, 1);
        assert_eq!(date_time.hour, 0);
        assert_eq!(date_time.minute, 0);
        assert_eq!(date_time.second, 0);
        assert_eq!(date_time.millisecond, 0);
        assert_eq!(date_time.microsecond, 0);
        assert_eq!(date_time.nanosecond, 0);
    }

    #[test]
    #[should_panic]
    fn incomplete_build() {
        let _ = DateTime::builder()
            .year(2015)
            // missing month
            .day(15)
            .hour(17)
            .minute(30)
            .second(45)
            .build();
    }

    #[test]
    fn truncate() {
        let source_datetime = DateTime::builder()
            .year(2015)
            .month(4)
            .day(15)
            .hour(17)
            .minute(30)
            .second(45)
            .millisecond(123)
            .build();
        let datetime = source_datetime.clone();
        assert_eq!(datetime.precision, Precision::Milliseconds);
        assert_eq!(datetime.year, 2015);
        assert_eq!(datetime.month, 4);
        assert_eq!(datetime.day, 15);
        assert_eq!(datetime.hour, 17);
        assert_eq!(datetime.minute, 30);
        assert_eq!(datetime.second, 45);
        assert_eq!(datetime.millisecond, 123);

        let datetime = source_datetime.clone().truncate(Precision::Seconds);
        assert_eq!(datetime.precision, Precision::Seconds);
        assert_eq!(datetime.year, 2015);
        assert_eq!(datetime.month, 4);
        assert_eq!(datetime.day, 15);
        assert_eq!(datetime.hour, 17);
        assert_eq!(datetime.minute, 30);
        assert_eq!(datetime.second, 45);

        let datetime = source_datetime.clone().truncate(Precision::Months);
        assert_eq!(datetime.precision, Precision::Months);
        assert_eq!(datetime.year, 2015);
        assert_eq!(datetime.month, 4);

        let datetime = source_datetime.clone().truncate(Precision::Years);
        assert_eq!(datetime.precision, Precision::Years);
        assert_eq!(datetime.year, 2015);

        let datetime = source_datetime.clone().truncate(Precision::Decades);
        assert_eq!(datetime.precision, Precision::Decades);
        assert_eq!(datetime.year, 2010);

        let datetime = source_datetime.clone().truncate(Precision::Centuries);
        assert_eq!(datetime.precision, Precision::Centuries);
        assert_eq!(datetime.year, 2000);

        let datetime = source_datetime.clone().truncate(Precision::Millennia);
        assert_eq!(datetime.precision, Precision::Millennia);
        assert_eq!(datetime.year, 2000);
    }

    #[test]
    fn add_subtract_seconds() {
        // Two seconds before 1990 leap second
        let datetime = DateTime::builder()
            .year(1990)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(58)
            .build();

        // Second before 1990 leap second
        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);

        // 1990 leap second.
        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 60);

        // Second after 1990 leap second.
        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1991);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 0);

        // Backwards again.
        let datetime = datetime.add_seconds(-1);
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 60);

        let datetime = datetime.add_seconds(-1);
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);

        // Add multiple seconds at once and make sure the leap second still gets counted.
        let datetime = datetime.add_seconds(3);
        assert_eq!(datetime.year, 1991);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 1);

        // And back again
        let datetime = datetime.add_seconds(-3);
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);
    }

    #[test]
    fn add_subtract_seconds_open_ends() {
        // This tests the open ends of the list of leap-second segments, i.e. where leap-second
        // counting started in the 1970s and where it ends in 2016.
        // TODO Because additional leap seconds can be added in the future, this test may
        // become obsolete. We should add a way to create artificial leap-second chronologies
        // for testing. This would also allow us to test negative leap seconds and multiple
        // leap seconds in a single segment.

        // The first leap second was added on 1972-06-30, but our first leap-second segment starts
        // at Unix epoch and ends with that date.
        let datetime = DateTime::builder()
            .year(1969)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(58)
            .build();

        // Go forward across the segment boundary.
        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1969);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);

        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1970);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 0);

        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1970);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 1);

        // Back up again.
        let datetime = datetime.add_seconds(-1);
        assert_eq!(datetime.year, 1970);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 0);

        let datetime = datetime.add_seconds(-1);
        assert_eq!(datetime.year, 1969);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);

        // Add multiple seconds at once and make sure we end up at the right place,
        // while also placing us near the first leap second. It's 912 days between
        // 1969-12-31 and 1972-06-30, but we want to end up one second earlier (:58)
        // so we add 912*24*60*60 - 1 seconds.
        let datetime = datetime.add_seconds(912 * 24 * 60 * 60 - 1);
        assert_eq!(datetime.year, 1972);
        assert_eq!(datetime.month, 6);
        assert_eq!(datetime.day, 30);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 58);

        // Go forward across the segment boundary, which has one leap second.
        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1972);
        assert_eq!(datetime.month, 6);
        assert_eq!(datetime.day, 30);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);

        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1972);
        assert_eq!(datetime.month, 6);
        assert_eq!(datetime.day, 30);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 60);

        let datetime = datetime.add_seconds(1);
        assert_eq!(datetime.year, 1972);
        assert_eq!(datetime.month, 7);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 0);

        // Back up again.
        let datetime = datetime.add_seconds(-1);
        assert_eq!(datetime.year, 1972);
        assert_eq!(datetime.month, 6);
        assert_eq!(datetime.day, 30);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 60);

        let datetime = datetime.add_seconds(-1);
        assert_eq!(datetime.year, 1972);
        assert_eq!(datetime.month, 6);
        assert_eq!(datetime.day, 30);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);
    }

    #[test]
    fn add_minutes() {
        // Two minutes before 1990 leap second
        let datetime = DateTime::builder()
            .year(1990)
            .month(12)
            .day(31)
            .hour(23)
            .minute(58)
            .second(59)
            .build();

        // Minute before 1990 leap second
        let datetime = datetime.add_minutes(1).unwrap();
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);

        // Check that date increases correctly when adding another minute (and second is preserved despite
        // the minute having 61 seconds).
        let datetime = datetime.add_minutes(1).unwrap();
        assert_eq!(datetime.year, 1991);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 59);

        // Back again.
        let datetime = datetime.add_minutes(-1).unwrap();
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);

        // Now move to the leap second and verify that we get one-second carry when adding a minute
        let datetime = datetime.add_seconds(1);
        let carry = datetime.add_minutes(1);
        assert_eq!(carry.seconds_carry(), 1);
        assert_eq!(carry.days_carry(), 0);
        let datetime = carry.drop_carry();
        assert_eq!(datetime.year, 1991);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 59);

        // And back again.
        let datetime = datetime.add_minutes(-1).unwrap();
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 59);

        // Now try adding the carry.
        let datetime = datetime.add_seconds(1);
        let datetime = datetime.add_minutes(1).apply_carry();
        assert_eq!(datetime.year, 1991);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 1);
        assert_eq!(datetime.second, 0);
    }

    #[test]
    fn carry_inverse_property_minutes() {
        // Verify that the inverse property holds for adding/subtracting minutes.
        let datetime = DateTime::builder()
            .year(1990)
            .month(12)
            .day(31)
            .hour(23)
            .minute(59)
            .second(60)
            .build();

        let datetime = datetime.add_minutes(1);
        assert_eq!(datetime.seconds_carry(), 1);
        assert_eq!(datetime.days_carry(), 0);
        let datetime = datetime.add_minutes(-1);
        assert_eq!(datetime.seconds_carry(), 1);
        assert_eq!(datetime.days_carry(), 0);
        let datetime = datetime.apply_carry();
        assert_eq!(datetime.year, 1990);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 23);
        assert_eq!(datetime.minute, 59);
        assert_eq!(datetime.second, 60);
    }

    #[test]
    fn add_months() {
        let datetime = DateTime::builder().year(2000).month(3).day(1).build();

        let datetime = datetime.add_months(1).unwrap();
        assert_eq!(datetime.year, 2000);
        assert_eq!(datetime.month, 4);
        assert_eq!(datetime.day, 1);

        let datetime = datetime.add_months(-1).unwrap();
        assert_eq!(datetime.year, 2000);
        assert_eq!(datetime.month, 3);
        assert_eq!(datetime.day, 1);

        let datetime = datetime.add_months(-2).unwrap();
        assert_eq!(datetime.year, 2000);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);

        let datetime = datetime.add_months(-1).unwrap();
        assert_eq!(datetime.year, 1999);
        assert_eq!(datetime.month, 12);
        assert_eq!(datetime.day, 1);

        let datetime = datetime.add_months(1).unwrap();
        assert_eq!(datetime.year, 2000);
        assert_eq!(datetime.month, 1);
        assert_eq!(datetime.day, 1);
    }

    #[test]
    fn carry_inverse_property_months() {
        // Verify that the inverse property holds for adding/subtracting months.
        let datetime = DateTime::builder()
            .year(2000)
            .month(3)
            .day(31)
            .hour(0)
            .minute(0)
            .second(0)
            .build();

        // TODO could keep segment position, julian day etc in carry to avoid recomputation.
        /*let datetime = datetime.add_months(1);
        assert_eq!(datetime.seconds_carry(), 0);
        assert_eq!(datetime.days_carry(), 1);
        let datetime = datetime.add_months(-1);
        assert_eq!(datetime.seconds_carry(), 0);
        assert_eq!(datetime.days_carry(), 1);
        let datetime = datetime.add_carry();
        assert_eq!(datetime.year, 2000);
        assert_eq!(datetime.month, 3);
        assert_eq!(datetime.day, 31);
        assert_eq!(datetime.hour, 0);
        assert_eq!(datetime.minute, 0);
        assert_eq!(datetime.second, 0);*/
    }
}
