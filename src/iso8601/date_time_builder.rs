use crate::gregorian_normalized_date::GregorianNormalizedDate;
use crate::iso8601::chronology::{load_chronology, Chronology};
use crate::iso8601::precision::Precision;
use crate::iso8601::DateTime;
use crate::zoneinfo::SegmentLookupResult;
use std::cmp::max;
use std::fmt::Debug;

#[derive(Default)]
pub struct DateTimeBuilder {
    chronology: Option<Chronology>,
    precision: Option<Precision>,
    year: Option<u16>,
    month: Option<u8>,
    day: Option<u8>,
    hour: Option<u8>,
    minute: Option<u8>,
    second: Option<u8>,
    millisecond: Option<u16>,
    microsecond: Option<u16>,
    nanosecond: Option<u16>,
    offset_hour: Option<u8>,
    offset_minute: Option<u8>,
}

#[derive(Eq, PartialEq)]
pub enum Error {
    InvalidDateTime,
    DateTimeOutOfBounds,
}

impl From<crate::gregorian_normalized_date::Error> for Error {
    fn from(e: crate::gregorian_normalized_date::Error) -> Self {
        match e {
            crate::gregorian_normalized_date::Error::InvalidDate => Error::InvalidDateTime,
            crate::gregorian_normalized_date::Error::DateOutOfBounds => Error::DateTimeOutOfBounds,
        }
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidDateTime => write!(f, "invalid datetime"),
            Error::DateTimeOutOfBounds => write!(f, "datetime out of bounds"),
        }
    }
}

impl DateTimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn chronology(&mut self, chronology: &Chronology) -> &mut Self {
        self.chronology = Some(chronology.clone());
        self
    }

    pub fn year(&mut self, year: u16) -> &mut Self {
        self.year = Some(year);
        self.precision = opt_max(self.precision, Precision::Years);
        self
    }

    pub fn month(&mut self, month: u8) -> &mut Self {
        self.month = Some(month);
        self.precision = opt_max(self.precision, Precision::Months);
        self
    }

    pub fn day(&mut self, day: u8) -> &mut Self {
        self.day = Some(day);
        self.precision = opt_max(self.precision, Precision::Days);
        self
    }

    pub fn hour(&mut self, hour: u8) -> &mut Self {
        self.hour = Some(hour);
        self.precision = opt_max(self.precision, Precision::Hours);
        self
    }

    pub fn minute(&mut self, minute: u8) -> &mut Self {
        self.minute = Some(minute);
        self.precision = opt_max(self.precision, Precision::Minutes);
        self
    }

    pub fn second(&mut self, second: u8) -> &mut Self {
        self.second = Some(second);
        self.precision = opt_max(self.precision, Precision::Seconds);
        self
    }

    pub fn millisecond(&mut self, millisecond: u16) -> &mut Self {
        self.millisecond = Some(millisecond);
        self.precision = opt_max(self.precision, Precision::Milliseconds);
        self
    }

    // TODO microsecond_of_second and nanosecond_of_second for specifying fractions of a second
    //  directly, instead of as a sum of milliseconds, microseconds, and nanoseconds.

    pub fn microsecond(&mut self, microsecond: u16) -> &mut Self {
        self.microsecond = Some(microsecond);
        self.precision = opt_max(self.precision, Precision::Microseconds);
        self
    }

    pub fn nanosecond(&mut self, nanosecond: u16) -> &mut Self {
        self.nanosecond = Some(nanosecond);
        self.precision = opt_max(self.precision, Precision::Nanoseconds);
        self
    }

    pub fn offset_hour(&mut self, offset_hour: u8) -> &mut Self {
        self.offset_hour = Some(offset_hour);
        self
    }

    pub fn offset_minute(&mut self, offset_minute: u8) -> &mut Self {
        self.offset_minute = Some(offset_minute);
        self
    }

    pub fn build(&self) -> DateTime {
        match self.checked_build() {
            Ok(dt) => dt,
            Err(e) => panic!("{:?}", e),
        }
    }

    pub fn checked_build(&self) -> Result<DateTime, Error> {
        let precision = self.precision.expect("No values have been provided");
        let chronology = match self.chronology {
            Some(ref chronology) => chronology.clone(),
            None => load_chronology("UTC"),
        };
        let year = self.year.expect("No year provided");
        let month = if precision >= Precision::Months {
            self.month.expect("No month provided")
        } else {
            0
        };
        let day = if precision >= Precision::Days {
            self.day.expect("No day provided")
        } else {
            0
        };
        let hour = if precision >= Precision::Hours {
            self.hour.expect("No hour provided")
        } else {
            0
        };
        let minute = if precision >= Precision::Minutes {
            self.minute.expect("No minute provided")
        } else {
            0
        };
        let second = if precision >= Precision::Seconds {
            self.second.expect("No second provided")
        } else {
            0
        };
        let millisecond = if precision >= Precision::Milliseconds {
            self.millisecond.expect("No millisecond provided")
        } else {
            0
        };
        let microsecond = if precision >= Precision::Microseconds {
            self.microsecond.expect("No microsecond provided")
        } else {
            0
        };
        let nanosecond = if precision >= Precision::Nanoseconds {
            self.nanosecond.expect("No nanosecond provided")
        } else {
            0
        };
        let offset_hour = self.offset_hour.unwrap_or(0);
        let offset_minute = self.offset_minute.unwrap_or(0);
        // TODO instant, ensure datetime validity, set offset from chronology (or smth - should we even have those members?)
        if year > 9999 {
            return Err(Error::DateTimeOutOfBounds);
        }
        let gnd = GregorianNormalizedDate::from_date(year as i32, month, day)?;

        if hour >= 24 || minute >= 60 {
            return Err(Error::InvalidDateTime);
        }

        let minute_length = if hour != 23 || minute != 59 {
            60
        } else {
            let leap_seconds = chronology.leap_seconds();
            let fixed_day = gnd.to_day();
            if let SegmentLookupResult::In(segment) = leap_seconds.by_day(fixed_day) {
                let day_in_segment = fixed_day as u32 - segment.start_day;
                let last_day = day_in_segment == segment.duration_days - 1;
                if !last_day {
                    60
                } else {
                    (60 + segment.leap_seconds) as u8
                }
            } else {
                60
            }
        };

        if second >= minute_length
            || millisecond >= 1000
            || microsecond >= 1000
            || nanosecond >= 1000
        {
            return Err(Error::InvalidDateTime);
        }

        let second = hour as u32 * 3600 + minute as u32 * 60 + second as u32;
        let nanosecond =
            millisecond as u32 * 1_000_000 + microsecond as u32 * 1_000 + nanosecond as u32;
        Ok(DateTime::new(
            chronology, precision, gnd, second, nanosecond,
        ))
    }
}

