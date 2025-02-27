use currency::Group;
use finance::coin::CoinDTO;

use super::swap_task::{CoinVisitor, CoinsNb, IterNext, IterState, SwapTask};

pub(super) fn visit_at_index<T, V>(
    spec: &T,
    coin_index: CoinsNb,
    visitor: &mut V,
) -> Result<IterState, V::Error>
where
    T: SwapTask,
    V: CoinVisitor<Result = ()>,
{
    let mut coins_visitor = CoinsIndexVisitor(coin_index, visitor);
    spec.on_coins(&mut coins_visitor)
}

struct CoinsIndexVisitor<'a, V>(CoinsNb, &'a mut V);
impl<'a, V> CoinsIndexVisitor<'a, V> {
    fn at_coin(&self) -> bool {
        self.0 == CoinsNb::default()
    }
    fn next_coin(&mut self) {
        debug_assert!(!self.at_coin());
        self.0 -= 1;
    }
}

impl<'a, V> CoinVisitor for CoinsIndexVisitor<'a, V>
where
    V: CoinVisitor<Result = ()>,
{
    type Result = IterNext;
    type Error = V::Error;

    fn visit<G>(&mut self, coin: &CoinDTO<G>) -> Result<Self::Result, Self::Error>
    where
        G: Group,
    {
        let res = if self.at_coin() {
            self.1.visit(coin)?;
            IterNext::Stop
        } else {
            self.next_coin();
            IterNext::Continue
        };
        Ok(res)
    }
}

#[cfg(test)]
mod test {
    use currency::test::{Dai, TestCurrencies, TestExtraCurrencies, Usdc};
    use finance::coin::{Coin, CoinDTO};

    use crate::{
        coin_index::CoinsIndexVisitor,
        swap_coins::TestVisitor,
        swap_task::{CoinVisitor, IterNext},
    };

    fn coin1() -> CoinDTO<TestCurrencies> {
        Coin::<Usdc>::new(32).into()
    }

    fn coin2() -> CoinDTO<TestExtraCurrencies> {
        Coin::<Dai>::new(28).into()
    }

    #[test]
    fn visit_first_index() {
        let mut v = TestVisitor::<()>::new();
        {
            let mut v_idx = CoinsIndexVisitor(0, &mut v);
            let v_res = v_idx.visit(&coin1()).unwrap();
            assert_eq!(v_res, IterNext::Stop);
        }
        assert!(v.first_visited(coin1().amount()));
        assert!(v.second_not_visited());
    }

    #[test]
    fn visit_second_index() {
        let mut v = TestVisitor::<()>::new();
        {
            let mut v_idx = CoinsIndexVisitor(1, &mut v);
            let v_res = v_idx.visit(&coin1()).unwrap();
            assert_eq!(v_res, IterNext::Continue);
            let v_res = v_idx.visit(&coin2()).unwrap();
            assert_eq!(v_res, IterNext::Stop);
        }
        assert!(v.first_visited(coin2().amount()));
        assert!(v.second_not_visited());
    }

    #[test]
    fn visit_bigger_index() {
        let mut v = TestVisitor::<()>::new();
        {
            let mut v_idx = CoinsIndexVisitor(2, &mut v);
            let v_res = v_idx.visit(&coin1()).unwrap();
            assert_eq!(v_res, IterNext::Continue);
            let v_res = v_idx.visit(&coin2()).unwrap();
            assert_eq!(v_res, IterNext::Continue);
        }
        assert!(v.first_not_visited());
        assert!(v.second_not_visited());
    }
}
