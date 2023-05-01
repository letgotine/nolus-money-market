use serde::{Deserialize, Serialize};

use platform::{
    bank,
    batch::{Batch, Emit, Emitter},
};
use sdk::cosmwasm_std::{DepsMut, Env, MessageInfo, QuerierWrapper, Timestamp};

use crate::{
    api::{ExecuteMsg, StateResponse},
    contract::{cmd::Close, Contract},
    error::ContractResult,
    event::Type,
    lease::{with_lease, LeaseDTO},
};

use super::{handler, Handler, Response};

#[derive(Serialize, Deserialize, Default)]
pub struct Closed {}

impl Closed {
    pub(super) fn enter_state(
        &self,
        lease: LeaseDTO,
        querier: &QuerierWrapper<'_>,
    ) -> ContractResult<Batch> {
        let lease_addr = lease.addr.clone();
        let lease_account = bank::account(&lease_addr, querier);
        with_lease::execute(lease, Close::new(lease_account), querier)
    }

    pub(super) fn emit_ok(&self, env: &Env, lease: &LeaseDTO) -> Emitter {
        Emitter::of_type(Type::Closed)
            .emit("id", lease.addr.clone())
            .emit_tx_info(env)
    }
}

impl Handler for Closed {
    fn execute(
        self,
        deps: &mut DepsMut<'_>,
        _env: Env,
        _info: MessageInfo,
        msg: ExecuteMsg,
    ) -> ContractResult<Response> {
        match msg {
            ExecuteMsg::Repay() => handler::err("repay", deps.api),
            ExecuteMsg::Close() => handler::err("close", deps.api),
            ExecuteMsg::PriceAlarm() | ExecuteMsg::TimeAlarm {} => super::ignore_msg(self),
        }
    }
}

impl Contract for Closed {
    fn state(
        self,
        _now: Timestamp,
        _querier: &QuerierWrapper<'_>,
    ) -> ContractResult<StateResponse> {
        Ok(StateResponse::Closed())
    }
}