fn opt_max<T: Ord + Copy>(lhs: Option<T>, rhs: T) -> Option<T> {
    Some(match lhs {
        Some(x) => max(x, rhs),
        None => rhs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        // GregorianNormalizedDate epoch
        let dt = DateTimeBuilder::new()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert_eq!(dt.year(), 2000);
        assert_eq!(dt.month(), 3);
        assert_eq!(dt.day(), 1);
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);

        // DateTime epoch
        let dt = DateTimeBuilder::new()
            .year(1970)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .build();
        assert_eq!(dt.year(), 1970);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 1);
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);

        // Out of bounds year.
        let result = DateTimeBuilder::new()
            .year(10000)
            .month(1)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .checked_build();
        assert_eq!(result, Err(Error::DateTimeOutOfBounds));

        // Invalid month.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(13)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Invalid day of month.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(3)
            .day(32)
            .hour(0)
            .minute(0)
            .second(0)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Leap day on a non-leap year.
        let result = DateTimeBuilder::new()
            .year(2001)
            .month(2)
            .day(29)
            .hour(0)
            .minute(0)
            .second(0)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Leap day on a leap year.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(2)
            .day(29)
            .hour(0)
            .minute(0)
            .second(0)
            .checked_build();
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2000);
        assert_eq!(dt.month(), 2);
        assert_eq!(dt.day(), 29);
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);

        // Invalid hour.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(3)
            .day(1)
            .hour(24)
            .minute(0)
            .second(0)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Invalid minute.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(60)
            .second(0)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Invalid second.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(60)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Valid leap second.
        let result = DateTimeBuilder::new()
            .year(1972)
            .month(6)
            .day(30)
            .hour(23)
            .minute(59)
            .second(60)
            .checked_build();
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 1972);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 30);
        assert_eq!(dt.hour(), 23);
        assert_eq!(dt.minute(), 59);
        assert_eq!(dt.second(), 60);

        // Invalid second at end of leap second segment.
        let result = DateTimeBuilder::new()
            .year(1972)
            .month(6)
            .day(30)
            .hour(23)
            .minute(59)
            .second(61)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Invalid millisecond.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .millisecond(1000)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Invalid microsecond.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .millisecond(0)
            .microsecond(1000)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));

        // Invalid nanosecond.
        let result = DateTimeBuilder::new()
            .year(2000)
            .month(3)
            .day(1)
            .hour(0)
            .minute(0)
            .second(0)
            .millisecond(0)
            .microsecond(0)
            .nanosecond(1000)
            .checked_build();
        assert_eq!(result, Err(Error::InvalidDateTime));
    }
}
