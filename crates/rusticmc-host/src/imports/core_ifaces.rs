//! `logging`, `events`, `commands`, `scheduler`, `permissions`, `services`, `storage`.

use crate::bindings::rustic::api::{
    commands as wc, events as wev, logging as wl, permissions as wp, scheduler as wsch,
    services as wsvc, storage as wst, types as wt,
};
use crate::convert::Cv;
use crate::state::HostState;
use rusticmc_core::model as m;

impl wl::Host for HostState {
    fn log(&mut self, level: wt::LogLevel, message: String) {
        let level: m::LogLevel = level.cv();
        if self.is_main {
            self.async_api.log(level, &message);
        } else {
            log::log!(target: "rustic", level.into(), "[{}/async] {message}", self.plugin);
        }
    }
}

impl wev::Host for HostState {
    fn subscribe(
        &mut self,
        event: String,
        priority: wt::Priority,
        ignore_cancelled: bool,
    ) -> Result<wev::HandlerId, String> {
        let api = self
            .main_api()
            .map_err(|e| format!("events.subscribe: {e}"))?;
        api.subscribe(&event, priority.cv(), ignore_cancelled)
    }

    fn unsubscribe(&mut self, handler: wev::HandlerId) {
        match self.main_api() {
            Ok(api) => api.unsubscribe(handler),
            Err(_) => self.main_only_ignored("events.unsubscribe"),
        }
    }

    fn call_custom(&mut self, name: String, payload: Vec<u8>) -> bool {
        match self.main_api() {
            Ok(api) => api.call_custom(&name, payload),
            Err(_) => {
                self.main_only_ignored("events.call-custom");
                false
            }
        }
    }
}

impl wc::Host for HostState {
    fn register(&mut self, spec: wc::CommandSpec) -> Result<wc::CommandId, String> {
        let api = self
            .main_api()
            .map_err(|e| format!("commands.register: {e}"))?;
        api.register_command(spec.cv())
    }

    fn unregister(&mut self, command: wc::CommandId) {
        match self.main_api() {
            Ok(api) => api.unregister_command(command),
            Err(_) => self.main_only_ignored("commands.unregister"),
        }
    }

    fn reply(&mut self, sender: wt::CommandSender, message: wt::Text) {
        match self.main_api() {
            Ok(api) => api.reply(sender.cv(), &message.cv()),
            Err(_) => self.main_only_ignored("commands.reply"),
        }
    }
}

impl wsch::Host for HostState {
    fn run_later(&mut self, delay: u64) -> wsch::TaskId {
        self.async_api.run_later(delay)
    }

    fn run_timer(&mut self, delay: u64, period: u64) -> wsch::TaskId {
        self.async_api.run_timer(delay, period)
    }

    fn run_async(&mut self, delay_ms: u64) -> wsch::TaskId {
        self.async_api.run_async(delay_ms)
    }

    fn run_async_timer(&mut self, delay_ms: u64, period_ms: u64) -> wsch::TaskId {
        self.async_api.run_async_timer(delay_ms, period_ms)
    }

    fn run_async_job(&mut self, job: String, payload: Vec<u8>, delay_ms: u64) -> wsch::TaskId {
        self.async_api.run_async_job(&job, payload, delay_ms)
    }

    fn cancel(&mut self, task: wsch::TaskId) {
        self.async_api.cancel(task);
    }

    fn post_to_main(&mut self, topic: String, payload: Vec<u8>) {
        self.async_api.post_to_main(&topic, payload);
    }

    fn current_tick(&mut self) -> u64 {
        self.async_api.current_tick()
    }

    fn is_main(&mut self) -> bool {
        self.is_main
    }
}

impl wp::Host for HostState {
    fn declare(
        &mut self,
        node: String,
        default: wt::PermissionDefault,
        description: String,
        children: Vec<(String, bool)>,
    ) {
        match self.main_api() {
            // Errors are logged by the core (the WIT function returns nothing).
            Ok(api) => {
                let _ = api.declare_permission(&node, default.cv(), &description, &children);
            }
            Err(_) => self.main_only_ignored("permissions.declare"),
        }
    }

    fn has(&mut self, subject: wt::CommandSender, node: String) -> bool {
        match self.main_api() {
            Ok(api) => api.has_permission(subject.cv(), &node),
            Err(_) => {
                self.warn_once(
                    "permissions.has",
                    "main instance only; returning false in the async instance",
                );
                false
            }
        }
    }
}

impl wsvc::Host for HostState {
    fn provide(&mut self, iface: String, priority: wt::Priority) -> Result<(), String> {
        let api = self
            .main_api()
            .map_err(|e| format!("services.provide: {e}"))?;
        api.provide_service(&iface, priority.cv())
    }

    fn provider(&mut self, iface: String) -> Option<String> {
        self.async_api.service_provider(&iface)
    }
}

impl wst::Host for HostState {
    fn get(&mut self, key: String) -> Option<Vec<u8>> {
        self.async_api.storage_get(&key)
    }

    fn set(&mut self, key: String, value: Vec<u8>) {
        self.async_api.storage_set(&key, value);
    }

    fn delete(&mut self, key: String) -> bool {
        self.async_api.storage_delete(&key)
    }

    fn keys(&mut self, prefix: String) -> Vec<String> {
        self.async_api.storage_keys(&prefix)
    }
}
