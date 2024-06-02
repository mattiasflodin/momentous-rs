use std::sync::Arc;

use zoneinfo_compiled::TZData;

use crate::zoneinfo::{get_leap_second_segments_since_day, ContinuousTimeSegment};
use crate::{zoneinfo, Instant, Nanoseconds};

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
