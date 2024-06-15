// The gregorian calendar works in cycles of 400 years. Each cycle starts with a leap year.
// From then every 4th year is a leap year, except for every 100th year. So the year
// 1600 is a leap year but 1700, 1800 and 1900 are not. This means that each cycle
// has 100-3=97 leap years and 303 normal years. 97*366 + 303*365 = 146097 days.
// Our segment start day is given in days since unix epoch (1970-01-01). To get this
// aligned with "cycles" we need to use a reference that starts with the beginning of a cycle.
//
// However, having the leap year at the beginning of a cycle or quadrennium makes calculations more
// complicated, since we have to take into account the extra day in the initial period. By shifting values so
// that the leap day comes out at the end of each period we can just let the leap days come naturally as an
// "overflow", without any branches in control flow (other than the implicit branch in a call to min() inside
// clamped_div_rem. So we pick 2000-03-01 as zero point, right after the last leap day of the preceding cycle.
// We then have a quadrennium consisting of the "years"
// - 2000-03-01 to 2001-02-28
// - 2001-03-01 to 2002-02-28
// - 2002-03-01 to 2003-02-28
// - 2003-03-01 to 2004-02-29
//
// This way the year ends with the leap day and the quadrennium ends with the leap year. The cycle ends with the
// "leap century" having
// - 2396-03-01 to 2397-02-28
// - 2397-03-01 to 2398-02-28
// - 2398-03-01 to 2399-02-28
// - 2399-03-01 to 2400-02-29

use crate::div_rem::ClampedDivRem;
use num_integer::Integer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GregorianNormalizedDate {
    // Number of 400-year cycles since 2000-03-01. We need to support dates from 0000-03-01 to
    // 9999-02-29; it turns out that 8 bits is enough to represent this, as 400*127 = 50800.
    pub(crate) cycle: i8,
    // Number of centuries since the start of the cycle (0-3)
    pub(crate) century: u8,
    // Number of quadrennia (4-year periods) since the start of the century (0-24).
    pub(crate) quadrennium: u8,
    // Number of years since the start of the quadrennium (0-3).
    pub(crate) year: u8,
    // Number of days since the start of the year (0-366, where the year starts March 1).
    pub(crate) day: u16,
}

const GREGORIAN_CYCLE_DAYS: u32 = 97 * 366 + 303 * 365;
const GREGORIAN_CENTURY_DAYS: u16 = 24 * 366 + 76 * 365;
#[allow(clippy::identity_op)]
const GREGORIAN_QUADRENNIUM_DAYS: u16 = 3 * 365 + 1 * 366;
const GREGORIAN_YEAR_DAYS: u16 = 365;
const GREGORIAN_CYCLE_YEARS: u16 = 400;
const GREGORIAN_CYCLE_CENTURIES: u8 = 4;
const GREGORIAN_CENTURY_YEARS: u8 = 100;
const GREGORIAN_CENTURY_QUADRENNIUMS: u8 = 25;
const GREGORIAN_QUADRENNIUM_YEARS: u8 = 4;

const GREGORIAN_NORMALIZED_DATE_OFFSET_DAYS: u16 = 11017; // 11017 days from 1970-01-01 to 2000-03-01
const GREGORIAN_MONTH_STARTS: [u16; 13] =
    [0, 31, 61, 92, 122, 153, 184, 214, 245, 275, 306, 337, 65535]; // Index 0 = March
const GREGORIAN_MONTH_LENGTHS: [u8; 11] = [31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31];
const JANUARY_1_DAY_OFFSET: u16 = 306;

fn month_day_from_year_day(day: u16) -> (u8, u8) {
    let mut month = (day / 30) as u8;
    let mut month_start_day = GREGORIAN_MONTH_STARTS[month as usize];
    if day < month_start_day {
        // We have overshot the month. Move back.
        month -= 1;
        month_start_day = GREGORIAN_MONTH_STARTS[month as usize];
    }
    (month, (day - month_start_day) as u8)
}

