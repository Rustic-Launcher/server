//! {{name}}: {{description}}
//!
//! Provides the standard economy service (`rustic:services/economy`, the Vault idea): other
//! plugins call `Economy::balance(&player)`, `Economy::withdraw(..)` without knowing this one.
//! Balances are in minor units (cents) and persisted in the plugin's storage.
//!
//! Commands: `/balance`, `/pay <player> <amount>`, `/eco give <player> <amount>` (operators).

use rustic::config::Config;
use rustic::permissions::Permissions;
use rustic::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const BALANCES_KEY: &str = "balances";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    /// Balance of a new account, in cents.
    starting_balance: i64,
    currency_singular: String,
    currency_plural: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            starting_balance: 10_000,
            currency_singular: "coin".into(),
            currency_plural: "coins".into(),
        }
    }
}

#[rustic::plugin]
#[derive(Default)]
struct {{struct_name}} {
    settings: Settings,
    /// Account (UUID string) → balance in cents.
    balances: BTreeMap<String, i64>,
}

impl {{struct_name}} {
    fn balance_of(&self, account: Uuid) -> i64 {
        self.balances
            .get(&account.to_string())
            .copied()
            .unwrap_or(self.settings.starting_balance)
    }

    fn set_balance(&mut self, account: Uuid, cents: i64) -> ServiceResult<i64> {
        self.balances.insert(account.to_string(), cents);
        Storage::set(BALANCES_KEY, &self.balances)
            .map_err(|e| ServiceError::Refused(e.to_string()))?;
        Ok(cents)
    }

    fn money(&self, cents: i64) -> String {
        let unit = if cents.abs() == 100 {
            &self.settings.currency_singular
        } else {
            &self.settings.currency_plural
        };
        format!("{}.{:02} {unit}", cents / 100, (cents % 100).abs())
    }
}

/// `12.5` / `12` / `0.05` → cents.
fn parse_amount(text: &str) -> Option<i64> {
    let (units, cents) = match text.split_once('.') {
        Some((u, c)) if c.len() <= 2 => (u, format!("{c:0<2}")),
        Some(_) => return None,
        None => (text, "00".to_owned()),
    };
    let cents = units.parse::<i64>().ok()?.checked_mul(100)? + cents.parse::<i64>().ok()?;
    (cents > 0).then_some(cents)
}

impl Plugin for {{struct_name}} {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Permissions::declare("{{id}}.admin")
            .default(PermissionDefault::Op)
            .description("Use /eco")
            .register();
        Ok(())
    }

    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        self.settings = Config::new().version(1).load()?;
        self.balances = Storage::get_or_default(BALANCES_KEY)?;
        Ok(())
    }
}

/// The service other plugins reach through `Economy::*` (feature `provide-economy`).
impl ProvideEconomy for {{struct_name}} {
    fn currency_name(&mut self, plural: bool) -> ServiceResult<String> {
        Ok(if plural {
            self.settings.currency_plural.clone()
        } else {
            self.settings.currency_singular.clone()
        })
    }

    fn format(&mut self, amount: i64) -> ServiceResult<String> {
        Ok(self.money(amount))
    }

    fn balance(&mut self, account: Uuid) -> ServiceResult<i64> {
        Ok(self.balance_of(account))
    }

    fn deposit(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> {
        if amount < 0 {
            return Err(ServiceError::Refused("negative amount".into()));
        }
        let new = self.balance_of(account) + amount;
        self.set_balance(account, new)
    }

    fn withdraw(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> {
        let current = self.balance_of(account);
        if amount < 0 || current < amount {
            return Err(ServiceError::Refused("insufficient funds".into()));
        }
        self.set_balance(account, current - amount)
    }
}

#[rustic::handlers]
impl {{struct_name}} {
    #[command(name = "balance", aliases = ["bal"], description = "Show your balance")]
    fn balance_cmd(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let player = cmd.player()?;
        let text = self.money(self.balance_of(player.uuid()));
        cmd.reply(Text::mini(&format!("<gold>Balance: <white>{text}")));
        Ok(())
    }

    #[command(name = "pay", usage = "<target:player> <amount:word>", description = "Pay a player")]
    fn pay(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let from = cmd.player()?;
        let to: Player = cmd.arg("target")?;
        let amount = parse_amount(&cmd.arg::<String>("amount")?)
            .ok_or("The amount must be positive, like 5 or 2.50.")?;
        if to == from {
            return Err("You cannot pay yourself.".into());
        }
        self.withdraw(from.uuid(), amount)
            .map_err(|_| "You do not have enough money.")?;
        self.deposit(to.uuid(), amount)
            .map_err(|e| format!("Payment failed: {e:?}"))?;
        let text = self.money(amount);
        cmd.reply(Text::mini(&format!("<green>Paid {text}.")));
        to.send_message(Text::mini(&format!("<green>You received {text}.")));
        Ok(())
    }

    #[command(
        name = "eco",
        permission = "{{id}}.admin",
        usage = "give <target:player> <amount:word>",
        description = "Give money (administrators)"
    )]
    fn eco(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let to: Player = cmd.arg("target")?;
        let amount = parse_amount(&cmd.arg::<String>("amount")?).ok_or("Invalid amount.")?;
        let new = self
            .deposit(to.uuid(), amount)
            .map_err(|e| format!("Deposit failed: {e:?}"))?;
        cmd.reply(format!("New balance: {}", self.money(new)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_amount;

    #[test]
    fn amounts() {
        assert_eq!(parse_amount("12"), Some(1200));
        assert_eq!(parse_amount("2.5"), Some(250));
        assert_eq!(parse_amount("0.05"), Some(5));
        assert_eq!(parse_amount("0"), None);
        assert_eq!(parse_amount("1.234"), None);
        assert_eq!(parse_amount("abc"), None);
    }
}
