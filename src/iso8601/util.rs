use crate::div_rem::RemFloor;

const GREGORIAN_MONTH_LENGTHS_NON_LEAP_YEAR: [u8; 12] =
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

pub(super) fn is_leap_year(year: i128) -> bool {
    // Get offset into cycle first so we can do this with smaller integers.
    let year = year.rem_floor(400);
    let year = year as u16;
    year % 4 == 0 && (year % 100 != 0 || year == 0)
}

pub(super) fn days_in_month(year: i128, month: u8) -> u8 {
    assert!((1..=12).contains(&month), "Month must be in range 1-12");
    if month == 2 && is_leap_year(year) {
        29
    } else {
        GREGORIAN_MONTH_LENGTHS_NON_LEAP_YEAR[(month - 1) as usize]
    }
}
