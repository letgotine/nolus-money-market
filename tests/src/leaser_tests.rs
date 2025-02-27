use std::collections::HashSet;

use currency::{
    lease::{Atom, Cro, Juno, Osmo},
    lpn::Usdc,
    native::Nls,
    Currency,
};
use finance::{
    coin::{Amount, Coin},
    percent::Percent,
    price::{total, total_of, Price},
};
use leaser::msg::QueryMsg;
use sdk::{
    cosmwasm_ext::Response,
    cosmwasm_std::{coin, Addr, Coin as CwCoin, DepsMut, Env, Event, MessageInfo},
    cw_multi_test::{next_block, AppResponse, ContractWrapper},
};

use crate::common::{
    cwcoin, lease as lease_mod, leaser as leaser_mod, lpp as lpp_mod, oracle as oracle_mod,
    test_case::{
        builder::BlankBuilder as TestCaseBuilder,
        response::{RemoteChain as _, ResponseWithInterChainMsgs},
        TestCase,
    },
    ADDON_OPTIMAL_INTEREST_RATE, BASE_INTEREST_RATE, USER, UTILIZATION_OPTIMAL,
};

type TheCurrency = Usdc;

#[test]
fn open_osmo_lease() {
    open_lease_impl::<Usdc, Osmo, Usdc>(true);
}

#[test]
fn open_cro_lease() {
    open_lease_impl::<Usdc, Cro, Usdc>(true);
}

#[test]
#[should_panic(expected = "Unsupported currency")]
fn open_lease_unsupported_currency_by_oracle() {
    open_lease_impl::<Usdc, Juno, Usdc>(false);
}

#[test]
#[should_panic(expected = "is not defined in the lpns currency group")]
fn init_lpp_with_unknown_currency() {
    type NotLpn = Osmo;

    TestCaseBuilder::<NotLpn>::new().init_lpp(
        None,
        BASE_INTEREST_RATE,
        UTILIZATION_OPTIMAL,
        ADDON_OPTIMAL_INTEREST_RATE,
        TestCase::DEFAULT_LPP_MIN_UTILIZATION,
    );
}

