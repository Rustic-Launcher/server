//! essentials-lite test fixture: an in-memory economy where every account starts at 150.

use std::collections::HashMap;

use rustic::prelude::*;

#[rustic::plugin]
#[derive(Default)]
struct TestBank {
    balances: HashMap<Uuid, i64>,
}

impl Plugin for TestBank {
    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}

impl TestBank {
    fn account(&mut self, id: Uuid) -> &mut i64 {
        self.balances.entry(id).or_insert(150)
    }
}

impl ProvideEconomy for TestBank {
    fn currency_name(&mut self, plural: bool) -> ServiceResult<String> {
        Ok(if plural { "coins" } else { "coin" }.to_owned())
    }

    fn format(&mut self, amount: i64) -> ServiceResult<String> {
        Ok(format!("{amount} coins"))
    }

    fn balance(&mut self, account: Uuid) -> ServiceResult<i64> {
        Ok(*self.account(account))
    }

    fn deposit(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> {
        let b = self.account(account);
        *b += amount;
        Ok(*b)
    }

    fn withdraw(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> {
        let b = self.account(account);
        if *b < amount {
            return Err(ServiceError::Refused("not enough coins".into()));
        }
        *b -= amount;
        Ok(*b)
    }
}
