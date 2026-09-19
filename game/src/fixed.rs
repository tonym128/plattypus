//! Fixed-point 16.16 math helpers for deterministic 60fps physics on PSX.

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub struct Fixed(pub i32);

impl Fixed {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1 << 16);
    pub const HALF: Self = Self(1 << 15);

    #[inline(always)]
    pub const fn from_int(v: i32) -> Self {
        Self(v << 16)
    }

    #[inline(always)]
    pub const fn to_int(self) -> i32 {
        self.0 >> 16
    }

    #[inline(always)]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub const fn raw(self) -> i32 {
        self.0
    }

    #[inline(always)]
    pub const fn from_fraction(num: i32, den: i32) -> Self {
        Self(((num as i64 * 65536) / den as i64) as i32)
    }

    #[inline(always)]
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }
}

impl core::ops::Add for Fixed {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl core::ops::AddAssign for Fixed {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_add(rhs.0);
    }
}

impl core::ops::Sub for Fixed {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl core::ops::SubAssign for Fixed {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0);
    }
}

impl core::ops::Mul for Fixed {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        let prod = (self.0 as i64) * (rhs.0 as i64);
        Self((prod >> 16) as i32)
    }
}

impl core::ops::Neg for Fixed {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self(-self.0)
    }
}
