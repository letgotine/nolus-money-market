use currency::Group;
use finance::coin::CoinDTO;
use sdk::{
    cosmos_sdk_proto::{
        cosmos::base::v1beta1::Coin as CosmosSdkCoin, ibc::applications::transfer::v1::MsgTransfer,
        traits::TypeUrl,
    },
    cosmwasm_std::{Addr, Coin as CwCoin, Timestamp},
};

use crate::{
    coin_legacy::{self},
    denom::dex::DexMapper,
    error::Result,
    ica::HostAccount,
    trx::Transaction,
};

pub struct Sender<'c> {
    ics20_channel_at_dex: &'c str,
    sender: HostAccount,
    receiver: Addr,
    timeout: Timestamp,
    amounts: Vec<CosmosSdkCoin>,
}

impl<'c> Sender<'c> {
    pub fn new(
        ics20_channel_at_dex: &'c str,
        sender: HostAccount,
        receiver: Addr,
        timeout: Timestamp,
    ) -> Self {
        Self {
            ics20_channel_at_dex,
            sender,
            receiver,
            timeout,
            amounts: vec![],
        }
    }

    pub fn send<G>(&mut self, amount: &CoinDTO<G>) -> Result<()>
    where
        G: Group,
    {
        let cw_coin_at_dex = coin_legacy::to_cosmwasm_on_network::<G, DexMapper>(amount)?;
        self.amounts.push(into_cosmos_sdk_coin(cw_coin_at_dex));
        Ok(())
    }

    fn into_ibc_msgs(self) -> impl Iterator<Item = MsgTransfer> + 'c {
        let channel = self.ics20_channel_at_dex;
        let sender = self.sender;
        let receiver = self.receiver;
        let timeout = self.timeout;
        self.amounts
            .into_iter()
            .map(move |amount| new_msg(channel, sender.clone(), receiver.clone(), amount, timeout))
    }
}

const ICS20_PORT_AT_DEX: &str = "transfer";

fn new_msg(
    ics20_channel_at_dex: &str,
    sender: HostAccount,
    receiver: Addr,
    amount: CosmosSdkCoin,
    timeout: Timestamp,
) -> MsgTransfer {
    MsgTransfer {
        source_port: ICS20_PORT_AT_DEX.into(),
        source_channel: ics20_channel_at_dex.into(),
        token: Some(amount),
        sender: sender.into(),
        receiver: receiver.into(),
        timeout_height: None,
        timeout_timestamp: timeout.nanos(),
    }
}

fn into_cosmos_sdk_coin(cw_coin: CwCoin) -> CosmosSdkCoin {
    CosmosSdkCoin {
        amount: cw_coin.amount.into(),
        denom: cw_coin.denom,
    }
}

impl<'c> From<Sender<'c>> for Transaction {
    fn from(sender: Sender<'c>) -> Self {
        let mut trx = Self::default();
        sender
            .into_ibc_msgs()
            .for_each(|msg| trx.add_message(MsgTransfer::TYPE_URL, msg));
        trx
    }
}

#[cfg(test)]
mod test {
    use currency::{
        test::{Nls, TestExtraCurrencies, Usdc},
        Currency,
    };
    use finance::coin::{Amount, Coin};
    use sdk::{
        cosmos_sdk_proto::{
            cosmos::base::v1beta1::Coin as CosmosSdkCoin,
            ibc::applications::transfer::v1::MsgTransfer, traits::TypeUrl,
        },
        cosmwasm_std::{Addr, Timestamp},
    };

    use crate::{
        bank_ibc::remote::{new_msg, Sender},
        ica::HostAccount,
        trx::Transaction,
    };

    #[test]
    fn send() {
        let channel = "channel-1045";
        let sender = HostAccount::try_from(String::from("sender")).unwrap();
        let receiver = Addr::unchecked("receiver");
        let timeout = Timestamp::from_seconds(100);
        let mut funds_sender = Sender::new(channel, sender.clone(), receiver.clone(), timeout);

        let coin1: Coin<Nls> = 63.into();
        let coin2: Coin<Usdc> = 2.into();
        funds_sender
            .send::<TestExtraCurrencies>(&coin1.into())
            .unwrap();
        funds_sender
            .send::<TestExtraCurrencies>(&coin2.into())
            .unwrap();

        assert_eq!(Transaction::try_from(funds_sender), {
            let mut trx = Transaction::default();
            trx.add_message(
                MsgTransfer::TYPE_URL,
                new_msg(
                    channel,
                    sender.clone(),
                    receiver.clone(),
                    into_cosmos_sdk_coin(coin1),
                    timeout,
                ),
            );
            trx.add_message(
                MsgTransfer::TYPE_URL,
                new_msg(
                    channel,
                    sender,
                    receiver,
                    into_cosmos_sdk_coin(coin2),
                    timeout,
                ),
            );
            Ok(trx)
        });
    }

    fn into_cosmos_sdk_coin<C>(coin: Coin<C>) -> CosmosSdkCoin
    where
        C: Currency,
    {
        CosmosSdkCoin {
            amount: Amount::from(coin).to_string(),
            denom: C::DEX_SYMBOL.into(),
        }
    }
}
