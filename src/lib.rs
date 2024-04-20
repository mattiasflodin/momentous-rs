mod instant;
mod duration;
mod scale;
mod zoneinfo;
mod period;
mod datetime;
mod iso8601;
mod gregorian_normalized_date;
mod cursor;
mod div_rem;
mod widen;
mod least_common_width;
mod slice_cursor;
mod shared_vec_cursor;

pub use scale::{Scale, Nanoseconds};
pub use instant::{Instant, InstantNs128, InstantOutOfRange};
pub use duration::{Duration, DurationNs128};
use slice_cursor::SliceCursor;
use div_rem::ClampedDivRem;
use gregorian_normalized_date::GregorianNormalizedDate;
use least_common_width::LeastCommonWidth;


pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