impl GregorianNormalizedDate {
    pub(crate) fn from_day(day: i32) -> Self {
        let day = day - GREGORIAN_NORMALIZED_DATE_OFFSET_DAYS as i32;
        let (cycle, days_into_cycle) = day.div_mod_floor(&(GREGORIAN_CYCLE_DAYS as i32));
        let days_into_cycle = days_into_cycle as u32; // 2^18 days per cycle

        // The first three centuries of each cycle are normal centuries with 24 leap years and 76 normal years.
        // The fourth century is a leap century with 25 leap years and 75 normal years, so it has one extra leap day
        // at the end.
        let (century, days_into_century) =
            days_into_cycle.clamped_div_rem(GREGORIAN_CENTURY_DAYS as u32, 3_u8);
        let days_into_century = days_into_century as u16; // 2^16 days per century

        // Each quadrennium has 3 normal years and 1 leap year, so it has one extra leap day at the end. The last
        // quadrennium of the first three centuries are exceptional since they lack the leap day, so they have one
        // day less. This means we can do a normal division (without clamped quotient).
        let (quadrennium, days_into_quadrennium) =
            days_into_century.div_rem(&GREGORIAN_QUADRENNIUM_DAYS);
        let quadrennium = quadrennium as u8;

        let (years_into_quadrennium, days_into_year) =
            days_into_quadrennium.clamped_div_rem(GREGORIAN_YEAR_DAYS, 3_u8);

        GregorianNormalizedDate {
            cycle: cycle as i8,
            century,
            quadrennium,
            year: years_into_quadrennium,
            day: days_into_year,
        }
    }

    pub(crate) fn to_day(&self) -> i32 {
        let cycle = self.cycle as i32;
        let century = self.century as i32;
        let quadrennium = self.quadrennium as i32;
        let year = self.year as i32;
        let day = self.day as i32;
        cycle * GREGORIAN_CYCLE_DAYS as i32
            + century * GREGORIAN_CENTURY_DAYS as i32
            + quadrennium * GREGORIAN_QUADRENNIUM_DAYS as i32
            + year * GREGORIAN_YEAR_DAYS as i32
            + day
            + GREGORIAN_NORMALIZED_DATE_OFFSET_DAYS as i32
    }

    pub(crate) fn from_date(year: u16, month: u8, day: u8) -> Self {
        assert!((1..=12).contains(&month));
        assert!((1..=31).contains(&day));

        let mut year = year as i16; // Year will not be greater than 9999.
        let mut month = month - 1;
        let day = day - 1;
        if month < 2 {
            month += 12;
            year -= 1;
        }
        month -= 2;
        year -= 2000;
        let (cycle, years_into_cycle) = year.div_mod_floor(&(GREGORIAN_CYCLE_YEARS as i16));
        let cycle = cycle as i8; // Cycle will not be greater than 24.
        let years_into_cycle = years_into_cycle as u16; // 2^9 years per cycle
        let (century, years_into_century) =
            years_into_cycle.clamped_div_rem(GREGORIAN_CENTURY_YEARS as u16, 3_u8);
        let (quadrennium, years_into_quadrennium) =
            years_into_century.clamped_div_rem(GREGORIAN_QUADRENNIUM_YEARS as u16, 24_u8);
        let years_into_quadrennium = years_into_quadrennium as u8; // 2^2 years per quadrennium

        let month_day_offset = GREGORIAN_MONTH_STARTS[month as usize];
        let days_into_year = month_day_offset + day as u16;
        GregorianNormalizedDate {
            cycle,
            century,
            quadrennium,
            year: years_into_quadrennium,
            day: days_into_year,
        }
    }

    pub(crate) fn to_date(&self) -> (u16, u8, u8) {
        let year = 2000
            + 400 * self.cycle as i16
            + 100 * self.century as i16
            + 4 * self.quadrennium as i16
            + self.year as i16;
        let mut year = year as u16;

        // NB: shifted so march is first. This way we don't need to care about how leap days
        // affect the month start since the leap day comes at the end of the year.
        let (mut month, days_into_month) = month_day_from_year_day(self.day);

        // Now adjust so march is represented as month 3 instead of month 1, since we want to be based off of the Gregorian new year.
        month += 2;
        if month >= 12 {
            month -= 12;
            year += 1;
        }
        (year, month + 1, days_into_month + 1)
    }

