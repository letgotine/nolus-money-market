use serde::Serialize;

use currency::Currency;
use lpp::stub::loan::LppLoan as LppLoanTrait;
use oracle::stub::Oracle as OracleTrait;
use sdk::cosmwasm_std::Timestamp;

use crate::{
    api::{opened::OngoingTrx, StateResponse},
    error::ContractError,
    lease::{with_lease::WithLease, Lease},
};

pub struct LeaseState {
    now: Timestamp,
    in_progress: Option<OngoingTrx>,
}

impl LeaseState {
    pub fn new(now: Timestamp, in_progress: Option<OngoingTrx>) -> Self {
        Self { now, in_progress }
    }
}

impl WithLease for LeaseState {
    type Output = StateResponse;

    type Error = ContractError;

    fn exec<Lpn, Asset, LppLoan, Oracle>(
        self,
        lease: Lease<Lpn, Asset, LppLoan, Oracle>,
    ) -> Result<Self::Output, Self::Error>
    where
        Lpn: Currency + Serialize,
        LppLoan: LppLoanTrait<Lpn>,
        Oracle: OracleTrait<Lpn>,
        Asset: Currency + Serialize,
    {
        Ok(StateResponse::opened_from(
            lease.state(self.now),
            self.in_progress,
        ))
    }
}
