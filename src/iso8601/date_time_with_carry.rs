use crate::iso8601::DateTime;

pub struct DateTimeWithCarry {
    date_time: DateTime,
    days_carry: u128,
    seconds_carry: u128,
    //day: i128,
    //segment_cursor: SliceCursor<ContinuousTimeSegment>,
}

impl DateTimeWithCarry {
    pub(super) fn with_days(date_time: DateTime, days: u128) -> Self {
        DateTimeWithCarry {
            date_time,
            days_carry: days,
            seconds_carry: 0,
        }
    }

    pub(super) fn with_seconds(date_time: DateTime, seconds: u128) -> Self {
        DateTimeWithCarry {
            date_time,
            days_carry: 0,
            seconds_carry: seconds,
        }
    }

    pub fn has_carry(&self) -> bool {
        self.days_carry != 0 || self.seconds_carry != 0
    }

    pub fn days_carry(&self) -> u128 {
        self.days_carry
    }

    pub fn seconds_carry(&self) -> u128 {
        self.seconds_carry
    }

    pub fn unwrap(self) -> DateTime {
        if !self.has_carry() {
            self.date_time
        } else {
            panic!("Trying to unwrap DateTimeWithCarry that has a carry")
        }
    }

    pub fn drop_carry(self) -> DateTime {
        self.date_time
    }

    pub fn apply_carry(&self) -> DateTime {
        if self.days_carry != 0 {
            todo!()
        }
        if self.seconds_carry != 0 {
            self.date_time.add_seconds(self.seconds_carry as i128)
        } else {
            self.date_time.clone()
        }
    }

    // TODO function to transfer as much carry as possible to datetime without
    // overflowing to the next component. E.g. with a second of 58 and a carry of
    // 2, the carry should be reduced to 1 and the second increased to 59.
    // I *think* there's a use case for this but that would need to be figured
    // out as well. Something about chaining multiple operations without
    // "falling behind" the actual time. It might matter when you're e.g.
    // adding a month to something that has fallen behind? I'm not sure.

    pub fn add_days(&self, days: i128) -> Self {
        let carry = self.date_time.add_days(days);
        assert_eq!(carry.days_carry, 0);
        DateTimeWithCarry {
            date_time: self.date_time.clone(),
            days_carry: self.days_carry,
            seconds_carry: self.seconds_carry + carry.seconds_carry,
        }
    }

    pub fn add_seconds(&self, seconds: i128) -> Self {
        DateTimeWithCarry {
            date_time: self.date_time.add_seconds(seconds),
            days_carry: self.days_carry,
            seconds_carry: self.seconds_carry,
        }
    }

    pub fn add_minutes(&self, minutes: i128) -> Self {
        let carry = self.date_time.add_minutes(minutes);
        DateTimeWithCarry {
            date_time: carry.date_time,
            days_carry: self.days_carry + carry.days_carry,
            seconds_carry: self.seconds_carry + carry.seconds_carry,
        }
    }

    /*fn add_seconds_mut(&mut self, seconds: u128) {
        let gnd = GregorianNormalizedDate::from_date(self.year, self.month, self.day);
        let mut day = gnd.to_day();
        let leap_seconds = get_leap_seconds();
        let mut segment_cursor = leap_seconds.by_day(day);

        let mut seconds_remaining = seconds;

        let v = vec![2, 3, 4];
        v.iter()
    }*/
}
