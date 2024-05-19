use std::hash::Hash;

pub trait Scale: Clone + Copy + Ord + PartialOrd + Eq + PartialEq + Hash + Sized {
    const TICKS_PER_SECOND: u32;
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Seconds;

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Milliseconds;

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Nanoseconds;

impl Scale for Seconds {
    const TICKS_PER_SECOND: u32 = 1;
}

impl Scale for Milliseconds {
    const TICKS_PER_SECOND: u32 = 1_000;
}

impl Scale for Nanoseconds {
    const TICKS_PER_SECOND: u32 = 1_000_000_000;
}
