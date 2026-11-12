//! A plugin providing every standard service compiles against the provider world
//! (`cargo test -p rusticmc-api --features provide-permissions,provide-economy`).
#![cfg(all(feature = "provide-permissions", feature = "provide-economy"))]

use rusticmc_api::prelude::*;

#[derive(Default)]
struct Provider;

impl Plugin for Provider {
    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}

impl ProvideEconomy for Provider {
    const PRIORITY: Priority = Priority::High;
    fn currency_name(&mut self, _plural: bool) -> ServiceResult<String> {
        Ok("coin".into())
    }
    fn format(&mut self, amount: i64) -> ServiceResult<String> {
        Ok(amount.to_string())
    }
    fn balance(&mut self, _a: Uuid) -> ServiceResult<i64> {
        Ok(0)
    }
    fn deposit(&mut self, _a: Uuid, n: i64) -> ServiceResult<i64> {
        Ok(n)
    }
    fn withdraw(&mut self, _a: Uuid, _n: i64) -> ServiceResult<i64> {
        Err(ServiceError::Refused("no".into()))
    }
}

impl ProvidePermissions for Provider {
    fn check(&mut self, _p: Uuid, _n: &str) -> ServiceResult<Tristate> {
        Ok(Tristate::Undefined)
    }
    fn groups(&mut self, _p: Uuid) -> ServiceResult<Vec<String>> {
        Ok(vec![])
    }
    fn add_group(&mut self, _p: Uuid, _g: &str) -> ServiceResult<()> {
        Ok(())
    }
    fn remove_group(&mut self, _p: Uuid, _g: &str) -> ServiceResult<()> {
        Ok(())
    }
    fn set_node(&mut self, _p: Uuid, _n: &str, _v: Tristate) -> ServiceResult<()> {
        Ok(())
    }
}

impl ProvideChatMeta for Provider {}

rusticmc_api::export_plugin!(Provider);

#[test]
fn provider_exports_compile() {
    use rusticmc_api::bindings::exports::rustic::services::permission_provider::Tristate as Out;
    assert_eq!(
        rusticmc_api::services::__tristate_out(Tristate::Allow),
        Out::Allow
    );
    assert_eq!(
        rusticmc_api::services::__tristate_in(Out::Deny),
        Tristate::Deny
    );
}
