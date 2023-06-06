use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use currency::{
    native::{Native, Nls},
    payment::PaymentGroup,
};
use dex::{
    Account, Enterable, Error as DexError, Handler, Response as DexResponse, Result as DexResult,
    StartLocalLocalState,
};
use finance::{
    coin::{Coin, CoinDTO, WithCoin, WithCoinResult},
    currency::{AnyVisitor, AnyVisitorResult, Currency, Group},
    duration::Duration,
};
use platform::{
    bank::{self, Aggregate, BankAccount, BankAccountView, BankStub, BankView},
    batch::Batch,
    message::Response as PlatformResponse,
    never::{safe_unwrap, Never},
};
use sdk::cosmwasm_std::{Addr, Deps, Env, QuerierWrapper, Timestamp};

use crate::{msg::ConfigResponse, profit::Profit, result::ContractResult};

use super::{
    buy_back::BuyBack, CadenceHours, Config, ConfigManagement, SetupDexHandler, State, StateEnum,
};

#[derive(Serialize, Deserialize)]
pub(super) struct Idle {
    config: Config,
    account: Account,
}

impl Idle {
    pub fn new(config: Config, account: Account) -> Self {
        Self { config, account }
    }

    fn send_nls<B>(
        &self,
        env: &Env,
        querier: &QuerierWrapper<'_>,
        account: B,
        native: Vec<CoinDTO<Native>>,
    ) -> ContractResult<PlatformResponse>
    where
        B: BankAccount,
    {
        let state_response: PlatformResponse =
            PlatformResponse::messages_only(self.enter(env.block.time, querier)?);

        let nls: Option<Coin<Nls>> = native
            .into_iter()
            .find_map(|coin_dto: CoinDTO<Native>| coin_dto.try_into().ok());

        Ok(if let Some(nls) = nls {
            Profit::transfer_nls(account, env, self.config.treasury(), nls)
                .merge_with(state_response)
        } else {
            state_response
        })
    }

    fn on_time_alarm(
        self,
        querier: &QuerierWrapper<'_>,
        env: Env,
    ) -> ContractResult<DexResponse<Self>> {
        let account: BankStub<BankView<'_>> = bank::account(&env.contract.address, querier);

        let balances: SplitCoins<Native, PaymentGroup> = account
            .balances::<PaymentGroup, _>(CoinToDTO(PhantomData, PhantomData))?
            .map(safe_unwrap)
            .unwrap_or_default();

        if balances.second.is_empty() {
            self.send_nls(&env, querier, account, balances.first).map(
                |response: PlatformResponse| DexResponse::<Self> {
                    response,
                    next_state: State(StateEnum::Idle(self)),
                },
            )
        } else {
            self.try_enter_buy_back(
                querier,
                env.contract.address,
                env.block.time,
                balances.second,
            )
        }
    }

    fn try_enter_buy_back(
        self,
        querier: &QuerierWrapper<'_>,
        profit_addr: Addr,
        now: Timestamp,
        balances: Vec<CoinDTO<PaymentGroup>>,
    ) -> ContractResult<DexResponse<Self>> {
        let state: StartLocalLocalState<BuyBack> = dex::start_local_local(BuyBack::new(
            profit_addr,
            self.config,
            self.account,
            balances,
        ));

        state
            .enter(now, querier)
            .map(|batch: Batch| DexResponse::<Self> {
                response: PlatformResponse::messages_only(batch),
                next_state: State(StateEnum::BuyBack(state.into())),
            })
            .map_err(Into::into)
    }
}

impl Enterable for Idle {
    fn enter(&self, now: Timestamp, _: &QuerierWrapper<'_>) -> Result<Batch, DexError> {
        self.config
            .time_alarms()
            .clone()
            .setup_alarm(now + Duration::from_hours(self.config.cadence_hours()))
            .map_err(DexError::TimeAlarmError)
    }
}

impl ConfigManagement for Idle {
    fn try_update_config(self, cadence_hours: CadenceHours) -> ContractResult<Self> {
        Ok(Self {
            config: self.config.update(cadence_hours),
            ..self
        })
    }

    fn try_query_config(&self) -> ContractResult<ConfigResponse> {
        Ok(ConfigResponse {
            cadence_hours: self.config.cadence_hours(),
        })
    }
}

impl Handler for Idle {
    type Response = State;
    type SwapResult = ContractResult<DexResponse<State>>;

    fn on_time_alarm(self, deps: Deps<'_>, env: Env) -> DexResult<Self> {
        DexResult::Finished(self.on_time_alarm(&deps.querier, env))
    }
}

impl SetupDexHandler for Idle {
    type State = Self;
}

pub struct BlankCoinVisitor;

impl AnyVisitor for BlankCoinVisitor {
    type Output = ();
    type Error = Never;

    fn on<C>(self) -> AnyVisitorResult<Self>
    where
        C: Currency + Serialize + DeserializeOwned,
    {
        Ok(())
    }
}

struct CoinToDTO<G1, G2>(PhantomData<G1>, PhantomData<G2>)
where
    G1: Group,
    G2: Group;

impl<G1, G2> WithCoin for CoinToDTO<G1, G2>
where
    G1: Group,
    G2: Group,
{
    type Output = SplitCoins<G1, G2>;
    type Error = Never;

    fn on<C>(&self, coin: Coin<C>) -> WithCoinResult<Self>
    where
        C: Currency,
    {
        Ok(if G1::contains::<C>() {
            SplitCoins {
                first: vec![coin.into()],
                second: Vec::new(),
            }
        } else {
            SplitCoins {
                first: Vec::new(),
                second: vec![coin.into()],
            }
        })
    }
}

struct SplitCoins<G1, G2>
where
    G1: Group,
    G2: Group,
{
    first: Vec<CoinDTO<G1>>,
    second: Vec<CoinDTO<G2>>,
}

impl<G1, G2> Default for SplitCoins<G1, G2>
where
    G1: Group,
    G2: Group,
{
    fn default() -> Self {
        Self {
            first: vec![],
            second: vec![],
        }
    }
}

impl<G1, G2> Aggregate for SplitCoins<G1, G2>
where
    G1: Group,
    G2: Group,
{
    fn aggregate(self, other: Self) -> Self
    where
        Self: Sized,
    {
        Self {
            first: self.first.aggregate(other.first),
            second: self.second.aggregate(other.second),
        }
    }
}
