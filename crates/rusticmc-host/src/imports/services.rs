//! `rustic:services/*` consumer imports, routed to the provider plugin's **main** instance
//! through [`rusticmc_core::Rustic::with_plugin_instance`], and the core's
//! [`PermissionProvider`] hook implemented over the same path.
//!
//! * no provider (or provider not enabled): `no-provider`;
//! * the provider is the caller, or is executing further up the stack: `provider-failed`
//!   (re-entry is refused, warned once);
//! * the provider faults: it is disabled by the core, the caller gets `provider-failed`;
//! * called from an async instance: `refused` (providers live on the main thread).

use crate::bindings::rustic::services::{
    chat_meta as wcm, common as wco, economy as weco, permission_provider as wpp,
};
use crate::convert::{Cv, tristate_from_export, tristate_to_export};
use crate::instance::ServiceFuncs;
use crate::plugin::WasmPlugin;
use crate::state::HostState;
use rusticmc_core::model as m;
use rusticmc_core::permission::PermissionProvider;
use rusticmc_core::services::{CHAT_META_IFACE, ECONOMY_IFACE, PERMISSION_PROVIDER_IFACE};
use rusticmc_core::{CallError, PluginId, context};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock, PoisonError};
use wasmtime::Store;

type SvcResult<T> = Result<T, wco::ServiceError>;

/// A provider-side call: `None` when the component does not export the interface.
pub(crate) type ProviderCall<'a, R> = Box<
    dyn FnOnce(&ServiceFuncs, &mut Store<HostState>) -> Option<wasmtime::Result<SvcResult<R>>> + 'a,
>;

fn warn_once(key: String, message: impl FnOnce() -> String) {
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let set = WARNED.get_or_init(|| Mutex::new(HashSet::new()));
    if set
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key)
    {
        log::warn!(target: "rustic", "{} (warned once)", message());
    }
}

/// Call `iface` on `provider`'s main instance. `caller` is the consumer (re-entry check).
pub(crate) fn call_provider<R>(
    caller: Option<&PluginId>,
    provider: &str,
    iface: &'static str,
    f: ProviderCall<'_, R>,
) -> SvcResult<R> {
    if caller.is_some_and(|c| c.as_str() == provider) || context::is_executing(provider) {
        warn_once(format!("reentry:{provider}:{iface}"), || {
            format!(
                "service {iface}: provider `{provider}` is executing (it called its own service, \
                 or a chain of calls came back to it); the call fails with provider-failed"
            )
        });
        return Err(wco::ServiceError::ProviderFailed);
    }
    let rt = context::runtime().map_err(|_| wco::ServiceError::ProviderFailed)?;
    let r = rt.with_plugin_instance(provider, |inst| {
        let Some(w) = inst
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<WasmPlugin>())
        else {
            return Ok(Err(wco::ServiceError::ProviderFailed));
        };
        match w.call_service(iface, f)? {
            Some(r) => Ok(r),
            None => {
                warn_once(format!("noexport:{provider}:{iface}"), || {
                    format!(
                        "[{provider}] declared itself provider of {iface} but its component \
                         does not export it"
                    )
                });
                Ok(Err(wco::ServiceError::ProviderFailed))
            }
        }
    });
    match r {
        Ok(v) => v,
        Err(CallError::NotEnabled) => Err(wco::ServiceError::NoProvider),
        Err(CallError::Busy) => {
            warn_once(format!("busy:{provider}:{iface}"), || {
                format!("service {iface}: provider `{provider}` is busy (re-entrant call refused)")
            });
            Err(wco::ServiceError::ProviderFailed)
        }
        Err(CallError::Fault(_)) => Err(wco::ServiceError::ProviderFailed),
    }
}

impl HostState {
    fn route<R>(&mut self, iface: &'static str, f: ProviderCall<'_, R>) -> SvcResult<R> {
        if !self.is_main {
            return Err(wco::ServiceError::Refused(
                "services are only reachable from the main instance (use post-to-main)".into(),
            ));
        }
        let Some(provider) = self.async_api.service_provider(iface) else {
            return Err(wco::ServiceError::NoProvider);
        };
        call_provider(Some(&self.plugin), &provider, iface, f)
    }
}

macro_rules! route {
    ($self:ident, $iface:expr, $field:ident, $method:ident ( $($arg:expr),* )) => {
        $self.route($iface, Box::new(move |svc: &ServiceFuncs, store: &mut Store<HostState>| {
            svc.$field.as_ref().map(|g| g.$method(store $(, $arg)*))
        }))
    };
}

