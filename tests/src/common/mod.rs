use cosmwasm_std::{
    coins, testing::mock_env, to_binary, Addr, Binary, BlockInfo, Coin, Deps, Env, StdResult,
    Timestamp,
};
use cw_multi_test::{App, AppBuilder};
use finance::{
    currency::{Currency, Nls},
    duration::Duration,
};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[allow(dead_code)]
pub mod dispatcher_wrapper;
pub mod lease_wrapper;
#[cfg(test)]
pub mod leaser_wrapper;
#[cfg(test)]
#[allow(dead_code)]
pub mod lpp_wrapper;
pub mod oracle_wrapper;
pub mod profit_wrapper;
pub mod timealarms_wrapper;

#[cfg(test)]
pub mod test_case;
pub mod treasury_wrapper;

pub const USER: &str = "user";
pub const ADMIN: &str = "admin";
pub const NATIVE_DENOM: &str = Nls::SYMBOL;

#[derive(Serialize, Clone, Debug, PartialEq)]
struct MockResponse {}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct MockQueryMsg {}

fn mock_query(_deps: Deps, _env: Env, _msg: MockQueryMsg) -> StdResult<Binary> {
    to_binary(&MockResponse {})
}

pub fn mock_app(init_funds: &[Coin]) -> App {
    let return_time = mock_env().block.time.minus_seconds(400 * 24 * 60 * 60);

    let mock_start_block = BlockInfo {
        height: 12_345,
        time: return_time,
        chain_id: "cosmos-testnet-14002".to_string(),
    };

    let mut funds = coins(1000, NATIVE_DENOM);
    funds.append(&mut init_funds.to_vec());

    AppBuilder::new()
        .with_block(mock_start_block)
        .build(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &Addr::unchecked(ADMIN), funds)
                .unwrap();
        })
}

pub trait AppExt {
    fn time_shift(&mut self, t: Duration);
}

impl AppExt for App {
    fn time_shift(&mut self, t: Duration) {
        self.update_block(|block| {
            let ct = block.time.nanos();
            block.time = Timestamp::from_nanos(ct + t.nanos());
            block.height += 1;
        })
    }
}
