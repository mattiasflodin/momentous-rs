use num_integer::Integer;
use std::cmp::min;

use num_traits::PrimInt;

/*pub(crate) trait DivFloor : Sized {
    fn div_floor(&self, other: Self) -> Self;
}

impl<T: PrimInt> DivFloor for T {
    fn div_floor(&self, other: Self) -> Self {
        let zero = Self::zero();
        let one = Self::one();
        if *self > zero && other < zero {
            ((*self - one) / other) - one
        } else if *self < zero && other > zero {
            ((*self + one) / other) - one
        } else {
            *self / other
        }
    }
}
*/

pub(crate) trait RemFloor: Sized {
    fn rem_floor(&self, other: Self) -> Self;
}

impl<T: PrimInt> RemFloor for T {
    // TODO unit tests for this
    fn rem_floor(&self, other: Self) -> Self {
        let zero = Self::zero();
        let one = Self::one();
        if *self > zero && other < zero {
            (*self - one) % other + other + one
        } else if *self < zero && other > zero {
            (*self + one) % other + other - one
        } else {
            *self % other
        }
    }
}

pub(crate) trait DivRemCeil: Sized {
    fn div_rem_ceil(&self, other: &Self) -> (Self, Self);
}

impl<T: Integer + Copy> DivRemCeil for T {
    // TODO test this
    fn div_rem_ceil(&self, other: &Self) -> (Self, Self) {
        let zero = Self::zero();
        let one = Self::one();
        if *self > zero && *other < zero {
            let (q, r) = (*self - one).div_rem(other);
            (q - one, r + *other - one)
        } else if *self < zero && *other > zero {
            let (q, r) = (*self + one).div_rem(other);
            (q - one, r + *other + one)
        } else {
            (*self).div_rem(other)
        }
    }
}

pub(crate) trait MulDivRemFloor: Sized {
    fn mul_div_rem_floor(&self, multiplier: Self, divisor: Self) -> (Self, Self);
}

impl<T: Integer + Copy> MulDivRemFloor for T {
    fn mul_div_rem_floor(&self, multiplier: Self, divisor: Self) -> (Self, Self) {
        // We want to compute the quotient and remainder of (a*b)/c, but
        // without computing the intermediate value a*b since it might overflow.
        // So we'll rewrite it as
        //
        // (a/c)*b = A*b [with A = a/c]
        //
        // and calculate
        //
        // A = a/c = q_a + r_a
        //
        // first, where q_a is the integer quotient and r_a is the remainder.
        // Putting that into the original equation we get
        //
        // (q_a + r_a/c)*b = q_a*b + r_a*b/c = q_a*b + r_a*B [with B = b/c]
        //
        // Next, we calculate the integer quotient and remainder of
        //
        // B = b/c = q_b + r_b/c,
        //
        // giving us
        //
        // (a/c)*b = q_a*b + r_a*(q_b + r_b) = q_a*b + r_a*q_b + r_a*r_b/c.
        //
        // Both r_a and r_b are remainders from a division by c, but r_a*r_b might still
        // be larger than c, so we need to perform one final calculation
        //
        // C = r_a*r_b/c = q_c + r_c/c,
        //
        // Giving us the final result
        //
        // (a/c)*b = q_a*b + r_a*q_b + q_c + r_c/c.
        //
        // where r_c is the remainder. If any term in this calculation overflows then the result
        // would be too large no matter how we compute it, so we don't need to worry about that.
        let a = *self;
        let b = multiplier;
        let c = divisor;
        let (q_a, r_a) = a.div_mod_floor(&c);
        let (q_b, r_b) = b.div_mod_floor(&c);
        let (q_c, r_c) = (r_a * r_b).div_mod_floor(&c);
        (q_a * b + r_a * q_b + q_c, r_c)
    }
}

pub(crate) trait DivDivRemFloor: Sized {
    fn div_div_rem_floor(&self, divisor1: Self, divisor2: Self) -> (Self, Self);
}

pub(crate) trait MulDivRemCeil: Sized {
    fn mul_div_rem_ceil(&self, multiplier: Self, divisor: Self) -> (Self, Self);
}

impl<T: Integer + Copy> MulDivRemCeil for T {
    fn mul_div_rem_ceil(&self, multiplier: Self, divisor: Self) -> (Self, Self) {
        // This is exactly the same as MulDivRemFloor, but we use div_rem_ceil
        // instead of div_rem_floor.
        let a = self;
        let b = multiplier;
        let c = divisor;
        let (q_a, r_a) = a.div_rem_ceil(&c);
        let (q_b, r_b) = b.div_rem_ceil(&c);
        let (q_c, r_c) = (r_a * r_b).div_rem_ceil(&c);
        (q_a * b + r_a * q_b + q_c, r_c)
    }
}

pub(crate) trait ClampedDivRem<Q: Ord>: Sized {
    type Quotient;
    fn clamped_div_rem(self, divisor: Self, max_quotient: Q) -> (Q, Self);
}

impl<T, Q> ClampedDivRem<Q> for T
where
    T: PrimInt + TryInto<Q>,
    Q: Ord + Into<T> + Copy,
{
    type Quotient = Q;
    fn clamped_div_rem(self, divisor: T, max_quotient: Self::Quotient) -> (Self::Quotient, Self) {
        let quotient = min(self / divisor, max_quotient.into());
        let remainder = self - quotient * divisor;
        let quotient: Self::Quotient = match quotient.try_into() {
            Ok(x) => x,
            Err(_) => panic!("quotient is too large"),
        };
        (quotient, remainder)
    }
}
