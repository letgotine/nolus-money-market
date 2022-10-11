use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use crate::{
    coin::CoinDTO, currency::Currency, error::Error, fractionable::HigherRank, price::Price,
};

pub mod with_base;
pub mod with_price;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PriceDTO {
    amount: CoinDTO,
    amount_quote: CoinDTO,
}

impl<C, QuoteC> TryFrom<PriceDTO> for Price<C, QuoteC>
where
    C: Currency,
    QuoteC: Currency,
{
    type Error = Error;

    fn try_from(value: PriceDTO) -> Result<Self, Self::Error> {
        Ok(Price::new(
            value.amount.try_into()?,
            value.amount_quote.try_into()?,
        ))
    }
}

impl PriceDTO {
    pub fn new(base: CoinDTO, quote: CoinDTO) -> Self {
        Self {
            amount: base,
            amount_quote: quote,
        }
    }

    pub const fn base(&self) -> &CoinDTO {
        &self.amount
    }

    pub const fn quote(&self) -> &CoinDTO {
        &self.amount_quote
    }
}

impl<C, QuoteC> From<Price<C, QuoteC>> for PriceDTO
where
    C: Currency,
    QuoteC: Currency,
{
    fn from(price: Price<C, QuoteC>) -> Self {
        Self {
            amount: price.amount.into(),
            amount_quote: price.amount_quote.into(),
        }
    }
}

impl PartialOrd for PriceDTO {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        type DoubleType = <u128 as HigherRank<u128>>::Type;

        let a: DoubleType = self.quote().amount().into();
        let d: DoubleType = other.base().amount().into();

        let b: DoubleType = self.base().amount().into();
        let c: DoubleType = other.quote().amount().into();
        (a * d).partial_cmp(&(b * c))
    }
}

pub trait WithPrice {
    type Output;
    type Error;

    fn exec<C, QuoteC>(self, _: Price<C, QuoteC>) -> Result<Self::Output, Self::Error>
    where
        C: Currency,
        QuoteC: Currency;
}

pub trait WithBase<C>
where
    C: Currency,
{
    type Output;
    type Error;

    fn exec<QuoteC>(self, _: Price<C, QuoteC>) -> Result<Self::Output, Self::Error>
    where
        QuoteC: Currency;
}
