use crate::gregorian_normalized_date::GregorianNormalizedDate;
use std::cmp::max;

use crate::iso8601::chronology::{load_chronology, Chronology};
use crate::iso8601::precision::Precision;
use crate::iso8601::DateTime;

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

    // TODO don't allow building invalid datetime (e.g. 32nd of January)
    pub fn build(&self) -> DateTime {
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
        let gnd = GregorianNormalizedDate::from_date(year as i32, month, day);
        let second = hour as u32 * 3600 + minute as u32 * 60 + second as u32;
        let nanosecond =
            millisecond as u32 * 1_000_000 + microsecond as u32 * 1_000 + nanosecond as u32;
        DateTime::new(chronology, precision, gnd, second, nanosecond)
    }
}

fn opt_max<T: Ord + Copy>(lhs: Option<T>, rhs: T) -> Option<T> {
    Some(match lhs {
        Some(x) => max(x, rhs),
        None => rhs,
    })
}
