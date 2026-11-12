//! Services: the standard `rustic:services` interfaces (the Vault idea) and the registry.
//!
//! **Consuming** a service needs nothing: every plugin's world imports
//! `rustic:services/{economy,permission-provider,chat-meta}@0.1.0`, which the host always
//! defines (returning [`ServiceError::NoProvider`] while no plugin provides them).
//!
//! ```ignore
//! let balance = Economy::balance(&player)?;            // ServiceResult<i64>, minor units
//! let groups = PermissionProvider::groups(&player)?;
//! let prefix = ChatMeta::prefix(&player)?.unwrap_or_default();
//! ```
//!
//! **Providing** a service: enable the SDK feature (`provide-economy`, or `provide-permissions`
//! for `permission-provider` + `chat-meta`), which makes the component target a provider world
//! (`rustic-plugin-economy`, `rustic-plugin-permissions`, or both) and export the interface, then
//! implement the matching trait on the plugin type. The SDK declares the service with
//! `services.provide` during `on-load` (main instance) at the trait's `PRIORITY`.
//!
//! ```ignore
//! // Cargo.toml: rustic = { package = "rusticmc-api", ..., features = ["provide-economy"] }
//! impl ProvideEconomy for Bank {
//!     fn currency_name(&mut self, plural: bool) -> ServiceResult<String> { Ok(if plural { "coins" } else { "coin" }.into()) }
//!     fn format(&mut self, amount: i64) -> ServiceResult<String> { Ok(format!("{}.{:02}", amount / 100, amount % 100)) }
//!     fn balance(&mut self, account: Uuid) -> ServiceResult<i64> { Ok(*self.accounts.get(&account).unwrap_or(&0)) }
//!     fn deposit(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> { ... }
//!     fn withdraw(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> { ... }
//! }
//! ```
//!
//! A provider must not call its own service through the consumer API (that would re-enter its
//! component instance).

use crate::__rt;
use crate::bindings::rustic::api::services as wit;
use crate::bindings::rustic::services::common::Uuid as WitUuid;
use crate::bindings::rustic::services::{chat_meta, economy, permission_provider};
use crate::error::{Error, Result};
use crate::host::call;
use crate::player::Player;
use crate::types::{Priority, Uuid};

pub use crate::bindings::rustic::services::common::ServiceError;
pub use crate::bindings::rustic::services::permission_provider::Tristate;

/// Result of a service call.
pub type ServiceResult<T> = std::result::Result<T, ServiceError>;

/// The service registry.
#[derive(Debug, Clone, Copy)]
pub struct Services;

impl Services {
    /// Declares this plugin as a provider of `iface` (`rustic:services/economy@0.1.0`). The
    /// provider features do this for you; call it yourself only for non-standard interfaces.
    /// No-op in the async instance.
    pub fn provide(iface: &str, priority: Priority) -> Result<()> {
        if !__rt::is_main() {
            return Ok(());
        }
        call!("services.provide", wit::provide(iface, priority)).map_err(Error::Other)
    }

    /// The id of the plugin currently providing `iface`.
    pub fn provider(iface: &str) -> Option<String> {
        call!("services.provider", wit::provider(iface))
    }
}

impl From<Uuid> for WitUuid {
    fn from(u: Uuid) -> Self {
        WitUuid { hi: u.hi, lo: u.lo }
    }
}

impl From<WitUuid> for Uuid {
    fn from(u: WitUuid) -> Self {
        Uuid { hi: u.hi, lo: u.lo }
    }
}

/// Something that names an account or player: [`Player`], [`Uuid`], or a reference to one.
pub trait Account {
    /// The UUID.
    fn account_uuid(&self) -> Uuid;
}

impl Account for Uuid {
    fn account_uuid(&self) -> Uuid {
        *self
    }
}

impl Account for Player {
    fn account_uuid(&self) -> Uuid {
        self.uuid()
    }
}

impl<A: Account + ?Sized> Account for &A {
    fn account_uuid(&self) -> Uuid {
        (**self).account_uuid()
    }
}

fn id(a: impl Account) -> WitUuid {
    a.account_uuid().into()
}

/// Human-readable text for a service error.
pub(crate) fn describe_service_error(e: &ServiceError) -> String {
    match e {
        ServiceError::NoProvider => "no plugin provides this service".to_owned(),
        ServiceError::ProviderFailed => "the service provider failed".to_owned(),
        ServiceError::Refused(m) => m.clone(),
    }
}

