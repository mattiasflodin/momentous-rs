use std::cmp::Ordering::{Equal, Greater, Less};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use numcmp::NumCmp;
use zoneinfo_compiled::{parse, TZData};

use crate::cursor::Cursor;
use crate::duration::DurationS32;
use crate::instant::{InstantS32, Tick};
use crate::scale::Seconds;
use crate::shared_vec_cursor::SharedVecCursor;
use crate::{Instant, Scale};

#[derive(Debug, PartialEq, Copy, Clone)]
pub(crate) struct LeapSecond {
    /// Unix timestamp at which a leap second occurs. NB: unix time stamps assume
    /// days are 86400 seconds and don't include leap seconds.
    pub unix_timestamp: i64,

    /// Number of leap seconds to be added.
    pub leap_second_count: i32,
}

// About names of zones
// https://docs.python.org/3/library/zoneinfo.html#zoneinfo.ZoneInfo.key
// "Although it is a somewhat common practice to expose these to end users,
// these values are designed to be primary keys for representing the relevant
// zones and not necessarily user-facing elements. Projects like CLDR (the
// Unicode Common Locale Data Repository) can be used to get more user-friendly
// strings from these keys.

// On windows it seems there's currently no way to get leap seconds reliably so we'd
// probably have to make some modular way of sourcing the data, and provide utility
// functions for downloading and caching the file.
// https://github.com/microsoft/STL/discussions/1624

// TODO caching and, when caching, check the modification time of the file
// in case it's been updated

fn tzdir() -> PathBuf {
    // Get the TZDIR environment variable. If it's not set, we default to /usr/share/zoneinfo.
    // We could try to be more clever here (look for the root directory that /etc/localtime points
    // to, check if /etc/zoneinfo has anything, etc), but the logic here appears to be what
    // the C library does so we probably shouldn't deviate too much from that behavior.
    std::env::var("TZDIR")
        .unwrap_or_else(|_| "/usr/share/zoneinfo".to_string())
        .into()
}

pub(crate) fn load_zoneinfo(name: &str) -> TZData {
    let path = tzdir().join("right").join(name);
    let data = std::fs::read(path).expect("failed to read zoneinfo file");
    parse(data).expect("failed to parse zoneinfo file")
}

lazy_static! {
    // TODO store expiration time and fire of a thread to refresh the cache
    // (or just load it during application start up). Need to also track
    // whether some thread is already refreshing the cache so we don't
    // fire up multiple threads. leap_seconds needs to be an Option.
    static ref LEAP_SECONDS: Mutex<LeapSecondChronology> = {
        let leap_seconds = LeapSecondChronology::load();
        Mutex::new(leap_seconds)
    };
    static ref LEAP_SECOND_SEGMENTS: Arc<Vec<ContinuousTimeSegment>> = {
        Arc::new(load_leap_segments())
    };
}

pub(crate) fn get_leap_seconds() -> LeapSecondChronology {
    let leap_seconds = LEAP_SECONDS.lock().unwrap();
    leap_seconds.clone()
}

/// A segment of time that ends with a leap second adjustment on the last day.
pub(crate) struct ContinuousTimeSegment {
    /// Instant at which this segment starts.
    pub(crate) start_instant: Instant<i32, Seconds>,
    /// Number of days since Unix epoch that this segment starts on.
    pub(crate) start_day: u32,
    /// Total length of this segment in days.
    pub(crate) duration_days: u32,
    /// Number of leap seconds that are added to UTC at the end of this segment. Note that
    /// this is signed; a negative value means the last day is shorter than 86,400 seconds.
    pub(crate) leap_seconds: i8,
    /// Accumulated number of leap seconds that have been added to UTC at the beginning of this segment.
    pub(crate) accumulated_leap_seconds: i32,
}

#[derive(Clone)]
pub(crate) struct LeapSecondChronology(Arc<Vec<ContinuousTimeSegment>>);

impl LeapSecondChronology {
    fn load() -> LeapSecondChronology {
        // On OS:es that don't have a zoneinfo directory we likely won't be able
        // to get a list of leap seconds for each time zone. Windows, for example, only
        // tracks one set of leap seconds for all time zones.
        //
        // However, all IANA time zones are based on UTC, and IERS only publishes one single
        // leap second table which is relative to UTC. So we can just use that table for all
        // time zones. In the future we will likely need to parse the leapseconds file at least
        // as a secondary strategy, because e.g. OS X does not have the "right/" directory.
        let path = tzdir().join("right/UTC");
        let data = std::fs::read(path).expect("failed to read zoneinfo file");
        let tz = parse(data).expect("failed to parse zoneinfo file");
        let mut vec: Vec<LeapSecond> = tz
            .leap_seconds
            .iter()
            .map(|ls| LeapSecond {
                unix_timestamp: ls.timestamp as i64,
                leap_second_count: ls.leap_second_count,
            })
            .collect();
        vec.sort_by(|a, b| a.unix_timestamp.cmp(&b.unix_timestamp)); // Most likely already sorted but just in case
                                                                     // TODO what's all of the above for !?
        LeapSecondChronology(Arc::new(load_leap_segments()))
    }

