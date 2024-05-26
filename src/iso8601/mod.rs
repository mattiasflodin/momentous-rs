pub use date_time::DateTime;
pub use date_time::DateTimeWithCarry;
pub use date_time_builder::DateTimeBuilder;

mod chronology;
mod date_time;
mod date_time_builder;
mod precision;
mod util;

const SECONDS_PER_DAY: u32 = 86_400;
const SECONDS_PER_HOUR: u16 = 3_600;
const SECONDS_PER_MINUTE: u8 = 60;
const HOURS_PER_DAY: u8 = 24;
const MINUTES_PER_HOUR: u8 = 60;
