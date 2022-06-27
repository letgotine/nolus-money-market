use std::{
    fmt::Debug,
    ops::{Add, Sub},
};

use cosmwasm_std::{Timestamp, Uint128};
use serde::{Deserialize, Serialize};

use crate::{
    fraction::Fraction,
    fractionable::{Fractionable, TimeSliceable},
    ratio::Rational,
};

pub type Units = u64;

/// A more storage and compute optimal version of its counterpart in the std::time.
/// Designed to represent a timespan between cosmwasm_std::Timestamp-s.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub struct Duration(Units);
impl Duration {
    const UNITS_IN_SECOND: Units = 1000 * 1000 * 1000;
    pub const YEAR: Duration = Duration::from_nanos(365 * 24 * 60 * 60 * Duration::UNITS_IN_SECOND);

    pub const fn from_nanos(nanos: Units) -> Self {
        Self(nanos)
    }
    pub fn from_secs(secs: u32) -> Self {
        Self::from_nanos(Units::from(secs) * Duration::UNITS_IN_SECOND)
    }
    pub fn between(start: Timestamp, end: Timestamp) -> Self {
        debug_assert!(start <= end);
        Self(end.nanos() - start.nanos())
    }
    pub const fn nanos(&self) -> Units {
        self.0
    }

    pub fn annualized_slice_of<T>(&self, annual_amount: T) -> T
    where
        T: TimeSliceable,
    {
        annual_amount.safe_mul(&Rational::new(self.nanos(), Duration::YEAR.nanos()))
    }

    pub fn into_slice_per_ratio<U>(self, amount: U, annual_amount: U) -> Self
    where
        Self: Fractionable<U>,
        U: Default + PartialEq + Copy,
    {
        Rational::new(amount, annual_amount).of(self)
    }
}

impl From<Duration> for u128 {
    fn from(d: Duration) -> Self {
        d.nanos().into()
    }
}

impl From<Duration> for Uint128 {
    fn from(d: Duration) -> Self {
        u128::from(d).into()
    }
}

impl TryFrom<u128> for Duration {
    type Error = <Units as TryFrom<u128>>::Error;

    fn try_from(value: u128) -> Result<Self, Self::Error> {
        Ok(Duration::from_nanos(value.try_into()?))
    }
}

impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: Duration) -> Self::Output {
        self.plus_nanos(rhs.nanos())
    }
}

impl Sub<Duration> for Duration {
    type Output = Duration;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self::Output::from_nanos(self.nanos().add(rhs.nanos()))
    }
}