/// The economy service (`rustic:services/economy@0.1.0`). Amounts are in minor units (cents).
#[derive(Debug, Clone, Copy)]
pub struct Economy;

impl Economy {
    /// The interface name.
    pub const INTERFACE: &'static str = "rustic:services/economy@0.1.0";

    /// Whether a plugin provides it.
    pub fn is_available() -> bool {
        Services::provider(Self::INTERFACE).is_some()
    }

    /// The currency name.
    pub fn currency_name(plural: bool) -> ServiceResult<String> {
        call!("economy.currency-name", economy::currency_name(plural))
    }

    /// Formats an amount for display (`12.50 coins`).
    pub fn format(amount: i64) -> ServiceResult<String> {
        call!("economy.format", economy::format(amount))
    }

    /// The account's balance.
    pub fn balance(account: impl Account) -> ServiceResult<i64> {
        call!("economy.balance", economy::balance(id(account)))
    }

    /// Whether the account holds at least `amount`.
    pub fn has(account: impl Account, amount: i64) -> ServiceResult<bool> {
        call!("economy.has", economy::has(id(account), amount))
    }

    /// Adds money; returns the new balance.
    pub fn deposit(account: impl Account, amount: i64) -> ServiceResult<i64> {
        call!("economy.deposit", economy::deposit(id(account), amount))
    }

    /// Removes money (`Refused` when too low); returns the new balance.
    pub fn withdraw(account: impl Account, amount: i64) -> ServiceResult<i64> {
        call!("economy.withdraw", economy::withdraw(id(account), amount))
    }
}

/// The replaceable permission engine (`rustic:services/permission-provider@0.1.0`): groups and
/// nodes. For a plain check use [`Player::has_permission`], which goes through it anyway.
#[derive(Debug, Clone, Copy)]
pub struct PermissionProvider;

impl PermissionProvider {
    /// The interface name.
    pub const INTERFACE: &'static str = "rustic:services/permission-provider@0.1.0";

    /// Whether a plugin provides it.
    pub fn is_available() -> bool {
        Services::provider(Self::INTERFACE).is_some()
    }

    /// The provider's verdict on a node (`Undefined`: the node's default applies).
    pub fn check(player: impl Account, node: &str) -> ServiceResult<Tristate> {
        call!(
            "permission-provider.check",
            permission_provider::check(id(player), node)
        )
    }

    /// The player's groups.
    pub fn groups(player: impl Account) -> ServiceResult<Vec<String>> {
        call!(
            "permission-provider.groups",
            permission_provider::groups(id(player))
        )
    }

    /// The player's primary group.
    pub fn primary_group(player: impl Account) -> ServiceResult<Option<String>> {
        call!(
            "permission-provider.primary-group",
            permission_provider::primary_group(id(player))
        )
    }

    /// Adds the player to a group.
    pub fn add_group(player: impl Account, group: &str) -> ServiceResult<()> {
        call!(
            "permission-provider.add-group",
            permission_provider::add_group(id(player), group)
        )
    }

    /// Removes the player from a group.
    pub fn remove_group(player: impl Account, group: &str) -> ServiceResult<()> {
        call!(
            "permission-provider.remove-group",
            permission_provider::remove_group(id(player), group)
        )
    }

    /// Sets a node for the player (`Undefined` unsets it).
    pub fn set_node(player: impl Account, node: &str, value: Tristate) -> ServiceResult<()> {
        call!(
            "permission-provider.set-node",
            permission_provider::set_node(id(player), node, value)
        )
    }
}

/// Chat metadata (`rustic:services/chat-meta@0.1.0`): prefixes and suffixes for chat formats.
#[derive(Debug, Clone, Copy)]
pub struct ChatMeta;

impl ChatMeta {
    /// The interface name.
    pub const INTERFACE: &'static str = "rustic:services/chat-meta@0.1.0";

    /// Whether a plugin provides it.
    pub fn is_available() -> bool {
        Services::provider(Self::INTERFACE).is_some()
    }

    /// The player's prefix (legacy or MiniMessage text, as the provider stores it).
    pub fn prefix(player: impl Account) -> ServiceResult<Option<String>> {
        call!("chat-meta.prefix", chat_meta::prefix(id(player)))
    }

    /// The player's suffix.
    pub fn suffix(player: impl Account) -> ServiceResult<Option<String>> {
        call!("chat-meta.suffix", chat_meta::suffix(id(player)))
    }

    /// A group's prefix.
    pub fn group_prefix(group: &str) -> ServiceResult<Option<String>> {
        call!("chat-meta.group-prefix", chat_meta::group_prefix(group))
    }
}

