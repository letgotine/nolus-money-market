use cosmwasm_std::{Binary, DepsMut, MessageInfo};
use serde::{Deserialize, Serialize};

use platform::batch::Batch;
use sdk::cosmwasm_std::{Deps, Env, QuerierWrapper, Timestamp};

use crate::{
    api::{dex::ConnectionParams, paid::ClosingTrx, ExecuteMsg, StateResponse},
    contract::{
        dex::DexConnectable,
        state::{self, controller, Controller, Response},
        Contract, Lease,
    },
    error::ContractResult,
    event::Type,
};

use super::transfer_in_finish::TransferInFinish;

#[derive(Serialize, Deserialize)]
pub struct TransferInInit {
    lease: Lease,
}

impl TransferInInit {
    pub(in crate::contract::state) fn new(lease: Lease) -> Self {
        Self { lease }
    }

    fn enter_state(&self, now: Timestamp) -> ContractResult<Batch> {
        let mut sender = self.lease.dex.transfer_from(now);
        sender.send(&self.lease.lease.amount)?;
        Ok(sender.into())
    }

    fn on_response(self, env: &Env, querier: &QuerierWrapper<'_>) -> ContractResult<Response> {
        TransferInFinish::from(self).try_complete(querier, env)
    }
}

impl DexConnectable for TransferInInit {
    fn dex(&self) -> &ConnectionParams {
        self.lease.dex()
    }
}

impl Controller for TransferInInit {
    fn enter(&self, _deps: Deps<'_>, env: Env) -> ContractResult<Batch> {
        self.enter_state(env.block.time)
    }

    fn execute(
        self,
        deps: &mut DepsMut<'_>,
        _env: Env,
        _info: MessageInfo,
        msg: ExecuteMsg,
    ) -> ContractResult<Response> {
        match msg {
            ExecuteMsg::Repay() => controller::err("repay", deps.api),
            ExecuteMsg::Close() => controller::err("close", deps.api),
            ExecuteMsg::PriceAlarm() => state::ignore_msg(self),
            ExecuteMsg::TimeAlarm {} => state::ignore_msg(self),
        }
    }

    fn on_response(self, _data: Binary, deps: Deps<'_>, env: Env) -> ContractResult<Response> {
        self.on_response(&env, &deps.querier)
    }

    fn on_timeout(self, deps: Deps<'_>, env: Env) -> ContractResult<Response> {
        state::on_timeout_repair_channel(self, Type::ClosingTransferIn, deps, env)
    }
}

impl Contract for TransferInInit {
    fn state(
        self,
        _now: Timestamp,
        _querier: &QuerierWrapper<'_>,
    ) -> ContractResult<StateResponse> {
        Ok(StateResponse::Paid {
            amount: self.lease.lease.amount,
            in_progress: Some(ClosingTrx::TransferInInit),
        })
    }
}

impl From<TransferInInit> for TransferInFinish {
    fn from(init: TransferInInit) -> Self {
        Self::new(init.lease)
    }
}