#[test]
fn open_lease_not_in_lease_currency() {
    type Lpn = Usdc;

    let lease_currency = Nls::TICKER;

    let user_addr = Addr::unchecked(USER);

    let mut test_case: TestCase<_, _, _, _, _, _, _> = TestCaseBuilder::<Lpn>::new()
        .init_lpp(
            None,
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_oracle(None)
        .init_treasury_without_dispatcher()
        .init_profit(24)
        .init_leaser()
        .into_generic();

    test_case.send_funds_from_admin(user_addr.clone(), &[cwcoin::<Lpn, _>(500)]);

    let downpayment: CwCoin = cwcoin::<Lpn, _>(3);

    let err = test_case
        .app
        .execute(
            user_addr,
            test_case.address_book.leaser().clone(),
            &leaser::msg::ExecuteMsg::OpenLease {
                currency: lease_currency.into(),
                max_ltd: None,
            },
            &[downpayment],
        )
        .unwrap_err();

    // For some reason the downcasting does not work. That is due to different TypeId-s of LeaseError and the root
    // cause stored into the err. Suppose that is a flaw of the cw-multi-test.
    // dbg!(err.root_cause().downcast_ref::<LeaseError>());
    // assert_eq!(
    //     &LeaseError::OracleError(OracleError::Std(StdError::GenericErr { msg: "".into() })),
    //     root_err
    // );

    assert!(err
        .root_cause()
        .to_string()
        .contains("which is not defined in the lease currency group"));
}

#[test]
fn open_multiple_loans() {
    type Lpn = Usdc;
    type LeaseCurrency = Atom;

    let user_addr = Addr::unchecked(USER);
    let other_user_addr = Addr::unchecked("other_user");

    let mut test_case: TestCase<_, _, _, _, _, _, _> = TestCaseBuilder::<Lpn>::new()
        .init_lpp(
            None,
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_oracle(None)
        .init_treasury_without_dispatcher()
        .init_profit(24)
        .init_leaser()
        .into_generic();

    test_case
        .send_funds_from_admin(user_addr.clone(), &[cwcoin::<Lpn, _>(500)])
        .send_funds_from_admin(other_user_addr.clone(), &[cwcoin::<Lpn, _>(50)]);

    let resp: HashSet<Addr> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.leaser().clone(),
            &QueryMsg::Leases {
                owner: user_addr.clone(),
            },
        )
        .unwrap();
    assert!(resp.is_empty());

    let mut loans = HashSet::new();
    for _ in 0..5 {
        let mut response: ResponseWithInterChainMsgs<'_, AppResponse> = test_case
            .app
            .execute(
                user_addr.clone(),
                test_case.address_book.leaser().clone(),
                &leaser::msg::ExecuteMsg::OpenLease {
                    currency: LeaseCurrency::TICKER.into(),
                    max_ltd: None,
                },
                &[cwcoin::<Lpn, _>(3)],
            )
            .unwrap();

        response.expect_register_ica(TestCase::LEASER_CONNECTION_ID, "0");

        let response: AppResponse = response.unwrap_response();

        test_case.app.update_block(next_block);

        let addr = lease_addr(&response.events);
        loans.insert(Addr::unchecked(addr));
    }

    assert_eq!(loans.len(), 5);

    let mut response = test_case
        .app
        .execute(
            other_user_addr.clone(),
            test_case.address_book.leaser().clone(),
            &leaser::msg::ExecuteMsg::OpenLease {
                currency: LeaseCurrency::TICKER.into(),
                max_ltd: None,
            },
            &[cwcoin::<Lpn, _>(30)],
        )
        .unwrap();

    response.expect_register_ica(TestCase::LEASER_CONNECTION_ID, "0");

    let response: AppResponse = response.unwrap_response();

    test_case.app.update_block(next_block);

    let user1_lease_addr = lease_addr(&response.events);

    let resp: HashSet<Addr> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.leaser().clone(),
            &QueryMsg::Leases {
                owner: other_user_addr,
            },
        )
        .unwrap();
    assert!(resp.contains(&Addr::unchecked(user1_lease_addr)));
    assert_eq!(resp.len(), 1);

    let user0_loans: HashSet<Addr> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.leaser().clone(),
            &QueryMsg::Leases { owner: user_addr },
        )
        .unwrap();
    assert_eq!(user0_loans, loans);
}

