#[derive(Debug, Clone)]
pub struct Carry {
    // TODO what's the maximum theoretical carry that you can get? Probably a lot less than these
    // types allow for.
    days: u32,
    seconds: u64,
}

impl Carry {
    pub fn is_zero(&self) -> bool {
        self.days == 0 && self.seconds == 0
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeWithCarry(crate::iso8601::DateTime, Carry);

struct DateTime {

}

impl DateTime {
    pub fn year(&self) -> u16 {
        todo!()
    }
    pub fn month(&self) -> u8 {
        todo!()
    }

    pub fn day(&self) -> u8 {
        todo!()
    }

    pub fn hour(&self) -> u8 {
        todo!()
    }

    pub fn minute(&self) -> u8 {
        todo!()
    }

    pub fn second(&self) -> u8 {
        todo!()
    }

    pub fn add_years(&self, years: i16) -> DateTimeWithCarry {
        todo!()
    }

    pub fn checked_add_years(&self, years: i16) -> Option<DateTimeWithCarry> {
        todo!()
    }

    pub fn add_months(&self, months: i32) -> DateTimeWithCarry {
        todo!()
    }

    pub fn checked_add_months(&self, months: i32) -> Option<DateTimeWithCarry> {
        todo!()
    }

    pub fn add_days(&self, days: i32) -> DateTimeWithCarry {
        todo!()
    }

    pub fn checked_add_days(&self, days: i32) -> Option<DateTimeWithCarry> {
        todo!()
    }

    pub fn add_hours(&self, hours: i32) -> DateTimeWithCarry {
        todo!()
    }

    pub fn checked_add_hours(&self, hours: i32) -> Option<DateTimeWithCarry> {
        todo!()
    }

    pub fn add_minutes(&self, minutes: i64) -> DateTimeWithCarry {
        todo!()
    }

    pub fn checked_add_minutes(&self, minutes: i64) -> Option<DateTimeWithCarry> {
        todo!()
    }

    pub fn add_seconds(&self, seconds: i64) -> Self {
        todo!()
    }

    pub fn checked_add_seconds(&self, seconds: i64) -> Option<Self> {
        todo!()
    }
}

pub fn find_easter_days(start: DateTime, end: DateTime) -> Vec<DateTime> {
    todo!()
}