impl weco::Host for HostState {
    fn currency_name(&mut self, plural: bool) -> SvcResult<String> {
        route!(self, ECONOMY_IFACE, economy, call_currency_name(plural))
    }

    fn format(&mut self, amount: i64) -> SvcResult<String> {
        route!(self, ECONOMY_IFACE, economy, call_format(amount))
    }

    fn balance(&mut self, account: wco::Uuid) -> SvcResult<i64> {
        route!(self, ECONOMY_IFACE, economy, call_balance(account))
    }

    fn has(&mut self, account: wco::Uuid, amount: i64) -> SvcResult<bool> {
        route!(self, ECONOMY_IFACE, economy, call_has(account, amount))
    }

    fn deposit(&mut self, account: wco::Uuid, amount: i64) -> SvcResult<i64> {
        route!(self, ECONOMY_IFACE, economy, call_deposit(account, amount))
    }

    fn withdraw(&mut self, account: wco::Uuid, amount: i64) -> SvcResult<i64> {
        route!(self, ECONOMY_IFACE, economy, call_withdraw(account, amount))
    }
}

impl wpp::Host for HostState {
    fn check(&mut self, player: wco::Uuid, node: String) -> SvcResult<wpp::Tristate> {
        self.route(
            PERMISSION_PROVIDER_IFACE,
            Box::new(move |svc: &ServiceFuncs, store: &mut Store<HostState>| {
                svc.permission_provider.as_ref().map(|g| {
                    g.call_check(store, player, &node)
                        .map(|r| r.map(tristate_from_export))
                })
            }),
        )
    }

    fn groups(&mut self, player: wco::Uuid) -> SvcResult<Vec<String>> {
        route!(
            self,
            PERMISSION_PROVIDER_IFACE,
            permission_provider,
            call_groups(player)
        )
    }

    fn primary_group(&mut self, player: wco::Uuid) -> SvcResult<Option<String>> {
        route!(
            self,
            PERMISSION_PROVIDER_IFACE,
            permission_provider,
            call_primary_group(player)
        )
    }

    fn add_group(&mut self, player: wco::Uuid, group: String) -> SvcResult<()> {
        route!(
            self,
            PERMISSION_PROVIDER_IFACE,
            permission_provider,
            call_add_group(player, &group)
        )
    }

    fn remove_group(&mut self, player: wco::Uuid, group: String) -> SvcResult<()> {
        route!(
            self,
            PERMISSION_PROVIDER_IFACE,
            permission_provider,
            call_remove_group(player, &group)
        )
    }

    fn set_node(&mut self, player: wco::Uuid, node: String, value: wpp::Tristate) -> SvcResult<()> {
        let value = tristate_to_export(value);
        route!(
            self,
            PERMISSION_PROVIDER_IFACE,
            permission_provider,
            call_set_node(player, &node, value)
        )
    }
}

impl wcm::Host for HostState {
    fn prefix(&mut self, player: wco::Uuid) -> SvcResult<Option<String>> {
        route!(self, CHAT_META_IFACE, chat_meta, call_prefix(player))
    }

    fn suffix(&mut self, player: wco::Uuid) -> SvcResult<Option<String>> {
        route!(self, CHAT_META_IFACE, chat_meta, call_suffix(player))
    }

    fn group_prefix(&mut self, group: String) -> SvcResult<Option<String>> {
        route!(self, CHAT_META_IFACE, chat_meta, call_group_prefix(&group))
    }
}

/// The core's permission-provider hook: every permission check of the server asks the
/// provider plugin's `permission-provider.check` export (the core falls back to its default
/// engine for that lookup when this fails).
#[derive(Debug, Default)]
pub(crate) struct PermissionBridge;

impl PermissionProvider for PermissionBridge {
    fn check(
        &mut self,
        provider: &PluginId,
        player: m::Uuid,
        node: &str,
    ) -> Result<m::Tristate, m::ServiceError> {
        let caller = context::current_call().map(|c| c.plugin_id);
        let player: wco::Uuid = player.cv();
        let node = node.to_owned();
        call_provider(
            caller.as_ref(),
            provider,
            PERMISSION_PROVIDER_IFACE,
            Box::new(move |svc: &ServiceFuncs, store: &mut Store<HostState>| {
                svc.permission_provider.as_ref().map(|g| {
                    g.call_check(store, player, &node)
                        .map(|r| r.map(tristate_from_export))
                })
            }),
        )
        .map(Cv::cv)
        .map_err(Cv::cv)
    }
}
