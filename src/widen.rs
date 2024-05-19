use crate::instant::Tick;

// Widen self into T. Output is whichever is the wider of the two types.
pub trait Widen<T: Tick>: Tick {
    type Output: Tick;
    fn widen(self) -> <Self as Widen<T>>::Output;
}

macro_rules! widen {
    ($From:ty, $($To:ty),+) => {
        $(
            impl Widen<$To> for $From {
                type Output = $To;
                fn widen(self) -> $To {
                    self as $To
                }
            }
        )+
    };
}

macro_rules! widen_noop {
    ($From:ty, $($To:ty),+) => {
        $(
            impl Widen<$To> for $From {
                type Output = $From;
                fn widen(self) -> $From {
                    self
                }
            }
        )+
    }
}

widen!(u8, u16, u32, u64, u128, i16, i32, i64, i128);
widen!(u16, u32, u64, u128, i32, i64, i128);
widen!(u32, u64, u128, i64, i128);
widen!(u64, u128, i128);

widen!(i8, i16, i32, i64, i128);
widen!(i16, i32, i64, i128);
widen!(i32, i64, i128);
widen!(i64, i128);

widen_noop!(u8, u8);
widen_noop!(u16, u8, u16);
widen_noop!(u32, u8, u16, u32);
widen_noop!(u64, u8, u16, u32, u64);
widen_noop!(u128, u8, u16, u32, u64, u128);

widen_noop!(i8, i8);
widen_noop!(i16, i8, i16);
widen_noop!(i32, i8, i16, i32);
widen_noop!(i64, i8, i16, i32, i64);
widen_noop!(i128, i8, i16, i32, i64, i128);
