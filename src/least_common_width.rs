use crate::instant::Tick;

pub(crate) trait LeastCommonWidth<T: Tick>: Tick {
    type Output: Tick;
    fn least_common_width(self, other: T) -> (<Self as LeastCommonWidth<T>>::Output, <Self as LeastCommonWidth<T>>::Output);
}

macro_rules! impl_least_common_width {
    ($Smaller:ty, $Bigger:ty) => {
        impl LeastCommonWidth<$Bigger> for $Smaller {
            type Output = $Bigger;
            fn least_common_width(self, other: $Bigger) -> ($Bigger, $Bigger) {
                (self as $Bigger, other)
            }
        }
        impl LeastCommonWidth<$Smaller> for $Bigger {
            type Output = $Bigger;
            fn least_common_width(self, other: $Smaller) -> ($Bigger, $Bigger) {
                (self, other as $Bigger)
            }
        }
    };
}

macro_rules! impl_least_common_width_equal {
    ($T:ty) => {
        impl LeastCommonWidth<$T> for $T {
            type Output = $T;
            fn least_common_width(self, other: $T) -> ($T, $T) {
                (self, other)
            }
        }
    };
}

impl_least_common_width!(u8, u16);
impl_least_common_width!(u8, u32);
impl_least_common_width!(u8, u64);
impl_least_common_width!(u8, u128);
impl_least_common_width!(u8, i16);
impl_least_common_width!(u8, i32);
impl_least_common_width!(u8, i64);
impl_least_common_width!(u8, i128);

impl_least_common_width!(u16, u32);
impl_least_common_width!(u16, u64);
impl_least_common_width!(u16, u128);
impl_least_common_width!(u16, i32);
impl_least_common_width!(u16, i64);
impl_least_common_width!(u16, i128);

impl_least_common_width!(u32, u64);
impl_least_common_width!(u32, u128);
impl_least_common_width!(u32, i64);
impl_least_common_width!(u32, i128);

impl_least_common_width!(u64, u128);
impl_least_common_width!(u64, i128);

impl_least_common_width!(i8, i16);
impl_least_common_width!(i8, i32);
impl_least_common_width!(i8, i64);
impl_least_common_width!(i8, i128);

impl_least_common_width!(i16, i32);
impl_least_common_width!(i16, i64);
impl_least_common_width!(i16, i128);

impl_least_common_width!(i32, i64);
impl_least_common_width!(i32, i128);

impl_least_common_width!(i64, i128);

impl_least_common_width_equal!(u8);
impl_least_common_width_equal!(u16);
impl_least_common_width_equal!(u32);
impl_least_common_width_equal!(u64);
impl_least_common_width_equal!(u128);
impl_least_common_width_equal!(i8);
impl_least_common_width_equal!(i16);
impl_least_common_width_equal!(i32);
impl_least_common_width_equal!(i64);
impl_least_common_width_equal!(i128);
