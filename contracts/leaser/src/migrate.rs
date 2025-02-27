use lease::api::MigrateMsg;
use platform::batch::Batch;
use sdk::cosmwasm_std::Addr;

use crate::{msg::MaxLeases, result::ContractResult};

pub struct Customer<LeaseIter> {
    customer: Addr,
    leases: LeaseIter,
}

#[derive(Default)]
#[cfg_attr(test, derive(Debug, Eq, PartialEq))]
pub struct MigrationResult {
    pub msgs: Batch,
    pub next_customer: Option<Addr>,
}

pub type MaybeCustomer<LI> = ContractResult<Customer<LI>>;

/// Builds a batch of messages for the migration of up to `max_leases`
///
/// Consumes the customers iterator to the next customer or error.
pub fn migrate_leases<I, LI>(
    mut customers: I,
    lease_code_id: u64,
    max_leases: MaxLeases,
) -> ContractResult<MigrationResult>
where
    I: Iterator<Item = MaybeCustomer<LI>>,
    LI: ExactSizeIterator<Item = Addr>,
{
    let mut msgs = MigrateBatch::new(lease_code_id, max_leases);
    customers
        .find_map(|maybe_customer| match maybe_customer {
            Ok(customer) => msgs.migrate_or_be_next(customer),
            Err(err) => Some(Err(err)),
        })
        .transpose()
        .map(|next_customer| MigrationResult {
            msgs: msgs.into(),
            next_customer,
        })
}

impl<LeaseIter> Customer<LeaseIter>
where
    LeaseIter: Iterator<Item = Addr>,
{
    pub fn from(customer: Addr, leases: LeaseIter) -> Self {
        Self { customer, leases }
    }
}

impl MigrationResult {
    pub fn try_add_msgs<F>(mut self, add_fn: F) -> ContractResult<Self>
    where
        F: FnOnce(&mut Batch) -> ContractResult<()>,
    {
        add_fn(&mut self.msgs).map(|()| self)
    }
}

struct MigrateBatch {
    new_code_id: u64,
    leases_left: MaxLeases,
    msgs: Batch,
}
impl MigrateBatch {
    fn new(new_code_id: u64, max_leases: MaxLeases) -> Self {
        Self {
            new_code_id,
            leases_left: max_leases,
            msgs: Default::default(),
        }
    }

    /// None if there is enough room for all customer's leases, otherwise return the customer
    fn migrate_or_be_next<LI>(&mut self, mut customer: Customer<LI>) -> Option<ContractResult<Addr>>
    where
        LI: ExactSizeIterator<Item = Addr>,
    {
        let maybe_leases_nb: Result<MaxLeases, _> = customer.leases.len().try_into();
        match maybe_leases_nb {
            Err(err) => Some(Err(err.into())),
            Ok(leases_nb) => {
                if let Some(left) = self.leases_left.checked_sub(leases_nb) {
                    self.leases_left = left;
                    customer.leases.find_map(|lease| {
                        self.msgs
                            .schedule_migrate_wasm_no_reply(&lease, MigrateMsg {}, self.new_code_id)
                            .map(|()| None)
                            .map_err(Into::into)
                            .transpose()
                    })
                } else {
                    Some(Ok(customer.customer))
                }
            }
        }
    }
}

impl From<MigrateBatch> for Batch {
    fn from(this: MigrateBatch) -> Self {
        this.msgs
    }
}

#[cfg(test)]
mod test {
    use std::vec::IntoIter;

    use lease::api::MigrateMsg;
    use sdk::cosmwasm_std::Addr;

    use crate::{
        migrate::{Customer, MigrationResult},
        result::ContractResult,
        ContractError,
    };

    const LEASE1: &str = "lease1";
    const LEASE21: &str = "lease21";
    const LEASE22: &str = "lease22";
    const LEASE3: &str = "lease3";
    const LEASE41: &str = "lease41";
    const LEASE42: &str = "lease42";
    const LEASE43: &str = "lease43";
    const CUSTOMER_ADDR1: &str = "customer1";
    const CUSTOMER_ADDR2: &str = "customer2";
    const CUSTOMER_ADDR3: &str = "customer3";
    const CUSTOMER_ADDR4: &str = "customer4";

    #[test]
    fn no_leases() {
        use std::array::IntoIter;
        let new_code: u64 = 242;
        let no_leases: Vec<Customer<IntoIter<Addr, 0>>> = vec![];
        assert_eq!(
            Ok(MigrationResult::default()),
            super::migrate_leases(no_leases.into_iter().map(Ok), new_code, 2)
        );
    }

    #[test]
    fn more_than_max_leases() {
        let new_code: u64 = 242;
        let lease1 = Addr::unchecked(LEASE1);
        let lease2 = Addr::unchecked(LEASE21);
        let lease3 = Addr::unchecked(LEASE22);
        let customer_addr1 = Addr::unchecked(CUSTOMER_ADDR1);
        let cust1 = Customer::from(
            customer_addr1.clone(),
            vec![lease1, lease2, lease3].into_iter(),
        );

        let customers = vec![Ok(cust1)];
        {
            let exp = MigrationResult {
                next_customer: Some(customer_addr1),
                ..Default::default()
            };
            assert_eq!(
                Ok(exp),
                super::migrate_leases(customers.into_iter(), new_code, 2)
            );
        }
    }