// ---------------------------------------------------------------------------------------------
// Providing

/// Implement on the plugin type to provide `rustic:services/economy` (feature
/// `provide-economy`). Called on the instance the host routes service calls to.
pub trait ProvideEconomy: 'static {
    /// Provider priority (the highest wins; ties go to the first loaded).
    const PRIORITY: Priority = Priority::Normal;
    /// The currency name.
    fn currency_name(&mut self, plural: bool) -> ServiceResult<String>;
    /// Formats an amount (minor units).
    fn format(&mut self, amount: i64) -> ServiceResult<String>;
    /// An account's balance.
    fn balance(&mut self, account: Uuid) -> ServiceResult<i64>;
    /// Whether an account holds at least `amount` (default: compares the balance).
    fn has(&mut self, account: Uuid, amount: i64) -> ServiceResult<bool> {
        Ok(self.balance(account)? >= amount)
    }
    /// Adds money; returns the new balance.
    fn deposit(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64>;
    /// Removes money (`Refused` when too low); returns the new balance.
    fn withdraw(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64>;
}

/// Implement on the plugin type to provide `rustic:services/permission-provider` (feature
/// `provide-permissions`, together with [`ProvideChatMeta`]).
pub trait ProvidePermissions: 'static {
    /// Provider priority.
    const PRIORITY: Priority = Priority::Normal;
    /// The verdict on a node (`Undefined`: the node's default applies).
    fn check(&mut self, player: Uuid, node: &str) -> ServiceResult<Tristate>;
    /// The player's groups.
    fn groups(&mut self, player: Uuid) -> ServiceResult<Vec<String>>;
    /// The primary group (default: the first group).
    fn primary_group(&mut self, player: Uuid) -> ServiceResult<Option<String>> {
        Ok(self.groups(player)?.into_iter().next())
    }
    /// Adds the player to a group.
    fn add_group(&mut self, player: Uuid, group: &str) -> ServiceResult<()>;
    /// Removes the player from a group.
    fn remove_group(&mut self, player: Uuid, group: &str) -> ServiceResult<()>;
    /// Sets a node for the player.
    fn set_node(&mut self, player: Uuid, node: &str, value: Tristate) -> ServiceResult<()>;
}

/// Implement on the plugin type to provide `rustic:services/chat-meta` (feature
/// `provide-permissions`). Every method defaults to "no prefix/suffix", so
/// `impl ProvideChatMeta for MyPerms {}` is enough when there is none.
pub trait ProvideChatMeta: 'static {
    /// Provider priority.
    const PRIORITY: Priority = Priority::Normal;
    /// The player's prefix.
    fn prefix(&mut self, player: Uuid) -> ServiceResult<Option<String>> {
        let _ = player;
        Ok(None)
    }
    /// The player's suffix.
    fn suffix(&mut self, player: Uuid) -> ServiceResult<Option<String>> {
        let _ = player;
        Ok(None)
    }
    /// A group's prefix.
    fn group_prefix(&mut self, group: &str) -> ServiceResult<Option<String>> {
        let _ = group;
        Ok(None)
    }
}

/// Converts the consumer-side [`Tristate`] to the exported interface's own copy of the type.
#[cfg(feature = "provide-permissions")]
#[doc(hidden)]
pub fn __tristate_out(
    t: Tristate,
) -> crate::bindings::exports::rustic::services::permission_provider::Tristate {
    use crate::bindings::exports::rustic::services::permission_provider::Tristate as Out;
    match t {
        Tristate::Allow => Out::Allow,
        Tristate::Deny => Out::Deny,
        Tristate::Undefined => Out::Undefined,
    }
}

/// The reverse of [`__tristate_out`].
#[cfg(feature = "provide-permissions")]
#[doc(hidden)]
pub fn __tristate_in(
    t: crate::bindings::exports::rustic::services::permission_provider::Tristate,
) -> Tristate {
    use crate::bindings::exports::rustic::services::permission_provider::Tristate as In;
    match t {
        In::Allow => Tristate::Allow,
        In::Deny => Tristate::Deny,
        In::Undefined => Tristate::Undefined,
    }
}