    pub(crate) fn days_in_month(&self) -> u8 {
        let (month, _) = month_day_from_year_day(self.day);
        if month < 11 {
            (GREGORIAN_MONTH_STARTS[(month + 1) as usize] - GREGORIAN_MONTH_STARTS[month as usize])
                as u8
        } else if self.is_leap_year() {
            29
        } else {
            28
        }
    }

    pub(crate) fn unnormalized_year(&self) -> u16 {
        // Since the normalized year is shifted so that January and February belong
        // to the previous year, we may get the year -1 here. This is adjusted below,
        // but we need to take care to keep the year signed until then.
        let year = 2000
            + 400 * self.cycle as i16
            + 100 * self.century as i16
            + 4 * self.quadrennium as i16
            + self.year as i16;

        (if self.day >= JANUARY_1_DAY_OFFSET {
            year + 1
        } else {
            year
        }) as u16
    }

    pub(crate) fn unnormalized_month(&self) -> u8 {
        let (month, _) = month_day_from_year_day(self.day);
        let month = month + 3;
        if month > 12 {
            month - 12
        } else {
            month
        }
    }

    pub(crate) fn unnormalized_day(&self) -> u8 {
        let (_, day) = month_day_from_year_day(self.day);
        day + 1
    }

    pub(crate) fn is_unnormalized_leap_year(&self) -> bool {
        // Leap years are at the end of each period: quadrennium, century and cycle.
        // However, because of the way we've shifted the year so that it begins in march,
        // it is only a leap year if the day is 306 (jan 1) or greater. If it is 305 (dec 31)
        // or less, we must subtract one from the current year and check the result of that. This
        // creates complicated code with a lot of special cases and branches. It'll likely be both
        // faster and simpler to just compute the gregorian year and check that per the usual
        // definition of a leap year. We use the same method as in to_date but ignore the
        // cycle since it doesn't matter.
        let mut year = 100 * self.century as u16 + 4 * self.quadrennium as u16 + self.year as u16;
        // We could use a binary search on GREGORIAN_MONTH_STARTS to find out the month (as we do
        // in to_date), but we really only need to know if month+2 is >= 12. Or in other words,
        // whether the day is on January 1 (day 306) or later.
        if self.day >= JANUARY_1_DAY_OFFSET {
            year = (year + 1) % 400
        }

        (year % 4 == 0) && (year % 100 != 0 || year == 0)
    }

    // Returns day carry
    pub(crate) fn add_years(&mut self, years: i16) -> bool {
        self.add_years_no_carry(years);

        if self.day == 365 && !self.is_leap_year() {
            self.day = 364;
            true
        } else {
            false
        }
    }

    fn add_years_no_carry(&mut self, years: i16) {
        // At each step of the calculation the remainder will be within the
        // range of the divisor, but the quotient can be as large as the entire
        // span of representable years. The years parameter can be a maximum of
        // +-9999. That's 2500 quadrenniums, 100 centuries, 25 cycles. So data
        // types for the arithmetic are chosen to fit this.
        let (quadrenniums, year) =
            (self.year as i16 + years).div_mod_floor(&(GREGORIAN_QUADRENNIUM_YEARS as i16));
        let year = year as u8;

        let (centuries, quadrennium) = (self.quadrennium as i16 + quadrenniums)
            .div_mod_floor(&(GREGORIAN_CENTURY_QUADRENNIUMS as i16));
        let (centuries, quadrennium) = (centuries as i8, quadrennium as u8);

        let (cycle, century) =
            (self.century as i8 + centuries).div_mod_floor(&(GREGORIAN_CYCLE_CENTURIES as i8));
        let century = century as u8;

        self.year = year;
        self.quadrennium = quadrennium;
        self.century = century;
        self.cycle += cycle;
    }