    pub(crate) fn by_instant<T, S: Scale>(
        &self,
        instant: Instant<T, S>,
    ) -> SharedVecCursor<ContinuousTimeSegment>
    where
        T: Tick + NumCmp<i32>,
    {
        let segments = self.0.as_slice();
        let instant: Instant<T, Seconds> = instant.floor();

        let search_result = segments.binary_search_by(|s| {
            if instant < s.start_instant {
                Greater
            } else if instant
                < s.start_instant
                    + DurationS32::new(s.duration_days as i32 * 86_400 + s.leap_seconds as i32)
            {
                Equal
            } else {
                Less
            }
        });
        match search_result {
            Ok(index) => SharedVecCursor::with_pos(&self.0, index),
            Err(index) => {
                if index == 0 {
                    SharedVecCursor::at_start(&self.0)
                } else {
                    assert_eq!(index, segments.len());
                    SharedVecCursor::at_end(&self.0)
                }
            }
        }
    }

    pub fn by_day(&self, day: i128) -> SharedVecCursor<ContinuousTimeSegment> {
        let segments = self.0.as_slice();
        let search_result = segments.binary_search_by(|s| {
            if day < s.start_day as i128 {
                Greater
            } else if day < (s.start_day + s.duration_days) as i128 {
                Equal
            } else {
                Less
            }
        });
        match search_result {
            Ok(index) => SharedVecCursor::with_pos(&self.0, index),
            Err(index) => {
                if index == 0 {
                    SharedVecCursor::at_start(&self.0)
                } else {
                    assert_eq!(index, segments.len());
                    SharedVecCursor::at_end(&self.0)
                }
            }
        }
    }
}

fn load_leap_segments() -> Vec<ContinuousTimeSegment> {
    // On OS:es that don't have a zoneinfo directory we likely won't be able
    // to get a list of leap seconds for each time zone. Windows, for example, only
    // tracks one set of leap seconds for all time zones.
    //
    // However, all IANA time zones are based on UTC, and IERS only publishes one single
    // leap second table which is relative to UTC. So we can just use that table for all
    // time zones. In the future we will likely need to parse the leapseconds file at least
    // as a secondary strategy, because e.g. OS X does not have the "right/" directory.
    let path = tzdir().join("right/UTC");
    let data = std::fs::read(path).expect("failed to read zoneinfo file");
    let tz = parse(data).expect("failed to parse zoneinfo file");

    let mut segments: Vec<ContinuousTimeSegment> = Vec::with_capacity(tz.leap_seconds.len());
    let mut start_instant: InstantS32 = Instant::new(0);
    let mut start_day = 0;
    let mut previous_leap_second_total = 0;
    for leap_second in tz.leap_seconds.iter() {
        let end_day = leap_second.timestamp as u32 / 86400;

        let leap_second_total = leap_second.leap_second_count;
        let leap_seconds = leap_second_total - previous_leap_second_total;
        assert!(leap_seconds >= i8::MIN as i32 && leap_seconds <= i8::MAX as i32);
        let leap_seconds = leap_seconds as i8;
        let duration_days = end_day - start_day;
        let duration_seconds = duration_days as i32 * 86400 + leap_seconds as i32;
        segments.push(ContinuousTimeSegment {
            start_instant,
            start_day,
            duration_days,
            leap_seconds,
            accumulated_leap_seconds: previous_leap_second_total,
        });

        start_instant = start_instant + DurationS32::new(duration_seconds);
        start_day += duration_days;
        previous_leap_second_total = leap_second_total;
    }

    segments
}

pub(crate) fn get_leap_second_adjustment(unix_timestamp: i128) -> i32 {
    // TODO can cast unix_timestamp to i32 here. If it's outside the range of the leap second array then
    // there are obviously no more leap seconds.
    let leap_seconds = get_leap_seconds();
    let day = unix_timestamp / 86400;
    let cursor = leap_seconds.by_day(day);
    if cursor.at_start() {
        0
    } else if cursor.at_end() {
        let segment = cursor
            .peek_prev()
            .expect("there should be at least one leap-second segment");
        segment.accumulated_leap_seconds
    } else {
        cursor.current().unwrap().accumulated_leap_seconds
    }
}

/*pub(crate) fn get_leap_second_segment(instant: Instant<i128, Nanoseconds>) -> Option<&'static ContinuousTimeSegment> {
    let segments = LEAP_SECOND_SEGMENTS.as_ref();
    let index = segments.partition_point(|s| s.start_instant <= instant);
    if index == 0 {
        return None;
    }
    let segment = &segments[index - 1];
    let segment_duration_seconds = segment.duration_days as i128 * 86_400 + segment.leap_seconds as i128;
    if instant < segment.start_instant + DurationNs128::from_seconds(segment_duration_seconds) {
        Some(segment)
    } else {
        None
    }
}

pub(crate) fn get_leap_second_segments_since_instant(instant: Instant<i128, Nanoseconds>) -> &'static [ContinuousTimeSegment] {
    let segments = LEAP_SECOND_SEGMENTS.as_ref();
    let index = segments.partition_point(|s| s.start_instant <= instant);
    if index == 0 {
        &segments
    } else {
        &segments[index - 1..]
    }
}
*/
pub(crate) fn get_leap_second_segments_since_day(day: u32) -> &'static [ContinuousTimeSegment] {
    let segments = LEAP_SECOND_SEGMENTS.as_ref();
    let index = segments.partition_point(|s| s.start_day <= day);
    if index == 0 {
        segments
    } else {
        &segments[index - 1..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let leap_seconds = get_leap_seconds();
        assert!(leap_seconds.0.len() > 0);
    }
}
