use finance::duration::Duration;
use platform::{batch::Batch, response};
use sdk::cosmwasm_std::{Deps, Env, QuerierWrapper, Timestamp};
use serde::{Deserialize, Serialize};
use timealarms::stub::TimeAlarmsRef;

use crate::{error::Result as DexResult, Contract, Handler, Result};

use super::{ica_connector::Enterable as EnterableT, Response};

#[derive(Serialize, Deserialize)]
pub struct EntryDelay<Enterable> {
    enterable: Enterable,
    time_alarms: TimeAlarmsRef,
}

impl<Enterable> EntryDelay<Enterable> {
    const RIGHT_AFTER_NOW: Duration = Duration::from_nanos(1);

    pub(super) fn new(enterable: Enterable, time_alarms: TimeAlarmsRef) -> Self {
        Self {
            enterable,
            time_alarms,
        }
    }
}

impl<Enterable> EnterableT for EntryDelay<Enterable> {
    fn enter(&self, _deps: Deps<'_>, env: Env) -> DexResult<Batch> {
        self.time_alarms
            .clone()
            .setup_alarm(env.block.time + Self::RIGHT_AFTER_NOW)
            .map_err(Into::into)
    }
}

impl<Enterable, R, SR> Handler for EntryDelay<Enterable>
where
    Enterable: EnterableT + Handler<Response = R, SwapResult = SR> + Into<R>,
{
    type Response = R;
    type SwapResult = SR;

    fn on_time_alarm(self, deps: Deps<'_>, env: Env) -> Result<Self> {
        let alarm_response = env.contract.address.clone();
        self.enterable
            .enter(deps, env)
            .and_then(|batch| {
                response::response_with_messages(&alarm_response, batch).map_err(Into::into)
            })
            .map(|response| Response::<Self>::from(response, self.enterable))
            .into()
    }
}

impl<Connectee> Contract for EntryDelay<Connectee>
where
    Connectee: Contract,
{
    type StateResponse = Connectee::StateResponse;

    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> Self::StateResponse {
        self.enterable.state(now, querier)
    }
}