/// Export glue for `permission-provider` + `chat-meta` (feature `provide-permissions`).
#[cfg(feature = "provide-permissions")]
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_permissions_guest {
    ($guest:ident, $ty:ty) => {
        impl $crate::bindings::exports::rustic::services::permission_provider::Guest for $guest {
            fn check(
                player: $crate::bindings::rustic::services::common::Uuid,
                node: ::std::string::String,
            ) -> ::core::result::Result<
                $crate::bindings::exports::rustic::services::permission_provider::Tristate,
                $crate::services::ServiceError,
            > {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvidePermissions::check(p, player.into(), &node)
                        .map($crate::services::__tristate_out)
                })
            }
            fn groups(
                player: $crate::bindings::rustic::services::common::Uuid,
            ) -> ::core::result::Result<
                ::std::vec::Vec<::std::string::String>,
                $crate::services::ServiceError,
            > {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvidePermissions::groups(p, player.into())
                })
            }
            fn primary_group(
                player: $crate::bindings::rustic::services::common::Uuid,
            ) -> ::core::result::Result<
                ::core::option::Option<::std::string::String>,
                $crate::services::ServiceError,
            > {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvidePermissions::primary_group(p, player.into())
                })
            }
            fn add_group(
                player: $crate::bindings::rustic::services::common::Uuid,
                group: ::std::string::String,
            ) -> ::core::result::Result<(), $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvidePermissions::add_group(p, player.into(), &group)
                })
            }
            fn remove_group(
                player: $crate::bindings::rustic::services::common::Uuid,
                group: ::std::string::String,
            ) -> ::core::result::Result<(), $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvidePermissions::remove_group(p, player.into(), &group)
                })
            }
            fn set_node(
                player: $crate::bindings::rustic::services::common::Uuid,
                node: ::std::string::String,
                value: $crate::bindings::exports::rustic::services::permission_provider::Tristate,
            ) -> ::core::result::Result<(), $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvidePermissions::set_node(
                        p,
                        player.into(),
                        &node,
                        $crate::services::__tristate_in(value),
                    )
                })
            }
        }

        impl $crate::bindings::exports::rustic::services::chat_meta::Guest for $guest {
            fn prefix(
                player: $crate::bindings::rustic::services::common::Uuid,
            ) -> ::core::result::Result<
                ::core::option::Option<::std::string::String>,
                $crate::services::ServiceError,
            > {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideChatMeta::prefix(p, player.into())
                })
            }
            fn suffix(
                player: $crate::bindings::rustic::services::common::Uuid,
            ) -> ::core::result::Result<
                ::core::option::Option<::std::string::String>,
                $crate::services::ServiceError,
            > {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideChatMeta::suffix(p, player.into())
                })
            }
            fn group_prefix(
                group: ::std::string::String,
            ) -> ::core::result::Result<
                ::core::option::Option<::std::string::String>,
                $crate::services::ServiceError,
            > {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideChatMeta::group_prefix(p, &group)
                })
            }
        }
    };
}

/// Without `provide-permissions`: nothing to export.
#[cfg(not(feature = "provide-permissions"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_permissions_guest {
    ($guest:ident, $ty:ty) => {};
}

/// Declares `permission-provider` and `chat-meta` during `on-load`.
#[cfg(feature = "provide-permissions")]
#[doc(hidden)]
#[macro_export]
macro_rules! __provide_permissions {
    ($ty:ty) => {
        $crate::services::Services::provide(
            $crate::services::PermissionProvider::INTERFACE,
            <$ty as $crate::services::ProvidePermissions>::PRIORITY,
        )
        .map_err(|e| e.to_string())?;
        $crate::services::Services::provide(
            $crate::services::ChatMeta::INTERFACE,
            <$ty as $crate::services::ProvideChatMeta>::PRIORITY,
        )
        .map_err(|e| e.to_string())?;
    };
}

/// Without `provide-permissions`: nothing to declare.
#[cfg(not(feature = "provide-permissions"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __provide_permissions {
    ($ty:ty) => {};
}

/// Export glue for `economy` (feature `provide-economy`).
#[cfg(feature = "provide-economy")]
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_economy_guest {
    ($guest:ident, $ty:ty) => {
        impl $crate::bindings::exports::rustic::services::economy::Guest for $guest {
            fn currency_name(
                plural: bool,
            ) -> ::core::result::Result<::std::string::String, $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideEconomy::currency_name(p, plural)
                })
            }
            fn format(
                amount: i64,
            ) -> ::core::result::Result<::std::string::String, $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideEconomy::format(p, amount)
                })
            }
            fn balance(
                account: $crate::bindings::rustic::services::common::Uuid,
            ) -> ::core::result::Result<i64, $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideEconomy::balance(p, account.into())
                })
            }
            fn has(
                account: $crate::bindings::rustic::services::common::Uuid,
                amount: i64,
            ) -> ::core::result::Result<bool, $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideEconomy::has(p, account.into(), amount)
                })
            }
            fn deposit(
                account: $crate::bindings::rustic::services::common::Uuid,
                amount: i64,
            ) -> ::core::result::Result<i64, $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideEconomy::deposit(p, account.into(), amount)
                })
            }
            fn withdraw(
                account: $crate::bindings::rustic::services::common::Uuid,
                amount: i64,
            ) -> ::core::result::Result<i64, $crate::services::ServiceError> {
                $crate::__rt::provider_call(|p: &mut $ty| {
                    $crate::services::ProvideEconomy::withdraw(p, account.into(), amount)
                })
            }
        }
    };
}

