pub use duration::{Duration, DurationNs128};
pub use instant::{Instant, InstantNs128, InstantOutOfRange};
pub use scale::{Nanoseconds, Scale};

mod cursor;
mod datetime;
mod div_rem;
mod duration;
mod gregorian_normalized_date;
mod instant;
pub mod iso8601;
mod least_common_width;
mod period;
mod scale;
mod slice_cursor;
mod widen;
mod zoneinfo;
mod gregorian;

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
