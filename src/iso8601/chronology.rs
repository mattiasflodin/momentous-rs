use std::convert::TryInto;
use std::sync::Arc;

use num_integer::Integer;
use num_traits::NumCast;
use numcmp::NumCmp;
use zoneinfo_compiled::TZData;

use crate::cursor::Cursor;
use crate::div_rem::ClampedDivRem;
use crate::duration::DurationS64;
use crate::gregorian_normalized_date::GregorianNormalizedDate;
use crate::instant::Tick;
use crate::iso8601::{DateTime, SECONDS_PER_DAY};
use crate::scale::Seconds;
use crate::widen::Widen;
use crate::zoneinfo::{
    get_leap_second_segments_since_day, get_leap_seconds, ContinuousTimeSegment,
};
use crate::{zoneinfo, Duration, Instant, Nanoseconds, Scale};

#[derive(Debug, Clone)]
pub struct Chronology {
    pimpl: Arc<SharedChronology>,
}

impl Chronology {
    #[deprecated]
    pub(super) fn get_leap_second_segments_since_instant(
        &self,
        instant: Instant<i128, Nanoseconds>,
    ) -> &'static [ContinuousTimeSegment] {
        self.pimpl.get_leap_second_segments_since_instant(instant)
    }

    #[deprecated]
    pub(super) fn get_leap_second_segments_since_day(
        &self,
        day: u32,
    ) -> &'static [ContinuousTimeSegment] {
        self.pimpl.get_leap_second_segments_since_day(day)
    }

    pub(super) fn leap_second_smearing(&self) -> bool {
        self.pimpl.leap_second_smearing
    }
}

impl Chronology {
    fn new(shared_chronology: SharedChronology) -> Self {
        Chronology {
            pimpl: Arc::new(shared_chronology),
        }
    }
}