    // Returns day carry
    pub(crate) fn add_months(&mut self, months: i32) -> u8 {
        let (month, _) = month_day_from_year_day(self.day);
        let day_in_month = (self.day - GREGORIAN_MONTH_STARTS[month as usize]) as u8;
        let (add_years, month) = (month as i32 + months).div_mod_floor(&12);
        let (add_years, month) = (add_years as i16, month as u8);

        self.add_years_no_carry(add_years);

        let total_days_in_month = if month == 11 {
            if self.is_leap_year() {
                29
            } else {
                28
            }
        } else {
            GREGORIAN_MONTH_LENGTHS[month as usize]
        };

        let carry = if day_in_month >= total_days_in_month {
            // Need to add +1 because day_in_month is 0-based. If there are
            // 30 days total and day_in_month is 30, then it the day needs to
            // become 29 giving us a carry of 1. Or in other words,
            // (30 + 1) - 30 = 1.
            (day_in_month + 1) - total_days_in_month
        } else {
            0
        };
        self.day = GREGORIAN_MONTH_STARTS[month as usize] + (day_in_month - carry) as u16;
        carry
    }

    pub(crate) fn add_days(&mut self, days: i32) {
        if days > 0 {
            self.add_days_forward(days as u32);
        } else {
            self.add_days_backward((-days) as u32);
        }
    }

    fn add_days_forward(&mut self, days: u32) {
        // -1 because the last day of the year has 0 remaining days (and the day is zero-based).
        let remaining_days_in_year = (self.year_length() - 1) - self.day;
        if days <= remaining_days_in_year as u32 {
            self.day += days as u16;
        } else {
            *self = GregorianNormalizedDate::from_day(self.to_day() + days as i32);
        }
    }

    fn add_days_backward(&mut self, days: u32) {
        if days <= self.day as u32 {
            self.day -= days as u16;
        } else {
            *self = GregorianNormalizedDate::from_day(self.to_day() - days as i32);
        }
    }

    fn year_length(&self) -> u16 {
        if self.is_leap_year() {
            366
        } else {
            365
        }
    }

