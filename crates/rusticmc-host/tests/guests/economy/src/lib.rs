//! Test guest: an in-memory economy provider (`rustic:services/economy@0.1.0`).

use rustic::prelude::*;
use std::collections::HashMap;

#[rustic::plugin]
#[derive(Default)]
struct Bank {
    balances: HashMap<(u64, u64), i64>,
}

impl Plugin for Bank {
    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}

impl ProvideEconomy for Bank {
    fn currency_name(&mut self, plural: bool) -> ServiceResult<String> {
        Ok(if plural { "coins" } else { "coin" }.to_owned())
    }

    fn format(&mut self, amount: i64) -> ServiceResult<String> {
        Ok(format!("{amount} coins"))
    }

    fn balance(&mut self, account: Uuid) -> ServiceResult<i64> {
        Ok(*self.balances.get(&(account.hi, account.lo)).unwrap_or(&0))
    }

    fn deposit(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> {
        if amount < 0 {
            panic!("negative deposit");
        }
        let b = self.balances.entry((account.hi, account.lo)).or_insert(0);
        *b += amount;
        Ok(*b)
    }

    fn withdraw(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> {
        let b = self.balances.entry((account.hi, account.lo)).or_insert(0);
        if *b < amount {
            return Err(ServiceError::Refused("insufficient funds".into()));
        }
        *b -= amount;
        Ok(*b)
    }
}
