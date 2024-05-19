use std::cmp::Ordering::{Equal, Greater, Less};
use std::path::PathBuf;

use numcmp::NumCmp;
use zoneinfo_compiled::{parse, TZData};

use crate::duration::DurationS32;
use crate::instant::{InstantS32, Tick};
use crate::scale::Seconds;
use crate::{iso8601, Instant, Scale};

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
    // TODO this won't work on MacOS.
    let path = tzdir().join("right").join(name);
    let data = std::fs::read(path).expect("failed to read zoneinfo file");
    parse(data).expect("failed to parse zoneinfo file")
}

/// A segment of time that ends with a leap second adjustment on the last day.
#[derive(Debug)]
pub(crate) struct ContinuousTimeSegment {
    /// Instant at which this segment starts.
    // TODO not sure it's accurate to call this an instant since the start time depends
    // on the time zone, so it's not a single universal point in time.
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

impl ContinuousTimeSegment {
    pub(crate) fn end_day(&self) -> u32 {
        self.start_day + self.duration_days
    }

    pub(crate) fn end_instant(&self) -> Instant<i32, Seconds> {
        self.start_instant
            + DurationS32::new(self.duration_days as i32 * 86_400 + self.leap_seconds as i32)
    }
}

pub(crate) struct LeapSecondChronology(pub(crate) Vec<ContinuousTimeSegment>);

pub(crate) enum SegmentLookupResult<'a> {
    BeforeFirst(&'a ContinuousTimeSegment),
    In(&'a ContinuousTimeSegment),
    AfterLast(&'a ContinuousTimeSegment),
}

pub(crate) fn lookup_leap_second_segment_by_instant<T, S: Scale>(
    segments: &[ContinuousTimeSegment],
    instant: Instant<T, S>,
) -> SegmentLookupResult
where
    T: Tick + NumCmp<i32>,
{
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
        Ok(index) => SegmentLookupResult::In(&segments[index]),
        Err(index) => {
            if index == 0 {
                SegmentLookupResult::BeforeFirst(&segments[0])
            } else {
                assert_eq!(index, segments.len());
                SegmentLookupResult::AfterLast(&segments[index - 1])
            }
        }
    }
}

pub fn lookup_leap_second_segment_by_day(
    segments: &[ContinuousTimeSegment],
    day: i32,
) -> SegmentLookupResult {
    let search_result = segments.binary_search_by(|s| {
        if day < s.start_day as i32 {
            Greater
        } else if day < s.end_day() as i32 {
            Equal
        } else {
            Less
        }
    });
    match search_result {
        Ok(index) => SegmentLookupResult::In(&segments[index]),
        Err(index) => {
            if index == 0 {
                SegmentLookupResult::BeforeFirst(&segments[0])
            } else {
                assert_eq!(index, segments.len());
                SegmentLookupResult::AfterLast(&segments[index - 1])
            }
        }
    }
}

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
        LeapSecondChronology(load_leap_segments())
    }

    pub(crate) fn by_instant<T, S: Scale>(&self, instant: Instant<T, S>) -> SegmentLookupResult
    where
        T: Tick + NumCmp<i32>,
    {
        lookup_leap_second_segment_by_instant(&self.0, instant)
    }

    pub fn by_day(&self, day: i32) -> SegmentLookupResult {
        lookup_leap_second_segment_by_day(&self.0, day)
    }
}

pub(crate) fn load_leap_segments() -> Vec<ContinuousTimeSegment> {
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
    let mut start_instant: InstantS32 = Instant::from_ticks_since_epoch(0);
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

pub(crate) fn get_leap_second_adjustment_for_unix_timestamp(unix_timestamp: i64) -> i32 {
    // TODO can cast unix_timestamp to i32 here. If it's outside the range of the leap second array then
    // there are obviously no more leap seconds.
    // TODO we need to make sure this is cached and possibly even have special treatment for
    // the leap-second chronology (which is the same for all time zones), to reduce latency.
    // Also... iso8601 seems a bit too specific for a function like this.
    let utc = iso8601::load_chronology("UTC");
    let leap_seconds = utc.leap_seconds();

    let day = (unix_timestamp / 86_400) as i32;
    match leap_seconds.by_day(day) {
        SegmentLookupResult::BeforeFirst(_) => 0,
        SegmentLookupResult::In(segment) => segment.accumulated_leap_seconds,
        SegmentLookupResult::AfterLast(last_segment) => last_segment.accumulated_leap_seconds,
    }
}