    fn is_leap_year(&self) -> bool {
        // Determine whether the current year (in the normalized calendar) has
        // 365 or 366 days. The normalized calendar is constructed so that the
        // leap day comes at the end of the year and always on year 3. The
        // exception is a date like 1900-02-29, which is invalid because it's
        // the first year of a century but not evenly divisible by 400. In the
        // normalized calendar this is represented as (cycle: -1, century: 2,
        // quadrennium: 24, year: 3, day: 365). Here the quadrennium is maxed
        // out, but the century is not. This is the only case where the year is
        // 3 but the year is not a leap year.
        self.year == 3 && !(self.quadrennium == 24 && self.century != 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gregorian_normalized_date() {
        // 1970-01-01, the zero-point of unix time.
        let date = GregorianNormalizedDate::from_day(0);
        // Because normalized years start in march, the normalized representation will be based on the year 1969.
        // 2000 -1*400 + 3*100 + 17*4 + 1 = 1969.
        assert_eq!(date.cycle, -1);
        assert_eq!(date.century, 3);
        assert_eq!(date.quadrennium, 17);
        assert_eq!(date.year, 1);
        assert_eq!(date.day, 306); // 306 days from 1969-03-01 to 1970-01-01.

        let day = date.to_day();
        assert_eq!(day, 0);

        let date = date.to_date();
        assert_eq!(date.0, 1970);
        assert_eq!(date.1, 1);
        assert_eq!(date.2, 1);

        // The zero-point of normalized dates.
        let date = GregorianNormalizedDate::from_date(2000, 3, 1);
        assert_eq!(date.cycle, 0);
        assert_eq!(date.century, 0);
        assert_eq!(date.quadrennium, 0);
        assert_eq!(date.year, 0);
        assert_eq!(date.day, 0);

        let day = date.to_day();
        assert_eq!(day, 11017);

        let date = date.to_date();
        assert_eq!(date.0, 2000);
        assert_eq!(date.1, 3);
        assert_eq!(date.2, 1);

        // The end of a cycle.
        let date = GregorianNormalizedDate::from_date(2000, 2, 29);
        assert_eq!(date.cycle, -1);
        assert_eq!(date.century, 3);
        assert_eq!(date.quadrennium, 24);
        assert_eq!(date.year, 3);
        assert_eq!(date.day, 365);

        // The end of the year before that. Just to probe around the leap day.
        let date = GregorianNormalizedDate::from_date(1999, 2, 28);
        assert_eq!(date.cycle, -1);
        assert_eq!(date.century, 3);
        assert_eq!(date.quadrennium, 24);
        assert_eq!(date.year, 2);
        assert_eq!(date.day, 364);
    }

    #[test]
    fn test_get_unnormalized_year() {
        let date = GregorianNormalizedDate::from_date(2000, 3, 1);
        assert_eq!(date.unnormalized_year(), 2000);

        let date = GregorianNormalizedDate::from_date(2000, 2, 29);
        assert_eq!(date.unnormalized_year(), 2000);

        let date = GregorianNormalizedDate::from_date(2000, 3, 2);
        assert_eq!(date.unnormalized_year(), 2000);

        let date = GregorianNormalizedDate::from_date(2000, 1, 1);
        assert_eq!(date.unnormalized_year(), 2000);

        let date = GregorianNormalizedDate::from_date(1999, 12, 31);
        assert_eq!(date.unnormalized_year(), 1999);

        let date = GregorianNormalizedDate::from_date(0, 1, 1);
        assert_eq!(date.unnormalized_year(), 0);

        let date = GregorianNormalizedDate::from_date(9999, 12, 31);
        assert_eq!(date.unnormalized_year(), 9999);
    }

    #[test]
    fn test_get_unnormalized_month() {
        let date = GregorianNormalizedDate::from_date(2000, 3, 1);
        assert_eq!(date.unnormalized_month(), 3);

        let date = GregorianNormalizedDate::from_date(2000, 2, 29);
        assert_eq!(date.unnormalized_month(), 2);

        let date = GregorianNormalizedDate::from_date(2000, 1, 1);
        assert_eq!(date.unnormalized_month(), 1);

        let date = GregorianNormalizedDate::from_date(1999, 12, 31);
        assert_eq!(date.unnormalized_month(), 12);
    }

    #[test]
    fn test_add_months() {
        let mut date = GregorianNormalizedDate::from_date(2000, 3, 1);
        let carry = date.add_months(1);
        assert_eq!(carry, 0);
        assert_eq!(date.to_date(), (2000, 4, 1));

        let mut date = GregorianNormalizedDate::from_date(2000, 3, 1);
        let carry = date.add_months(12);
        assert_eq!(carry, 0);
        assert_eq!(date.to_date(), (2001, 3, 1));

        let mut date = GregorianNormalizedDate::from_date(2000, 3, 1);
        let carry = date.add_months(13);
        assert_eq!(carry, 0);
        assert_eq!(date.to_date(), (2001, 4, 1));

        let mut date = GregorianNormalizedDate::from_date(2000, 3, 1);
        let carry = date.add_months(24);
        assert_eq!(carry, 0);
        assert_eq!(date.to_date(), (2002, 3, 1));

        let mut date = GregorianNormalizedDate::from_date(2000, 3, 1);
        let carry = date.add_months(25);
        assert_eq!(carry, 0);
        assert_eq!(date.to_date(), (2002, 4, 1));

        let mut date = GregorianNormalizedDate::from_date(2000, 3, 1);
        let carry = date.add_months(-1);
        assert_eq!(carry, 0);
        assert_eq!(date.to_date(), (2000, 2, 1));

        let mut date = GregorianNormalizedDate::from_date(2000, 1, 31);
        let carry = date.add_months(1);
        assert_eq!(carry, 2);
        assert_eq!(date.to_date(), (2000, 2, 29));

        let mut date = GregorianNormalizedDate::from_date(2001, 1, 31);
        let carry = date.add_months(1);
        assert_eq!(carry, 3);
        assert_eq!(date.to_date(), (2001, 2, 28));
    }

    #[test]
    fn test_add_days() {
        // 2000-03-01, the zero-point of normalized dates.
        let mut date = GregorianNormalizedDate::from_date(2000, 3, 1);
        date.add_days(1);
        assert_eq!(date.to_date(), (2000, 3, 2));
        date.add_days(-1);
        assert_eq!(date.to_date(), (2000, 3, 1));
        date.add_days(-1);
        assert_eq!(date.to_date(), (2000, 2, 29));
        date.add_days(1);
        assert_eq!(date.to_date(), (2000, 3, 1));

        // 1999-03-01, a non-leap year.
        let mut date = GregorianNormalizedDate::from_date(1999, 3, 1);
        date.add_days(1);
        assert_eq!(date.to_date(), (1999, 3, 2));
        date.add_days(-1);
        assert_eq!(date.to_date(), (1999, 3, 1));
        date.add_days(-1);
        assert_eq!(date.to_date(), (1999, 2, 28));
        date.add_days(1);
        assert_eq!(date.to_date(), (1999, 3, 1));

        let mut date = GregorianNormalizedDate::from_date(2000, 3, 1);
        date.add_days(365);
        assert_eq!(date.to_date(), (2001, 3, 1));
        date.add_days(-365);
        assert_eq!(date.to_date(), (2000, 3, 1));
        date.add_days(-365);
        assert_eq!(date.to_date(), (1999, 3, 2)); // because of leap day we end up on 1999-03-02.
        date.add_days(365);
        assert_eq!(date.to_date(), (2000, 3, 1));
    }

    #[test]
    fn test_is_unnormalized_leap_year() {
        // 2000-03-01, the zero-point of normalized dates.
        let date = GregorianNormalizedDate::from_date(2000, 3, 1);
        assert!(date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2000, 2, 29);
        assert!(date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2000, 3, 2);
        assert!(date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2000, 1, 1);
        assert!(date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(1999, 12, 31);
        assert!(!date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2001, 3, 1);
        assert!(!date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2001, 2, 28);
        assert!(!date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2001, 3, 2);
        assert!(!date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2004, 3, 1);
        assert!(date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2004, 2, 29);
        assert!(date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(2004, 3, 2);
        assert!(date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(1900, 3, 1);
        assert!(!date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(1900, 1, 1);
        assert!(!date.is_unnormalized_leap_year());

        let date = GregorianNormalizedDate::from_date(1899, 12, 31);
        assert!(!date.is_unnormalized_leap_year());
    }

    #[test]
    fn test_month_from_day_offset() {
        assert_eq!(month_day_from_year_day(0), (0, 0));
        assert_eq!(month_day_from_year_day(30), (0, 30));
        assert_eq!(month_day_from_year_day(31), (1, 0));
        assert_eq!(month_day_from_year_day(60), (1, 29));
        assert_eq!(month_day_from_year_day(61), (2, 0));
        assert_eq!(month_day_from_year_day(91), (2, 30));
        assert_eq!(month_day_from_year_day(92), (3, 0));
        assert_eq!(month_day_from_year_day(121), (3, 29));
        assert_eq!(month_day_from_year_day(122), (4, 0));
        assert_eq!(month_day_from_year_day(152), (4, 30));
        assert_eq!(month_day_from_year_day(153), (5, 0));
        assert_eq!(month_day_from_year_day(183), (5, 30));
        assert_eq!(month_day_from_year_day(184), (6, 0));
        assert_eq!(month_day_from_year_day(213), (6, 29));
        assert_eq!(month_day_from_year_day(214), (7, 0));
        assert_eq!(month_day_from_year_day(244), (7, 30));
        assert_eq!(month_day_from_year_day(245), (8, 0));
        assert_eq!(month_day_from_year_day(274), (8, 29));
        assert_eq!(month_day_from_year_day(275), (9, 0));
        assert_eq!(month_day_from_year_day(305), (9, 30));
        assert_eq!(month_day_from_year_day(306), (10, 0));
        assert_eq!(month_day_from_year_day(336), (10, 30));
        assert_eq!(month_day_from_year_day(337), (11, 0));
        assert_eq!(month_day_from_year_day(365), (11, 28));
    }
}
