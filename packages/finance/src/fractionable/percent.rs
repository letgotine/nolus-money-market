use crate::{
    coin::Coin,
    percent::{Percent, Units},
    ratio::Ratio,
};
use currency::Currency;

use super::{Fractionable, HigherRank};

impl<T> HigherRank<T> for u32
where
    T: Into<Self>,
{
    type Type = u64;
    type Intermediate = Self;
}

impl Fractionable<Units> for Percent {
    #[track_caller]
    fn safe_mul<R>(self, ratio: &R) -> Self
    where
        R: Ratio<Units>,
    {
        Percent::from_permille(self.units().safe_mul(ratio))
    }
}

impl<C> Fractionable<Coin<C>> for Percent
where
    C: Currency + PartialEq + Default + Copy,
{
    #[track_caller]
    fn safe_mul<F>(self, fraction: &F) -> Self
    where
        F: Ratio<Coin<C>>,
    {
        let p128: u128 = self.units().into();
        // TODO re-assess the design of Ratio ... and whether it could be > 1
        let res: Units = p128
            .safe_mul(fraction)
            .try_into()
            .expect("overflow computing a fraction of permille");
        Self::from_permille(res)
    }
}

#[cfg(test)]
mod test {
    mod percent {
        use crate::{
            fractionable::{Fractionable, HigherRank},
            percent::{Percent, Units},
        };

        #[test]
        fn safe_mul() {
            assert_eq!(
                Percent::from_permille(410 * 222222 / 1000),
                Percent::from_percent(41).safe_mul(&Percent::from_permille(222222))
            );

            let p_units: Units = 410;
            let p64: <u32 as HigherRank<u8>>::Type = p_units.into();
            let p64_res: <u32 as HigherRank<u8>>::Type = p64 * u64::from(Units::MAX) / 1000;
            let p_units_res: Units = p64_res.try_into().expect("u64 -> Units overflow");
            assert_eq!(
                Percent::from_permille(p_units_res),
                Percent::from_percent(41).safe_mul(&Percent::from_permille(Units::MAX))
            );
        }

        #[test]
        fn safe_mul_hundred_percent() {
            assert_eq!(
                Percent::from_permille(Units::MAX),
                Percent::from_percent(100).safe_mul(&Percent::from_permille(Units::MAX))
            );
            assert_eq!(
                Percent::from_percent(u16::MAX),
                Percent::from_percent(100).safe_mul(&Percent::from_percent(u16::MAX))
            );
        }

        #[test]
        #[should_panic]
        fn safe_mul_overflow() {
            Percent::from_permille(1001).safe_mul(&Percent::from_permille(Units::MAX));
        }
    }

    mod rational {
        use crate::{
            coin::Coin,
            fractionable::Fractionable,
            percent::{Percent, Units},
            ratio::Rational,
        };
        use currency::test::Nls;

        #[test]
        fn safe_mul() {
            let ratio_one = Rational::new(Coin::<Nls>::new(u128::MAX), Coin::<Nls>::new(u128::MAX));
            assert_eq!(
                Percent::from_permille(Units::MAX),
                Fractionable::<Coin<_>>::safe_mul(Percent::from_permille(Units::MAX), &ratio_one)
            );
        }
    }
}
