//! Test guest exercising the host from the inside: event priorities, cancellation and
//! MONITOR, commands with completion, sync tasks, async jobs and post-to-main, storage, the
//! WASI data folder, the HTTP capability, a service consumer, permissions.

use rustic::prelude::*;

const ACCOUNT: Uuid = Uuid { hi: 0, lo: 42 };

#[rustic::plugin]
#[derive(Default)]
struct Probe;

impl Plugin for Probe {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        rustic::permissions::Permissions::declare("probe.secret")
            .default(PermissionDefault::Deny)
            .description("probe secret")
            .register();
        Ok(())
    }

    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        // Priorities: LOW cancels bedrock, HIGH (ignore_cancelled) clears drops, MONITOR
        // records what it saw and tries to change it (ignored by the host).
        ctx.on_with(Priority::Low, false, |e: &mut BlockBreakEvent| {
            if e.block().id == "minecraft:bedrock" {
                e.cancel();
            }
        })?;
        ctx.on_with(Priority::High, true, |e: &mut BlockBreakEvent| {
            e.set_drop_items(false);
        })?;
        ctx.on_with(Priority::Monitor, false, |e: &mut BlockBreakEvent| {
            let _ = Storage::set("monitor_saw_cancelled", &e.is_cancelled());
            let flipped = !e.is_cancelled();
            e.set_cancelled(flipped);
            e.set_drop_items(true);
        })?;
        ctx.on(|e: &mut PlayerJoinEvent| {
            let name = e.name().to_owned();
            e.set_join_message(Text::new(format!("welcome {name}")));
            e.player().send_message(Text::new(format!("hi {name}")));
        })?;
        ctx.on_custom("probe:ping", |e: &mut CustomEvent| {
            if e.payload() == b"cancel" {
                e.cancel();
            }
        })?;

        ctx.async_task("double", |n: i64| -> Result<()> {
            let world_access = Player::from_uuid(ACCOUNT).teleport(&Location {
                world: "minecraft:overworld".into(),
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
            });
            let report = format!(
                "is_main={} world={}",
                Scheduler::is_main(),
                match world_access {
                    Ok(()) => "ok".to_owned(),
                    Err(e) => e.to_string(),
                }
            );
            Storage::set("async_report", &report)?;
            Scheduler::post_to_main("doubled", &(n * 2))?;
            Ok(())
        });
        ctx.on_message("doubled", |v: i64| Storage::set("doubled", &v));
        ctx.async_task("fetch", |url: String| -> Result<()> {
            let outcome = match rustic::http::Request::get(url).send() {
                Ok(r) => format!("status {}", r.status),
                Err(e) => format!("error {e}"),
            };
            Scheduler::post_to_main("fetched", &outcome)?;
            Ok(())
        });
        ctx.on_message("fetched", |v: String| Storage::set("fetched", &v));

        ctx.command(
            Command::new("probe-echo")
                .usage("<msg:greedy>")
                .executor(|cmd| {
                    let msg: String = cmd.arg("msg")?;
                    cmd.reply(Text::new(format!("echo {msg}")));
                    Ok(())
                }),
        )?;
        ctx.command(
            Command::new("probe-pick")
                .usage("<thing:word*>")
                .executor(|cmd| {
                    let t: String = cmd.arg("thing")?;
                    cmd.reply(Text::new(format!("picked {t}")));
                    Ok(())
                })
                .completer(|c| c.suggest(["apple", "apricot", "banana"])),
        )?;
        ctx.command(Command::new("probe-fail").executor(|_| Err(CommandError::new("nope"))))?;
        ctx.command(Command::new("probe-later").executor(|_| {
            Scheduler::run_later(2, || {
                let _ = Storage::set("later_ran_at", &Scheduler::current_tick());
            });
            Ok(())
        }))?;
        ctx.command(
            Command::new("probe-async")
                .usage("<n:integer>")
                .executor(|cmd| {
                    let n: i64 = cmd.arg("n")?;
                    Scheduler::run_async("double", &n)?;
                    Ok(())
                }),
        )?;
        ctx.command(
            Command::new("probe-http")
                .usage("<url:greedy>")
                .executor(|cmd| {
                    let url: String = cmd.arg("url")?;
                    Scheduler::run_async("fetch", &url)?;
                    Ok(())
                }),
        )?;
        ctx.command(
            Command::new("probe-pay")
                .usage("<amount:integer>")
                .executor(|cmd| {
                    let amount: i64 = cmd.arg("amount")?;
                    let line = match Economy::deposit(ACCOUNT, amount) {
                        Ok(b) => format!("balance {b}"),
                        Err(e) => format!("service error {e:?}"),
                    };
                    cmd.reply(Text::new(line));
                    Ok(())
                }),
        )?;
        ctx.command(Command::new("probe-config").executor(|cmd| {
            let text = std::fs::read_to_string("/config.toml")
                .unwrap_or_else(|e| format!("unreadable: {e}"));
            cmd.reply(Text::new(format!("config {}", text.trim())));
            Ok(())
        }))?;
        ctx.command(
            Command::new("probe-store")
                .usage("<key:word> <value:word>")
                .executor(|cmd| {
                    let k: String = cmd.arg("key")?;
                    let v: String = cmd.arg("value")?;
                    Storage::set(&k, &v)?;
                    Ok(())
                }),
        )?;
        ctx.command(
            Command::new("probe-get")
                .usage("<key:word>")
                .executor(|cmd| {
                    let k: String = cmd.arg("key")?;
                    let v: Option<String> = Storage::get(&k)?;
                    cmd.reply(Text::new(format!(
                        "value {}",
                        v.unwrap_or_else(|| "none".into())
                    )));
                    Ok(())
                }),
        )?;
        ctx.command(Command::new("probe-perm").executor(|cmd| {
            cmd.reply(Text::new(format!(
                "secret {}",
                cmd.sender().has_permission("probe.secret")
            )));
            Ok(())
        }))?;
        Ok(())
    }
}