/// Without `provide-economy`: nothing to export.
#[cfg(not(feature = "provide-economy"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_economy_guest {
    ($guest:ident, $ty:ty) => {};
}

/// Declares `economy` during `on-load`.
#[cfg(feature = "provide-economy")]
#[doc(hidden)]
#[macro_export]
macro_rules! __provide_economy {
    ($ty:ty) => {
        $crate::services::Services::provide(
            $crate::services::Economy::INTERFACE,
            <$ty as $crate::services::ProvideEconomy>::PRIORITY,
        )
        .map_err(|e| e.to_string())?;
    };
}

/// Without `provide-economy`: nothing to declare.
#[cfg(not(feature = "provide-economy"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __provide_economy {
    ($ty:ty) => {};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_conversions_and_accounts() {
        let u = Uuid::from_u128(0x1234_5678_9abc_def0_1122_3344_5566_7788);
        let w: WitUuid = u.into();
        assert_eq!((w.hi, w.lo), (u.hi, u.lo));
        assert_eq!(Uuid::from(w), u);
        let p = Player::from_uuid(u);
        assert_eq!(p.account_uuid(), u);
        assert_eq!(Account::account_uuid(&&p), u);
        let by_ref: &Uuid = &u;
        assert_eq!(id(by_ref).lo, u.lo);
    }

    #[test]
    fn error_text() {
        assert_eq!(
            describe_service_error(&ServiceError::NoProvider),
            "no plugin provides this service"
        );
        assert_eq!(
            describe_service_error(&ServiceError::Refused("poor".into())),
            "poor"
        );
    }

    #[derive(Default)]
    struct Bank(i64);

    impl ProvideEconomy for Bank {
        fn currency_name(&mut self, _plural: bool) -> ServiceResult<String> {
            Ok("coin".into())
        }
        fn format(&mut self, amount: i64) -> ServiceResult<String> {
            Ok(amount.to_string())
        }
        fn balance(&mut self, _account: Uuid) -> ServiceResult<i64> {
            Ok(self.0)
        }
        fn deposit(&mut self, _account: Uuid, amount: i64) -> ServiceResult<i64> {
            self.0 += amount;
            Ok(self.0)
        }
        fn withdraw(&mut self, _account: Uuid, amount: i64) -> ServiceResult<i64> {
            if amount > self.0 {
                return Err(ServiceError::Refused("too poor".into()));
            }
            self.0 -= amount;
            Ok(self.0)
        }
    }

    struct Perms;

    impl ProvidePermissions for Perms {
        const PRIORITY: Priority = Priority::High;
        fn check(&mut self, _p: Uuid, node: &str) -> ServiceResult<Tristate> {
            Ok(if node == "a" {
                Tristate::Allow
            } else {
                Tristate::Undefined
            })
        }
        fn groups(&mut self, _p: Uuid) -> ServiceResult<Vec<String>> {
            Ok(vec!["admin".into(), "default".into()])
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

    impl ProvideChatMeta for Perms {}

    #[test]
    fn provider_trait_defaults() {
        let mut b = Bank(10);
        assert!(b.has(Uuid::NIL, 10).unwrap());
        assert!(!b.has(Uuid::NIL, 11).unwrap());
        assert_eq!(b.withdraw(Uuid::NIL, 4).unwrap(), 6);
        assert!(b.withdraw(Uuid::NIL, 40).is_err());
        let mut p = Perms;
        assert_eq!(
            p.primary_group(Uuid::NIL).unwrap().as_deref(),
            Some("admin")
        );
        assert_eq!(p.check(Uuid::NIL, "a").unwrap(), Tristate::Allow);
        assert_eq!(p.prefix(Uuid::NIL).unwrap(), None);
        assert_eq!(<Perms as ProvidePermissions>::PRIORITY, Priority::High);
        assert_eq!(<Bank as ProvideEconomy>::PRIORITY, Priority::Normal);
    }
}
