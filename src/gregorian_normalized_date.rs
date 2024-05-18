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

pub(crate) struct GregorianNormalizedDate {
    // Number of 400-year cycles since 2000-03-01.
    cycle: i128,
    // Number of centuries since the start of the cycle (0-3)
    century: u8,
    // Number of quadrennia (4-year periods) since the start of the century (0-24).
    quadrennium: u8,
    // Number of years since the start of the quadrennium (0-3).
    year: u8,
    // Number of days since the start of the year (0-366, where the year starts March 1).
    day: u16,
}

const GREGORIAN_CYCLE_DAYS: u32 = 97 * 366 + 303 * 365;
const GREGORIAN_CENTURY_DAYS: u16 = 24 * 366 + 76 * 365;
#[allow(clippy::identity_op)]
const GREGORIAN_QUADRENNIUM_DAYS: u16 = 3 * 365 + 1 * 366;
const GREGORIAN_YEAR_DAYS: u16 = 365;
const GREGORIAN_CYCLE_YEARS: u16 = 400;
const GREGORIAN_CENTURY_YEARS: u8 = 100;
const GREGORIAN_QUADRENNIUM_YEARS: u8 = 4;

const GREGORIAN_NORMALIZED_DATE_OFFSET_DAYS: u16 = 11017; // 11017 days from 1970-01-01 to 2000-03-01
const GREGORIAN_MONTH_STARTS: [u16; 13] =
    [0, 31, 61, 92, 122, 153, 184, 214, 245, 275, 306, 337, 65535]; // Index 0 = March

fn month_from_day_offset(day: u16) -> u8 {
    let mut month = (day / 30) as u8;
    if day < GREGORIAN_MONTH_STARTS[month as usize] {
        // We have overshot the month. Move back.
        month -= 1;
    }
    month
}

impl GregorianNormalizedDate {
    pub(crate) fn from_day(day: i128) -> Self {
        let day = day - GREGORIAN_NORMALIZED_DATE_OFFSET_DAYS as i128;
        let (cycle, days_into_cycle) = day.div_mod_floor(&(GREGORIAN_CYCLE_DAYS as i128));
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
            cycle,
            century,
            quadrennium,
            year: years_into_quadrennium,
            day: days_into_year,
        }
    }

    pub(crate) fn to_day(&self) -> i128 {
        let cycle = self.cycle;
        let century = self.century as i128;
        let quadrennium = self.quadrennium as i128;
        let year = self.year as i128;
        let day = self.day as i128;
        cycle * GREGORIAN_CYCLE_DAYS as i128
            + century * GREGORIAN_CENTURY_DAYS as i128
            + quadrennium * GREGORIAN_QUADRENNIUM_DAYS as i128
            + year * GREGORIAN_YEAR_DAYS as i128
            + day
            + GREGORIAN_NORMALIZED_DATE_OFFSET_DAYS as i128
    }

    pub(crate) fn from_date(year: i128, month: u8, day: u8) -> Self {
        assert!((1..=12).contains(&month));
        assert!((1..=31).contains(&day));

        let mut year = year;
        let mut month = month - 1;
        let day = day - 1;
        if month < 2 {
            month += 12;
            year -= 1;
        }
        month -= 2;
        year -= 2000;
        let (cycle, years_into_cycle) = year.div_mod_floor(&(GREGORIAN_CYCLE_YEARS as i128));
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

    pub(crate) fn to_date(&self) -> (i128, u8, u8) {
        let mut year = 2000
            + 400 * self.cycle
            + 100 * self.century as i128
            + 4 * self.quadrennium as i128
            + self.year as i128;

        // NB: shifted so march is first. This way we don't need to care about how leap days
        // affect the month start since the leap day comes at the end of the year.
        let mut month = month_from_day_offset(self.day);
        let days_into_month = (self.day - GREGORIAN_MONTH_STARTS[month as usize]) as u8;

        // Now adjust so march is represented as month 3 instead of month 1, since we want to be based off of the Gregorian new year.
        month += 2;
        if month >= 12 {
            month -= 12;
            year += 1;
        }
        (year, month + 1, days_into_month + 1)
    }

    pub(crate) fn days_in_month(&self) -> u8 {
        let month = month_from_day_offset(self.day);
        if month < 11 {
            (GREGORIAN_MONTH_STARTS[(month + 1) as usize] - GREGORIAN_MONTH_STARTS[month as usize])
                as u8
        } else if self.is_leap_year() {
            29
        } else {
            28
        }
    }

    pub(crate) fn is_leap_year(&self) -> bool {
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
        if self.day >= 306 {
            year = (year + 1) % 400
        }

        (year % 4 == 0) && (year % 100 != 0 || year == 0)
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
    fn test_is_leap_year() {
        // 2000-03-01, the zero-point of normalized dates.
        let date = GregorianNormalizedDate::from_date(2000, 3, 1);
        assert!(date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2000, 2, 29);
        assert!(date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2000, 3, 2);
        assert!(date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2000, 1, 1);
        assert!(date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(1999, 12, 31);
        assert!(!date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2001, 3, 1);
        assert!(!date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2001, 2, 28);
        assert!(!date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2001, 3, 2);
        assert!(!date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2004, 3, 1);
        assert!(date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2004, 2, 29);
        assert!(date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(2004, 3, 2);

        let date = GregorianNormalizedDate::from_date(1900, 3, 1);
        assert!(!date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(1900, 1, 1);
        assert!(!date.is_leap_year());

        let date = GregorianNormalizedDate::from_date(1899, 12, 31);
        assert!(!date.is_leap_year());
    }

    #[test]
    fn test_month_from_day_offset() {
        // const GREGORIAN_MONTH_STARTS: [u16; 13] = [0, 31, 61, 92, 122, 153, 184, 214, 245, 275, 306, 337, 65535];
        assert_eq!(month_from_day_offset(0), 0);
        assert_eq!(month_from_day_offset(30), 0);
        assert_eq!(month_from_day_offset(31), 1);
        assert_eq!(month_from_day_offset(60), 1);
        assert_eq!(month_from_day_offset(61), 2);
        assert_eq!(month_from_day_offset(91), 2);
        assert_eq!(month_from_day_offset(92), 3);
        assert_eq!(month_from_day_offset(121), 3);
        assert_eq!(month_from_day_offset(122), 4);
        assert_eq!(month_from_day_offset(152), 4);
        assert_eq!(month_from_day_offset(153), 5);
        assert_eq!(month_from_day_offset(183), 5);
        assert_eq!(month_from_day_offset(184), 6);
        assert_eq!(month_from_day_offset(213), 6);
        assert_eq!(month_from_day_offset(214), 7);
        assert_eq!(month_from_day_offset(244), 7);
        assert_eq!(month_from_day_offset(245), 8);
        assert_eq!(month_from_day_offset(274), 8);
        assert_eq!(month_from_day_offset(275), 9);
        assert_eq!(month_from_day_offset(305), 9);
        assert_eq!(month_from_day_offset(306), 10);
        assert_eq!(month_from_day_offset(336), 10);
        assert_eq!(month_from_day_offset(337), 11);
        assert_eq!(month_from_day_offset(365), 11);
    }
}
