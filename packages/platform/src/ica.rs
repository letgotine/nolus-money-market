use std::fmt::{Display, Formatter, Result as FmtResult};

use serde::{Deserialize, Serialize};

use currency::Currency;
use finance::{coin::Coin, duration::Duration};
use sdk::neutron_sdk::bindings::msg::{IbcFee, NeutronMsg};

use crate::{
    batch::Batch,
    coin_legacy,
    error::{Error, Result},
    trx::Transaction,
};

#[cfg(not(feature = "testing"))]
use self::impl_::OpenAckVersion;
#[cfg(feature = "testing")]
pub use self::impl_::OpenAckVersion;

/// Identifier of the ICA account opened by a lease
/// It is unique for a lease and allows the support of multiple accounts per lease
const ICA_ACCOUNT_ID: &str = "0";

/// ICA Host Account
///
/// Holds the address on the ICA host network
#[derive(Clone, Serialize, Deserialize)]
pub struct HostAccount(String);

impl TryFrom<String> for HostAccount {
    type Error = Error;
    fn try_from(addr: String) -> Result<Self> {
        if addr.is_empty() {
            Err(Error::InvalidICAHostAccount())
        } else {
            Ok(Self(addr))
        }
    }
}

impl From<HostAccount> for String {
    fn from(account: HostAccount) -> Self {
        account.0
    }
}

impl Display for HostAccount {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.0.as_str())
    }
}

pub fn register_account<C>(connection: C) -> Batch
where
    C: Into<String>,
{
    let mut batch = Batch::default();
    batch.schedule_execute_no_reply(NeutronMsg::register_interchain_account(
        connection.into(),
        ICA_ACCOUNT_ID.into(),
    ));
    batch
}

pub fn parse_register_response(response: &str) -> Result<HostAccount> {
    let open_ack = serde_json_wasm::from_str::<OpenAckVersion>(response)?;
    open_ack.address.try_into()
}

pub fn submit_transaction<Conn, M, C>(
    connection: Conn,
    trx: Transaction,
    memo: M,
    timeout: Duration,
    ack_tip: Coin<C>,
    timeout_tip: Coin<C>,
) -> Batch
where
    Conn: Into<String>,
    M: Into<String>,
    C: Currency,
{
    let mut batch = Batch::default();

    batch.schedule_execute_no_reply(NeutronMsg::submit_tx(
        connection.into(),
        ICA_ACCOUNT_ID.into(),
        trx.into_msgs(),
        memo.into(),
        timeout.secs(),
        IbcFee {
            recv_fee: vec![],
            ack_fee: vec![coin_legacy::to_cosmwasm_impl(ack_tip)],
            timeout_fee: vec![coin_legacy::to_cosmwasm_impl(timeout_tip)],
        },
    ));
    batch
}

mod impl_ {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "snake_case")]
    pub struct OpenAckVersion {
        pub version: String,
        pub controller_connection_id: String,
        pub host_connection_id: String,
        pub address: String,
        pub encoding: String,
        pub tx_type: String,
    }
}