    #[test]
    fn paging() {
        let new_code = 242;
        let lease1 = Addr::unchecked(LEASE1);
        let lease21 = Addr::unchecked(LEASE21);
        let lease22 = Addr::unchecked(LEASE22);
        let lease3 = Addr::unchecked(LEASE3);
        let lease41 = Addr::unchecked(LEASE41);
        let lease42 = Addr::unchecked(LEASE42);
        let lease43 = Addr::unchecked(LEASE43);
        let customer_addr1 = Addr::unchecked(CUSTOMER_ADDR1);
        let customer_addr2 = Addr::unchecked(CUSTOMER_ADDR2);
        let customer_addr3 = Addr::unchecked(CUSTOMER_ADDR3);
        let customer_addr4 = Addr::unchecked(CUSTOMER_ADDR4);

        {
            let exp = MigrationResult {
                next_customer: Some(customer_addr1),
                ..Default::default()
            };
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 0)
            );
        }
        {
            let mut exp = add_expected(MigrationResult::default(), &lease1, new_code);
            exp.next_customer = Some(customer_addr2.clone());
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 1)
            );
        }
        {
            let mut exp = add_expected(MigrationResult::default(), &lease1, new_code);
            exp.next_customer = Some(customer_addr2);
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 2)
            );
        }
        {
            let exp = add_expected(MigrationResult::default(), &lease1, new_code);
            let exp = add_expected(exp, &lease21, new_code);
            let mut exp = add_expected(exp, &lease22, new_code);
            exp.next_customer = Some(customer_addr3);
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 3)
            );
        }
        {
            let exp = add_expected(MigrationResult::default(), &lease1, new_code);
            let exp = add_expected(exp, &lease21, new_code);
            let exp = add_expected(exp, &lease22, new_code);
            let mut exp = add_expected(exp, &lease3, new_code);
            exp.next_customer = Some(customer_addr4.clone());
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 4)
            );
        }
        {
            let exp = add_expected(MigrationResult::default(), &lease1, new_code);
            let exp = add_expected(exp, &lease21, new_code);
            let exp = add_expected(exp, &lease22, new_code);
            let mut exp = add_expected(exp, &lease3, new_code);
            exp.next_customer = Some(customer_addr4);
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 5)
            );
        }
        {
            let exp = add_expected(MigrationResult::default(), &lease1, new_code);
            let exp = add_expected(exp, &lease21, new_code);
            let exp = add_expected(exp, &lease22, new_code);
            let exp = add_expected(exp, &lease3, new_code);
            let exp = add_expected(exp, &lease41, new_code);
            let exp = add_expected(exp, &lease42, new_code);
            let mut exp = add_expected(exp, &lease43, new_code);
            exp.next_customer = None;
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 7)
            );
        }
    }

    #[test]
    fn err_leases() {
        let new_code = 242;
        let lease1 = Addr::unchecked("lease11");
        let lease2 = Addr::unchecked("lease12");
        let lease3 = Addr::unchecked("lease13");
        let cust1 = Customer::from(
            Addr::unchecked("customer1"),
            vec![lease1, lease2, lease3].into_iter(),
        );
        let err = "testing error";

        let customers = vec![
            Ok(cust1),
            Err(ContractError::ParseError { err: err.into() }),
        ];
        assert_eq!(
            Err(ContractError::ParseError { err: err.into() }),
            super::migrate_leases(customers.into_iter(), new_code, 3)
        );
    }

    fn add_expected(mut exp: MigrationResult, lease_addr: &Addr, new_code: u64) -> MigrationResult {
        exp.msgs
            .schedule_migrate_wasm_no_reply(lease_addr, MigrateMsg {}, new_code)
            .unwrap();
        exp
    }

    fn test_customers() -> impl Iterator<Item = ContractResult<Customer<IntoIter<Addr>>>> {
        let lease1 = Addr::unchecked(LEASE1);
        let customer_addr1 = Addr::unchecked(CUSTOMER_ADDR1);
        let cust1 = Customer::from(customer_addr1, vec![lease1].into_iter());

        let lease21 = Addr::unchecked(LEASE21);
        let lease22 = Addr::unchecked(LEASE22);
        let customer_addr2 = Addr::unchecked(CUSTOMER_ADDR2);
        let cust2 = Customer::from(customer_addr2, vec![lease21, lease22].into_iter());

        let lease3 = Addr::unchecked(LEASE3);
        let customer_addr3 = Addr::unchecked(CUSTOMER_ADDR3);
        let cust3 = Customer::from(customer_addr3, vec![lease3].into_iter());

        let lease41 = Addr::unchecked(LEASE41);
        let lease42 = Addr::unchecked(LEASE42);
        let lease43 = Addr::unchecked(LEASE43);
        let customer_addr4 = Addr::unchecked(CUSTOMER_ADDR4);
        let cust4 = Customer::from(customer_addr4, vec![lease41, lease42, lease43].into_iter());

        vec![Ok(cust1), Ok(cust2), Ok(cust3), Ok(cust4)].into_iter()
    }
}
