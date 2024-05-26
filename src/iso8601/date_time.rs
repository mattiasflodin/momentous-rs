use crate::cursor::Cursor;
use crate::gregorian_normalized_date::GregorianNormalizedDate;
use crate::iso8601::chronology::Chronology;
use crate::iso8601::precision::Precision;
use crate::iso8601::{
    DateTimeBuilder, HOURS_PER_DAY, MINUTES_PER_HOUR, SECONDS_PER_DAY, SECONDS_PER_HOUR,
    SECONDS_PER_MINUTE,
};
use crate::shared_vec_cursor::SharedVecCursor;
use crate::zoneinfo::{get_leap_seconds, ContinuousTimeSegment};
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

    // TODO function to transfer as much carry as possible to datetime without
    // overflowing to the next component. E.g. with a second of 58 and a carry of
    // 2, the carry should be reduced to 1 and the second increased to 59.
    // I *think* there's a use case for this but that would need to be figured
    // out as well. Something about chaining multiple operations without
    // "falling behind" the actual time. It might matter when you're e.g.
    // adding a month to something that has fallen behind? I'm not sure.

    pub fn add_days(&self, days: i128) -> Self {
        todo!()
    }

    pub fn add_seconds(&self, seconds: i128) -> Self {
        todo!()
    }

    pub fn add_minutes(&self, minutes: i128) -> Self {
        todo!()
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
}