#[test]
fn test_quote() {
    type Lpn = TheCurrency;
    type Downpayment = Lpn;
    type LeaseCurrency = Osmo;

    let mut test_case: TestCase<_, _, _, _, _, _, _> = TestCaseBuilder::<Lpn>::new()
        .init_lpp(
            None,
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_oracle(None)
        .init_treasury_without_dispatcher()
        .init_profit(24)
        .init_leaser()
        .into_generic();

    test_case.send_funds_from_admin(Addr::unchecked(USER), &[cwcoin::<Lpn, _>(500)]);

    let price_lease_lpn: Price<LeaseCurrency, Lpn> = total_of(2.into()).is(1.into());
    let feeder = setup_feeder(&mut test_case);
    oracle_mod::feed_price(
        &mut test_case,
        feeder,
        Coin::<LeaseCurrency>::new(2),
        Coin::<Lpn>::new(1),
    );

    let leaser = test_case.address_book.leaser().clone();
    let downpayment = Coin::new(100);
    let borrow = Coin::<Lpn>::new(185);
    let resp = leaser_mod::query_quote::<Downpayment, LeaseCurrency>(
        &mut test_case.app,
        leaser,
        downpayment,
        None,
    );

    assert_eq!(resp.borrow.try_into(), Ok(borrow));
    assert_eq!(
        resp.total.try_into(),
        Ok(total(downpayment + borrow, price_lease_lpn.inv()))
    );

    /*   TODO: test with different time periods and amounts in LPP
     */

    assert_eq!(resp.annual_interest_rate, Percent::from_permille(94),);

    assert_eq!(resp.annual_interest_rate_margin, Percent::from_permille(30),);

    let leaser = test_case.address_book.leaser().clone();
    let resp = leaser_mod::query_quote::<Downpayment, LeaseCurrency>(
        &mut test_case.app,
        leaser,
        Coin::new(15),
        None,
    );

    assert_eq!(resp.borrow.try_into(), Ok(Coin::<Lpn>::new(27)));
    assert_eq!(
        resp.total.try_into(),
        Ok(Coin::<LeaseCurrency>::new(15 * 2 + 27 * 2))
    );
}

fn common_quote_with_conversion(downpayment: Coin<Osmo>, borrow_after_mul2: Coin<TheCurrency>) {
    type Lpn = TheCurrency;
    type LeaseCurrency = Cro;

    const LPNS: Amount = 5_000_000_000_000;
    const OSMOS: Amount = 5_000_000_000_000;
    const CROS: Amount = 5_000_000_000_000;

    const USER_ATOMS: Amount = 5_000_000_000;

    let lpp_reserve = vec![
        cwcoin::<Lpn, _>(LPNS),
        cwcoin::<Osmo, _>(OSMOS),
        cwcoin::<LeaseCurrency, _>(CROS),
    ];

    let user_reserve = cwcoin::<Atom, _>(USER_ATOMS);

    let user_addr = Addr::unchecked(USER);

    let mut test_case: TestCase<_, _, _, _, _, _, _> = TestCaseBuilder::<Lpn>::with_reserve(&{
        let mut reserve = vec![cwcoin::<Lpn, _>(1_000_000_000)];

        reserve.extend_from_slice(&lpp_reserve);

        reserve.extend_from_slice(&[user_reserve.clone()]);

        reserve
    })
    .init_lpp_with_funds(
        None,
        &[
            coin(LPNS, Lpn::BANK_SYMBOL),
            coin(OSMOS, Osmo::BANK_SYMBOL),
            coin(CROS, LeaseCurrency::BANK_SYMBOL),
        ],
        BASE_INTEREST_RATE,
        UTILIZATION_OPTIMAL,
        ADDON_OPTIMAL_INTEREST_RATE,
        TestCase::DEFAULT_LPP_MIN_UTILIZATION,
    )
    .init_time_alarms()
    .init_oracle(None)
    .init_treasury_without_dispatcher()
    .init_profit(24)
    .init_leaser()
    .into_generic();

    test_case.send_funds_from_admin(user_addr, &[user_reserve]);

    let feeder_addr = Addr::unchecked("feeder1");

    oracle_mod::add_feeder(&mut test_case, feeder_addr.as_str());

    let dpn_lpn_base = Coin::<Osmo>::new(1);
    let dpn_lpn_quote = Coin::<Lpn>::new(2);
    let dpn_lpn_price = total_of(dpn_lpn_base).is(dpn_lpn_quote);

    let lpn_asset_base = Coin::<Lpn>::new(1);
    let lpn_asset_quote = Coin::<LeaseCurrency>::new(2);
    let lpn_asset_price = total_of(lpn_asset_base).is(lpn_asset_quote);

    oracle_mod::feed_price(
        &mut test_case,
        feeder_addr.clone(),
        dpn_lpn_base,
        dpn_lpn_quote,
    );
    oracle_mod::feed_price(&mut test_case, feeder_addr, lpn_asset_quote, lpn_asset_base);

    let resp = leaser_mod::query_quote::<Osmo, LeaseCurrency>(
        &mut test_case.app,
        test_case.address_book.leaser().clone(),
        downpayment,
        None,
    );

    assert_eq!(
        resp.borrow.try_into(),
        Ok(borrow_after_mul2),
        "Borrow amount is different!"
    );
    assert_eq!(
        resp.total.try_into(),
        Ok(total(
            total(downpayment, dpn_lpn_price) + borrow_after_mul2,
            lpn_asset_price
        )),
        "Total amount is different!"
    );
}

#[test]
fn test_quote_with_conversion_100() {
    common_quote_with_conversion(Coin::new(100), Coin::new(371));
}

#[test]
fn test_quote_with_conversion_200() {
    common_quote_with_conversion(Coin::new(200), Coin::new(742));
}

#[test]
fn test_quote_with_conversion_5000() {
    common_quote_with_conversion(Coin::new(5000), Coin::new(18571));
}

#[test]
fn test_quote_fixed_rate() {
    type Lpn = TheCurrency;
    type Downpayment = Lpn;
    type LeaseCurrency = Osmo;

    let mut test_case: TestCase<_, _, _, _, _, _, _> = TestCaseBuilder::<Lpn>::new()
        .init_lpp(
            Some(
                ContractWrapper::new(
                    lpp::contract::execute,
                    lpp::contract::instantiate,
                    lpp_mod::mock_quote_query,
                )
                .with_sudo(lpp::contract::sudo),
            ),
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_oracle(None)
        .init_treasury_without_dispatcher()
        .init_profit(24)
        .init_leaser()
        .into_generic();

    let feeder = setup_feeder(&mut test_case);
    oracle_mod::feed_price(
        &mut test_case,
        feeder,
        Coin::<LeaseCurrency>::new(3),
        Coin::<Lpn>::new(1),
    );
    let resp = leaser_mod::query_quote::<Downpayment, LeaseCurrency>(
        &mut test_case.app,
        test_case.address_book.leaser().clone(),
        Coin::<Downpayment>::new(100),
        None,
    );

    assert_eq!(resp.borrow.try_into(), Ok(Coin::<Lpn>::new(185)));
    assert_eq!(
        resp.total.try_into(),
        Ok(Coin::<LeaseCurrency>::new(100 * 3 + 185 * 3))
    );

    /*   TODO: test with different time periods and amounts in LPP
        103% =
        100% lpp annual_interest_rate (when calling the test version of get_annual_interest_rate() in lpp_querier.rs)
        +
        3% margin_interest_rate of the leaser
    */

    assert_eq!(resp.annual_interest_rate, Percent::HUNDRED);

    assert_eq!(resp.annual_interest_rate_margin, Percent::from_percent(3));
}

fn setup_feeder<Dispatcher, Treasury, Profit, Leaser, Lpp, TimeAlarms>(
    test_case: &mut TestCase<Dispatcher, Treasury, Profit, Leaser, Lpp, Addr, TimeAlarms>,
) -> Addr {
    let feeder = Addr::unchecked("feeder_main");
    oracle_mod::add_feeder(test_case, &feeder);
    feeder
}

#[test]
#[should_panic(expected = "Insufficient balance")]
fn open_loans_lpp_fails() {
    type Lpn = Usdc;
    type LeaseCurrency = Atom;

    let user_addr = Addr::unchecked(USER);
    let downpayment = cwcoin::<Lpn, _>(30);

    fn mock_lpp_execute(
        deps: DepsMut<'_>,
        env: Env,
        info: MessageInfo,
        msg: lpp::msg::ExecuteMsg,
    ) -> Result<Response, lpp::error::ContractError> {
        match msg {
            lpp::msg::ExecuteMsg::OpenLoan { amount: _ } => {
                Err(lpp::error::ContractError::InsufficientBalance)
            }
            _ => Ok(lpp::contract::execute(deps, env, info, msg)?),
        }
    }

    let mut test_case: TestCase<_, _, _, _, _, _, _> = TestCaseBuilder::<Lpn>::new()
        .init_lpp(
            Some(
                ContractWrapper::new(
                    mock_lpp_execute,
                    lpp::contract::instantiate,
                    lpp::contract::query,
                )
                .with_sudo(lpp::contract::sudo),
            ),
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_oracle(None)
        .init_treasury_without_dispatcher()
        .init_profit(24)
        .init_leaser()
        .into_generic();

    test_case.send_funds_from_admin(user_addr.clone(), std::slice::from_ref(&downpayment));

    let _res: AppResponse = test_case
        .app
        .execute(
            user_addr,
            test_case.address_book.leaser().clone(),
            &leaser::msg::ExecuteMsg::OpenLease {
                currency: LeaseCurrency::TICKER.into(),
                max_ltd: None,
            },
            &[downpayment],
        )
        .unwrap()
        .unwrap_response();
}

fn open_lease_impl<Lpn, LeaseC, DownpaymentC>(feed_prices: bool)
where
    Lpn: Currency,
    LeaseC: Currency,
    DownpaymentC: Currency,
{
    let user_addr = Addr::unchecked(USER);

    let mut test_case: TestCase<_, _, _, _, _, _, _> = TestCaseBuilder::<Lpn>::with_reserve(&[
        cwcoin::<Lpn, _>(1_000_000_000),
        cwcoin::<LeaseC, _>(1_000_000_000),
        cwcoin::<DownpaymentC, _>(1_000_000_000),
    ])
    .init_lpp(
        None,
        BASE_INTEREST_RATE,
        UTILIZATION_OPTIMAL,
        ADDON_OPTIMAL_INTEREST_RATE,
        TestCase::DEFAULT_LPP_MIN_UTILIZATION,
    )
    .init_time_alarms()
    .init_oracle(None)
    .init_treasury_without_dispatcher()
    .init_profit(24)
    .init_leaser()
    .into_generic();

    test_case.send_funds_from_admin(user_addr.clone(), &[cwcoin::<DownpaymentC, _>(500)]);

    // 0 => lpp
    // 1 => time alarms
    // 2 => oracle
    // 3 => treasury
    // 4 => profit
    let leaser_addr: Addr = test_case.address_book.leaser().clone(); // 5 => leaser
    let lease_addr: Addr = Addr::unchecked("contract6"); // 6 => lease

    if feed_prices {
        oracle_mod::add_feeder(&mut test_case, user_addr.clone());

        if !currency::equal::<DownpaymentC, Lpn>() {
            oracle_mod::feed_price(
                &mut test_case,
                user_addr.clone(),
                Coin::<DownpaymentC>::new(1),
                Coin::<Lpn>::new(1),
            );
        }

        if !currency::equal::<LeaseC, Lpn>() {
            oracle_mod::feed_price(
                &mut test_case,
                user_addr.clone(),
                Coin::<LeaseC>::new(1),
                Coin::<Lpn>::new(1),
            );
        }
    }

    let downpayment: Coin<DownpaymentC> = Coin::new(40);
    let quote = leaser_mod::query_quote::<DownpaymentC, LeaseC>(
        &mut test_case.app,
        leaser_addr.clone(),
        downpayment,
        None,
    );
    let exp_borrow = TryInto::<Coin<Lpn>>::try_into(quote.borrow).unwrap();
    let exp_lease = TryInto::<Coin<LeaseC>>::try_into(quote.total).unwrap();

    let mut response: ResponseWithInterChainMsgs<'_, ()> = test_case
        .app
        .execute(
            user_addr,
            leaser_addr,
            &leaser::msg::ExecuteMsg::OpenLease {
                currency: LeaseC::TICKER.into(),
                max_ltd: None,
            },
            &[cwcoin(downpayment)],
        )
        .unwrap()
        .ignore_response();

    response.expect_register_ica(TestCase::LEASER_CONNECTION_ID, "0");

    () = response.unwrap_response();

    lease_mod::complete_initialization::<Lpn, DownpaymentC, LeaseC>(
        &mut test_case.app,
        TestCase::LEASER_CONNECTION_ID,
        lease_addr,
        downpayment,
        exp_borrow,
        exp_lease,
    );
}

fn lease_addr(events: &[Event]) -> String {
    events
        .last()
        .unwrap()
        .attributes
        .get(1)
        .unwrap()
        .value
        .clone()
}