impl Chronology {
    pub(super) fn get_date_time<T, S: Scale>(&self, instant: Instant<T, S>) -> DateTime
    where
        T: Tick + NumCmp<i32> + TryInto<i64>,
        i32: Widen<T>,
        i64: Widen<T>,
    {
        let leap_second_chronology = get_leap_seconds();
        let cursor = leap_second_chronology.by_instant(instant);

        let day_length = Duration::<T, Seconds>::new(
            <T as NumCast>::from(86_400i32).expect("86,400 seconds should be representable as T"),
        );

        let (day, into_day) = if cursor.at_start() {
            // There are no segments before unix epoch (instant 0). So just calculate days backwards, with each
            // day having exactly 86,400 seconds. This gives us a negative number of days.
            let segment = cursor
                .peek_next()
                .expect("at least one segment in leap second chronology");
            let segment_start: Instant<_, Seconds> = segment.start_instant.widen::<T>();
            let segment_start: Instant<_, S> = segment_start
                .extend()
                .expect("segment start instant should be representable as <T, S>");
            let duration_before_segment = segment_start - instant;
            let (days, into_day) = duration_before_segment.div_rem_ceil(day_length);
            (T::zero() - days, into_day)
        } else if let Some(segment) = cursor.current() {
            // The instant is within a known leap-second segment, so we calculate the number of days
            // since the start of the segment, with each day having exactly 86,400 seconds.
            let segment_start: Instant<_, Seconds> = segment.start_instant.widen::<T>();
            let segment_start: Instant<_, S> = segment_start
                .extend()
                .expect("segment start instant should be representable as <T, S>");
            let d = instant - segment_start;
            let (mut day, mut into_day) = d.div_rem_floor(day_length);
            if day == T::from(segment.duration_days).unwrap() {
                // The instant is on one of the leap seconds at the end of the segment, so we need
                // transfer seconds from the day into the da\y offset to get a correct day number.
                day = day - T::one();
                into_day = into_day
                    + day_length
                        .extend()
                        .expect("day length should fit in T at scale S");
            }
            (
                T::from(segment.start_day).expect("segment start day should be representable as T")
                    + day,
                into_day,
            )
        } else {
            // The instant is past the last known leap-second segment, so we calculate the number
            // of days after the last segment, with each day having exactly 86,400 seconds.
            // TODO this behavior should be parameterized. It's not clear what the correct behavior is.
            // We could
            // - Fail entirely, since it's impossible to do accurately.
            // - Assume no leap seconds after the last known segment, and use 86400 seconds per day.
            // - Calculate the earth's rotation rate and use that to predict decisions by the
            //   IERS.
            // - Use a simple quadratic model to predict leap seconds. Apparently the earth rotation
            //    slows by about 1.8 milliseconds per day
            //    (rspa.royalsocietypublishing.org/content/472/2196/20160404).
            //
            // However, it is likely that the International Telecommunication Union (ITU) will
            // decide to abolish leap seconds in the future. In fact no leap second have been added
            // since 2016. So we might not need to worry about this.
            let segment = cursor
                .peek_prev()
                .expect("at least one segment in leap second chronology");
            let segment_start: Instant<i64, Seconds> = segment.start_instant.into::<i64>();
            let segment_end = segment_start
                + DurationS64::new(SECONDS_PER_DAY as i64) * (segment.duration_days as i64)
                + DurationS64::new(segment.leap_seconds as i64);
            let segment_end: Instant<_, Seconds> = segment_end.widen::<T>();
            let segment_end: Instant<_, S> = segment_end
                .extend()
                .expect("segment end instant should be representable as <T, S>");
            let d = instant - segment_end;
            d.div_rem_floor(day_length)
        };

        let gnd = GregorianNormalizedDate::from_day(day.to_i128().unwrap());
        let (year, month, day) = gnd.to_date();

        let (hour, minute, second, millisecond, microsecond, nanosecond) =
            if self.pimpl.leap_second_smearing {
                todo!("leap second smearing")
            } else {
                let nanoseconds_into_day: Duration<i64, S> = into_day
                    .try_into()
                    .expect("nanoseconds into day should fit in i64");
                let nanoseconds_into_day: Duration<i64, Nanoseconds> = nanoseconds_into_day
                    .extend()
                    .expect("nanoseconds into day should be representable as i64");
                let nanoseconds_into_day: i64 = nanoseconds_into_day.ticks();
                let (hour, nanoseconds_into_hour) =
                    nanoseconds_into_day.clamped_div_rem(3_600_000_000_000, 23_u8);
                let (minute, nanoseconds_into_minute) =
                    nanoseconds_into_hour.clamped_div_rem(60_000_000_000, 59_u8);
                // Allow seconds to "overflow" into 60 seconds here (i.e. no clamping needed after this point)
                // to allow for the leap seconds.
                // TODO test negative leap seconds
                let (second, nanoseconds_into_second) =
                    nanoseconds_into_minute.div_rem(&1_000_000_000);
                let nanoseconds_into_second = nanoseconds_into_second as u32; // 2^30 nanoseconds per second
                let (millisecond, nanoseconds_into_millisecond) =
                    nanoseconds_into_second.div_rem(&1_000_000);
                let (microsecond, nanoseconds_into_microsecond) =
                    nanoseconds_into_millisecond.div_rem(&1_000);
                let nanosecond = nanoseconds_into_microsecond as u16;
                (
                    hour,
                    minute,
                    second as u8,
                    millisecond as u16,
                    microsecond as u16,
                    nanosecond,
                )
            };

        DateTime::builder()
            .chronology(self)
            .year(year)
            .month(month)
            .day(day)
            .hour(hour)
            .minute(minute)
            .second(second)
            .millisecond(millisecond)
            .microsecond(microsecond)
            .nanosecond(nanosecond)
            .offset_hour(0)
            .offset_minute(0)
            .build()
    }
}

#[derive(Debug)]
pub(super) struct SharedChronology {
    tz_data: TZData,
    leap_second_smearing: bool,
}

impl SharedChronology {
    pub(super) fn get_leap_second_segments_since_instant(
        &self,
        _instant: Instant<i128, Nanoseconds>,
    ) -> &'static [ContinuousTimeSegment] {
        //get_leap_second_segments_since_instant(instant)
        todo!()
    }

    pub(super) fn get_leap_second_segments_since_day(
        &self,
        day: u32,
    ) -> &'static [ContinuousTimeSegment] {
        get_leap_second_segments_since_day(day)
    }
}

pub fn load_chronology(time_zone: &str) -> Chronology {
    // TODO make this cache time zones, add leap smearing etc
    let tz_data = zoneinfo::load_zoneinfo(time_zone);
    let leap_smearing = false;
    Chronology {
        pimpl: Arc::new(SharedChronology {
            tz_data,
            leap_second_smearing: leap_smearing,
        }),
    }
}
