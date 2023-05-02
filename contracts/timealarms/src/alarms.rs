use platform::{
    contract,
    dispatcher::{AlarmsDispatcher, Id},
    message::Response as MessageResponse,
};
use sdk::cosmwasm_std::{Addr, DepsMut, Env, Storage, Timestamp};
use time_oracle::Alarms;

use crate::{
    msg::{AlarmsCount, AlarmsStatusResponse, ExecuteAlarmMsg},
    result::ContractResult,
    ContractError,
};

pub struct TimeAlarms {
    time_alarms: Alarms<'static>,
}

impl TimeAlarms {
    const ALARMS_NAMESPACE: &'static str = "alarms";
    const ALARMS_IDX_NAMESPACE: &'static str = "alarms_idx";
    const REPLY_ID: Id = 0;
    const EVENT_TYPE: &str = "timealarm";

    pub(super) fn new() -> Self {
        Self {
            time_alarms: Alarms::new(Self::ALARMS_NAMESPACE, Self::ALARMS_IDX_NAMESPACE),
        }
    }

    pub(super) fn remove(
        &self,
        storage: &mut dyn Storage,
        addr: Addr,
    ) -> Result<(), ContractError> {
        self.time_alarms.remove(storage, addr)?;
        Ok(())
    }

    pub(super) fn try_add(
        &self,
        deps: DepsMut<'_>,
        env: Env,
        address: Addr,
        time: Timestamp,
    ) -> ContractResult<MessageResponse> {
        if time < env.block.time {
            return Err(ContractError::InvalidAlarm(time));
        }
        contract::validate_addr(&deps.querier, &address)
            .map_err(ContractError::from)
            .and_then(|()| {
                self.time_alarms
                    .add(deps.storage, address, time)
                    .map_err(Into::into)
            })
            .map(|()| Default::default())
    }

    pub(super) fn try_notify(
        &self,
        storage: &mut dyn Storage,
        ctime: Timestamp,
        max_count: AlarmsCount,
    ) -> ContractResult<(AlarmsCount, MessageResponse)> {
        self.time_alarms
            .alarms_selection(storage, ctime)
            .take(max_count.try_into()?)
            .try_fold(
                AlarmsDispatcher::new(ExecuteAlarmMsg::TimeAlarm {}, Self::EVENT_TYPE),
                |dispatcher, alarm| {
                    dispatcher
                        .send_to(&alarm?.0, Self::REPLY_ID)
                        .map_err::<ContractError, _>(Into::into)
                },
            )
            .map(|dispatcher| (dispatcher.nb_sent(), dispatcher.into()))
    }

    pub(super) fn try_any_alarm(
        &self,
        storage: &dyn Storage,
        ctime: Timestamp,
    ) -> Result<AlarmsStatusResponse, ContractError> {
        let remaining_alarms = self
            .time_alarms
            .alarms_selection(storage, ctime)
            .next()
            .transpose()?
            .is_some();
        Ok(AlarmsStatusResponse { remaining_alarms })
    }
}

#[cfg(test)]
mod tests {
    use platform::contract;
    use sdk::cosmwasm_std::{
        testing::{self, mock_dependencies, MockQuerier},
        Addr, QuerierWrapper, Timestamp,
    };

    use crate::{alarms::TimeAlarms, ContractError};

    #[test]
    fn try_add_invalid_contract_address() {
        let mut deps = mock_dependencies();
        let mut env = testing::mock_env();
        env.block.time = Timestamp::from_seconds(0);

        let msg_sender = Addr::unchecked("some address");
        assert!(TimeAlarms::new()
            .try_add(
                deps.as_mut(),
                env.clone(),
                msg_sender.clone(),
                Timestamp::from_nanos(8)
            )
            .is_err());

        let expected_error: ContractError =
            contract::validate_addr(&deps.as_mut().querier, &msg_sender)
                .unwrap_err()
                .into();

        let result = TimeAlarms::new()
            .try_add(deps.as_mut(), env, msg_sender, Timestamp::from_nanos(8))
            .unwrap_err();

        assert_eq!(expected_error, result);
    }

    #[test]
    fn try_add_valid_contract_address() {
        let mut mock_querier = MockQuerier::default();
        mock_querier.update_wasm(contract::testing::valid_contract_handler);
        let querier = QuerierWrapper::new(&mock_querier);
        let mut deps_temp = mock_dependencies();
        let mut deps = deps_temp.as_mut();
        deps.querier = querier;
        let mut env = testing::mock_env();
        env.block.time = Timestamp::from_seconds(0);

        let msg_sender = Addr::unchecked("some address");
        assert!(TimeAlarms::new()
            .try_add(deps, env, msg_sender, Timestamp::from_nanos(4))
            .is_ok());
    }

    #[test]
    fn try_add_alarm_in_the_past() {
        let mut mock_querier = MockQuerier::default();
        mock_querier.update_wasm(contract::testing::valid_contract_handler);
        let querier = QuerierWrapper::new(&mock_querier);
        let mut deps_temp = mock_dependencies();
        let mut deps = deps_temp.as_mut();
        deps.querier = querier;

        let mut env = testing::mock_env();
        env.block.time = Timestamp::from_seconds(100);

        let msg_sender = Addr::unchecked("some address");
        TimeAlarms::new()
            .try_add(deps, env, msg_sender, Timestamp::from_nanos(4))
            .unwrap_err();
    }
}
