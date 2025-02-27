use crate::coin::{Coin, CoinDTO};
use ::currency::Currency;

pub fn funds<G, C>(amount: u128) -> CoinDTO<G>
where
    C: Currency,
{
    Coin::<C>::new(amount).into()
}

pub mod coin {
    use crate::{
        coin::{Amount, Coin, WithCoin, WithCoinResult},
        error::Error,
    };
    use currency::{equal, Currency};

    #[derive(PartialEq, Eq, Debug, Clone)]
    pub struct Expect<CExp>(pub Coin<CExp>)
    where
        CExp: Currency;

    impl<CExp> WithCoin for Expect<CExp>
    where
        CExp: Currency,
    {
        type Output = bool;

        type Error = Error;

        fn on<C>(&self, coin: Coin<C>) -> WithCoinResult<Self>
        where
            C: Currency,
        {
            Ok(equal::<CExp, C>() && Amount::from(coin) == self.0.into())
        }
    }
}
