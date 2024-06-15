use std::sync::Arc;

use zoneinfo_compiled::TZData;

use crate::zoneinfo;
use crate::zoneinfo::{load_leap_segments, LeapSecondChronology};

#[derive(Clone)]
pub struct Chronology {
    pimpl: Arc<SharedChronology>,
}

impl Chronology {
    #[inline]
    pub(crate) fn leap_seconds(&self) -> &LeapSecondChronology {
        self.pimpl.leap_seconds()
    }
}

impl Chronology {
    fn new(shared_chronology: SharedChronology) -> Self {
        Chronology {
            pimpl: Arc::new(shared_chronology),
        }
    }
}

pub(super) struct SharedChronology {
    pub(crate) tz_data: TZData,
    leap_second_smearing: bool,
    leap_seconds: LeapSecondChronology,
}

impl SharedChronology {
    fn leap_seconds(&self) -> &LeapSecondChronology {
        &self.leap_seconds
    }
}

pub fn load_chronology(time_zone: &str) -> Chronology {
    // TODO make this cache time zones, add leap smearing etc
    let tz_data = zoneinfo::load_zoneinfo(time_zone);
    let leap_smearing = false;
    let leap_seconds = load_leap_segments();
    Chronology {
        pimpl: Arc::new(SharedChronology {
            tz_data,
            leap_second_smearing: leap_smearing,
            leap_seconds: LeapSecondChronology(leap_seconds),
        }),
    }
}
