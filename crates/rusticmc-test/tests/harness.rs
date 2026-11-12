//! The harness's own tests, driven by native (closure) plugins: every simulated action fires the
//! right event and applies its outcome like the real server.

use rusticmc_core::commands::build::{command, dyn_arg, usage};
use rusticmc_core::context::backend_call;
use rusticmc_core::model::*;
use rusticmc_core::{CommandOutcome, PluginState};
use rusticmc_test::{AsyncMode, CommandRun, MockServer, NativePlugin};
use std::cell::RefCell;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

/// A plugin that subscribes to `events` (one handler id per event, in order) and handles them
/// with `f`.
fn listener(
    events: &'static [&'static str],
    f: impl Fn(&Event) -> Event + Clone + 'static,
) -> impl FnMut() -> NativePlugin + 'static {
    move || {
        let f = f.clone();
        NativePlugin::new()
            .on_enable(move |api| {
                for e in events {
                    api.subscribe(e, Priority::Normal, false)?;
                }
                Ok(())
            })
            .on_event(move |_api, _h, e| Ok(f(e)))
    }
}

#[test]
fn join_and_quit_broadcast_lines_and_handlers_change_them() {
    let server = MockServer::with_native_plugin(
        "greeter",
        listener(&["player-join", "player-quit"], |e| {
            let mut e = e.clone();
            match &mut e {
                Event::PlayerJoin(j) => {
                    let _ = backend_call(|b| {
                        b.players_send_message(
                            j.player,
                            &Text::plain(format!("Welcome {}", j.name)),
                        )
                    });
                    j.join_message = Some(Text::plain(format!("+ {}", j.name)));
                }
                Event::PlayerQuit(q) => q.quit_message = None,
                _ => {}
            }
            e
        }),
    );
    let steve = server.add_player("Steve");
    steve.assert_message_contains("Welcome Steve");
    steve.assert_message_contains("+ Steve");
    let alex = server.add_player("Alex");
    steve.assert_message_contains("+ Alex");
    assert_eq!(server.online_players().len(), 2);
    alex.disconnect();
    assert!(!alex.is_online());
    assert_eq!(server.online_players(), vec![steve.clone()]);
    // The quit line was hidden.
    assert!(!server.broadcasts().iter().any(|b| b.contains("left")));
    assert_eq!(steve.uuid(), rusticmc_test::offline_uuid("Steve"));
}

#[test]
fn pre_login_can_refuse_and_the_server_can_be_full() {
    let server = MockServer::with_native_plugin(
        "bouncer",
        listener(&["player-pre-login"], |e| {
            let mut e = e.clone();
            if let Event::PlayerPreLogin(p) = &mut e
                && p.name == "Griefer"
            {
                p.cancelled = true;
                p.kick_message = Some(Text::plain("Banned"));
            }
            e
        }),
    );
    assert_eq!(server.try_add_player("Griefer").unwrap_err(), "Banned");
    server.set_max_players(1);
    server.add_player("Steve");
    assert!(server.try_add_player("Alex").unwrap_err().contains("full"));
    server.add_op("Admin"); // operators bypass the limit
}

#[test]
fn cancelled_chat_is_not_broadcast_and_display_replaces_the_line() {
    let server = MockServer::with_native_plugin(
        "filter",
        listener(&["player-chat"], |e| {
            let mut e = e.clone();
            if let Event::PlayerChat(c) = &mut e {
                if c.message.contains("heck") {
                    c.cancelled = true;
                } else {
                    c.display = Some(Text::plain(format!("[VIP] {}: {}", c.name, c.message)));
                }
            }
            e
        }),
    );
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    assert!(!steve.chat("what the heck"));
    alex.assert_no_message_contains("heck");
    assert!(steve.chat("hello"));
    alex.assert_message_contains("[VIP] Steve: hello");
    server.assert_console_contains("[VIP] Steve: hello");
}

fn greet_plugin(calls: Log) -> impl FnMut() -> NativePlugin + 'static {
    move || {
        let calls = calls.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.register_command(CommandSpec {
                    permission: Some("greeter.greet".into()),
                    ..command(
                        "greet",
                        vec![usage(vec![]), usage(vec![dyn_arg("target", ArgKind::Word)])],
                    )
                })?;
                api.declare_permission("greeter.greet", PermissionDefault::Allow, "", &[])?;
                api.register_command(CommandSpec {
                    permission: Some("greeter.secret".into()),
                    ..command("secret", vec![usage(vec![])])
                })?;
                let n = ArgSpec {
                    min: Some(1.0),
                    max: Some(10.0),
                    ..ArgSpec::new("n", ArgKind::Integer)
                };
                api.register_command(command("count", vec![usage(vec![NodeSpec::Argument(n)])]))
                    .map(|_| ())
            })
            .on_command(move |api, _id, ctx| {
                calls
                    .borrow_mut()
                    .push(format!("{} {:?}", ctx.label, ctx.args));
                api.reply(ctx.sender, &Text::plain(format!("ran {}", ctx.label)));
                Ok(Ok(()))
            })
            .on_complete(|_api, _id, ctx| {
                let names = backend_call(|b| b.players_online())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|u| backend_call(|b| b.players_name(u)).ok().flatten())
                    .collect::<Vec<_>>();
                let _ = ctx;
                Ok(names)
            })
    }
}

#[test]
fn commands_run_complete_and_check_permissions() {
    let calls: Log = Rc::default();
    let server = MockServer::with_native_plugin("greeter", greet_plugin(calls.clone()));
    let steve = server.add_player("Steve");
    server.add_player("Alex");
    assert_eq!(
        steve.command("/greet Alex"),
        CommandRun::Plugin(CommandOutcome::Executed)
    );
    steve.assert_message_contains("ran greet");
    assert!(steve.command("greet").executed());
    // Not allowed: default op permission.
    assert_eq!(
        steve.command("/secret"),
        CommandRun::Plugin(CommandOutcome::NoPermission)
    );
    steve.set_op(true);
    assert!(steve.command("/secret").executed());
    // Explicit deny wins over the declared default.
    steve.set_permission("greeter.greet", false);
    assert!(!steve.has_permission("greeter.greet"));
    assert_eq!(
        steve.command("/greet"),
        CommandRun::Plugin(CommandOutcome::NoPermission)
    );
    steve.unset_permission("greeter.greet");
    // Argument validation by the runtime.
    assert!(matches!(
        steve.command("/count 50"),
        CommandRun::Plugin(CommandOutcome::Usage(_))
    ));
    // Completion: dynamic suggestions filtered by what is typed.
    assert_eq!(server.complete_as(&steve, "greet A"), vec!["Alex"]);
    assert!(
        server
            .complete_as(&steve, "gre")
            .contains(&"greet".to_owned())
    );
    // Unknown commands.
    assert_eq!(steve.command("/nope"), CommandRun::Unknown);
    steve.assert_message_contains("Unknown or incomplete command");
    // The console runs everything.
    assert!(server.console_command("secret").executed());
    server.assert_console_contains("ran secret");
    assert_eq!(calls.borrow().len(), 4);
    assert!(server.has_command("greet") && server.has_command("greeter:greet"));
}

#[test]
fn cancelled_player_command_does_not_run() {
    let calls: Log = Rc::default();
    let server = MockServer::builder()
        .native_plugin("greeter", greet_plugin(calls.clone()))
        .native_plugin(
            "blocker",
            listener(&["player-command"], |e| {
                let mut e = e.clone();
                e.set_cancelled(true);
                e
            }),
        )
        .build();
    let steve = server.add_player("Steve");
    assert_eq!(steve.command("/greet"), CommandRun::Cancelled);
    assert!(calls.borrow().is_empty());
}

#[test]
fn vanilla_commands_need_op() {
    let server = MockServer::new();
    let steve = server.add_player("Steve");
    assert_eq!(
        steve.command("/give @s diamond 3"),
        CommandRun::Vanilla(false)
    );
    steve.set_op(true);
    assert!(steve.command("/give Steve diamond 3").executed());
    assert_eq!(steve.count("diamond"), 3);
    assert!(steve.command("/gamemode creative").executed());
    assert_eq!(steve.game_mode(), GameMode::Creative);
    assert!(steve.allow_flight());
    assert!(server.console_command("time set noon").executed());
    assert_eq!(server.overworld().time(), 6000);
    assert!(server.console_command("tp Steve 10 70 10").executed());
    assert_eq!(steve.location().x, 10.0);
    assert!(server.console_command("kick Steve bye").executed());
    assert!(!steve.is_online());
    assert_eq!(steve.kick_reason().as_deref(), Some("bye"));
}

#[test]
fn block_break_cancel_keeps_the_block_and_drops_follow_the_event() {
    let server = MockServer::with_native_plugin(
        "protect",
        listener(&["block-break"], |e| {
            let mut e = e.clone();
            if let Event::BlockBreak(b) = &mut e {
                if b.block.id == "minecraft:diamond_ore" {
                    b.cancelled = true;
                } else if b.block.id == "minecraft:stone" {
                    b.drop_items = false;
                }
            }
            e
        }),
    );
    let steve = server.add_player("Steve");
    let world = server.overworld();
    world.set_block((1, 64, 1), "diamond_ore");
    assert!(!steve.break_block((1, 64, 1)));
    assert_eq!(world.block_id((1, 64, 1)), "minecraft:diamond_ore");
    assert!(steve.break_block((2, 64, 2))); // grass: drops
    assert_eq!(world.block_id((2, 64, 2)), "minecraft:air");
    assert_eq!(world.dropped_items().len(), 1);
    assert!(steve.break_block((2, 63, 2))); // stone: no drop
    assert_eq!(world.dropped_items().len(), 1);
    assert!(!steve.break_block((5, 100, 5)), "air cannot be broken");
    steve.set_game_mode(GameMode::Adventure);
    assert!(!steve.break_block((3, 64, 3)));
}

#[test]
fn block_place_needs_the_item_in_survival() {
    let server = MockServer::new();
    let steve = server.add_player("Steve");
    assert!(!steve.place_block((0, 65, 3), "stone"));
    steve.give("stone", 2);
    assert!(steve.place_block((0, 65, 3), "stone"));
    assert_eq!(steve.count("stone"), 1);
    assert!(!steve.place_block((0, 65, 3), "stone"), "occupied");
    steve.set_game_mode(GameMode::Creative);
    assert!(steve.place_block((0, 66, 3), "oak_log[axis=x]"));
    assert_eq!(
        server.overworld().block((0, 66, 3)).properties,
        vec![("axis".to_owned(), "x".to_owned())]
    );
}

#[test]
fn cancelled_move_puts_the_player_back() {
    let server = MockServer::with_native_plugin(
        "fence",
        listener(&["player-move"], |e| {
            let mut e = e.clone();
            if let Event::PlayerMove(m) = &mut e
                && m.to.x > 10.0
            {
                m.cancelled = true;
            }
            e
        }),
    );
    let steve = server.add_player("Steve");
    assert!(steve.move_by(5.0, 0.0, 0.0));
    assert!(!steve.move_by(10.0, 0.0, 0.0));
    assert_eq!(steve.location().x, 5.5);
    // Teleports are not move events.
    assert!(steve.teleport(server.nether().location(100.0, 70.0, 0.0)));
    assert_eq!(steve.location().world, "minecraft:the_nether");
}

#[test]
fn damage_death_and_respawn() {
    let seen: Log = Rc::default();
    let s = seen.clone();
    let server = MockServer::with_native_plugin("deaths", move || {
        let s = s.clone();
        NativePlugin::new()
            .on_enable(|api| {
                for e in ["entity-damage", "player-death", "player-respawn"] {
                    api.subscribe(e, Priority::Normal, false)?;
                }
                Ok(())
            })
            .on_event(move |_api, _h, e| {
                s.borrow_mut().push(e.name().to_owned());
                let mut e = e.clone();
                match &mut e {
                    Event::EntityDamage(d) if d.cause == "minecraft:fall" => d.amount /= 2.0,
                    Event::PlayerDeath(d) => {
                        d.keep_inventory = true;
                        d.death_message = Some(Text::plain("RIP"));
                    }
                    Event::PlayerRespawn(r) => {
                        r.location = Location {
                            world: "minecraft:the_end".into(),
                            x: 0.0,
                            y: 60.0,
                            z: 0.0,
                            yaw: 0.0,
                            pitch: 0.0,
                        }
                    }
                    _ => {}
                }
                Ok(e)
            })
    });
    let steve = server.add_player("Steve");
    steve.give("diamond", 5);
    assert_eq!(steve.damage_by(10.0, "minecraft:fall", None), Some(5.0));
    assert_eq!(steve.health(), 15.0);
    steve.damage(30.0);
    assert!(steve.is_dead());
    assert_eq!(steve.count("diamond"), 5, "kept by the handler");
    steve.assert_message_contains("RIP");
    let at = steve.respawn().unwrap();
    assert_eq!(at.world, "minecraft:the_end");
    assert_eq!(steve.health(), 20.0);
    assert!(!steve.is_dead());
    assert_eq!(
        seen.borrow().as_slice(),
        [
            "entity-damage",
            "entity-damage",
            "player-death",
            "player-respawn"
        ]
    );
    // Creative players take no damage.
    steve.set_game_mode(GameMode::Creative);
    assert_eq!(steve.damage(5.0), None);
}

#[test]
fn death_without_keep_inventory_drops_everything() {
    let server = MockServer::new();
    let steve = server.add_player("Steve");
    steve.give("diamond", 70);
    steve.set_level(30);
    steve.kill();
    assert_eq!(steve.count("diamond"), 0);
    assert_eq!(steve.level(), 0);
    let drops = server.overworld().dropped_items();
    assert_eq!(drops.iter().map(|i| i.count).sum::<u32>(), 70);
    assert!(server.broadcasts().contains(&"Steve died".to_owned()));
}

/// A plugin opening a 1-row menu on `/menu`: slot 0 is takeable, slot 1 closes the menu.
fn menu_plugin(clicks: Log) -> impl FnMut() -> NativePlugin + 'static {
    move || {
        let clicks = clicks.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("inventory-click", Priority::Normal, false)?;
                api.subscribe("inventory-close", Priority::Normal, false)?;
                api.register_command(command("menu", vec![usage(vec![])]))
                    .map(|_| ())
            })
            .on_command(|api, _id, ctx| {
                let player = ctx.sender.player().unwrap();
                api.menus_open(
                    player,
                    &MenuSpec {
                        rows: 1,
                        title: Text::plain("Shop"),
                        items: vec![
                            (0, ItemStack::new("minecraft:diamond", 2)),
                            (1, ItemStack::new("minecraft:barrier", 1)),
                        ],
                    },
                )
                .map_err(|e| rusticmc_core::lifecycle::PluginFault::other(e.to_string()))?;
                Ok(Ok(()))
            })
            .on_event(move |api, _h, e| {
                let mut e = e.clone();
                match &mut e {
                    Event::InventoryClick(c) => {
                        clicks.borrow_mut().push(format!(
                            "click {} {:?} {}",
                            c.slot,
                            c.menu.is_some(),
                            c.cancelled
                        ));
                        if c.slot == 0 {
                            c.cancelled = false;
                        }
                        if c.slot == 1
                            && let Some(m) = c.menu
                        {
                            api.menus_close(m);
                        }
                    }
                    Event::InventoryClose(c) => {
                        clicks
                            .borrow_mut()
                            .push(format!("close {}", c.menu.is_some()));
                    }
                    _ => {}
                }
                Ok(e)
            })
    }
}

#[test]
fn menus_open_click_and_close() {
    let clicks: Log = Rc::default();
    let server = MockServer::with_native_plugin("shop", menu_plugin(clicks.clone()));
    let steve = server.add_player("Steve");
    assert!(steve.command("/menu").executed());
    let menu = steve.open_menu().expect("menu open");
    assert_eq!(menu.title_plain(), "Shop");
    assert_eq!(menu.item(0).count, 2);
    // Display slots stay cancelled: nothing moves.
    assert!(!steve.click_menu_slot(5, ClickKind::Left));
    // Slot 0 is allowed: shift-click moves the stack into the inventory.
    assert!(steve.click_menu_slot(0, ClickKind::ShiftLeft));
    assert_eq!(steve.count("diamond"), 2);
    assert!(steve.open_menu().unwrap().item(0).is_empty());
    // Slot 1 closes the menu from the handler: the close event follows the click.
    assert!(!steve.click_menu_slot(1, ClickKind::Left));
    assert!(steve.open_menu().is_none());
    assert_eq!(
        clicks.borrow().as_slice(),
        [
            "click 5 true true",
            "click 0 true true",
            "click 1 true true",
            // The plugin closed its own menu: the runtime already released it, so the close
            // event no longer names it.
            "close false"
        ]
    );
    // Reopen and close with Escape.
    steve.command("/menu");
    assert!(steve.close_menu());
    assert!(!steve.close_menu());
}

#[test]
fn menu_left_click_picks_up_onto_the_cursor() {
    let clicks: Log = Rc::default();
    let server = MockServer::with_native_plugin("shop", menu_plugin(clicks));
    let steve = server.add_player("Steve");
    steve.command("/menu");
    assert!(steve.click_menu_slot(0, ClickKind::Left));
    assert_eq!(steve.cursor().count, 2);
    assert!(steve.open_menu().unwrap().item(0).is_empty());
}

#[test]
fn scheduler_runs_on_ticks_and_async_runs_inline() {
    let log: Log = Rc::default();
    let l = log.clone();
    let server = MockServer::with_native_plugin("tasks", move || {
        let (l1, l2, l3) = (l.clone(), l.clone(), l.clone());
        NativePlugin::new()
            .on_enable(|api| {
                api.run_later(5);
                api.run_timer(1, 10);
                api.run_async_job("fetch", b"payload".to_vec(), 0);
                Ok(())
            })
            .on_task(move |api, t| {
                l1.borrow_mut()
                    .push(format!("task {t} @{}", api.current_tick()));
                Ok(())
            })
            .on_job(move |_t, job, payload| {
                l2.borrow_mut()
                    .push(format!("job {job} {}", String::from_utf8_lossy(payload)));
                Ok(())
            })
            .on_message(move |_api, topic, _p| {
                l3.borrow_mut().push(format!("msg {topic}"));
                Ok(())
            })
    });
    server.tick(1);
    assert!(log.borrow().contains(&"job fetch payload".to_owned()));
    server.tick(20);
    let l = log.borrow();
    assert!(l.iter().any(|x| x.ends_with("@5")), "{l:?}");
    assert_eq!(
        l.iter().filter(|x| x.starts_with("task")).count(),
        4,
        "{l:?}"
    );
    assert_eq!(server.current_tick(), 21);
    assert_eq!(server.overworld().time(), 21);
}

#[test]
fn manual_async_mode_waits_for_run_async() {
    let ran = Rc::new(RefCell::new(0));
    let r = ran.clone();
    let server = MockServer::builder()
        .async_mode(AsyncMode::Manual)
        .native_plugin("bg", move || {
            let r = r.clone();
            NativePlugin::new()
                .on_enable(|api| {
                    api.run_async_job("work", vec![], 1000);
                    Ok(())
                })
                .on_job(move |_, _, _| {
                    *r.borrow_mut() += 1;
                    Ok(())
                })
        })
        .build();
    server.tick(10);
    assert_eq!(*ran.borrow(), 0);
    assert_eq!(server.run_async(), 0, "not due yet: 500 ms of fake time");
    server.advance_time(std::time::Duration::from_millis(500));
    assert_eq!(server.run_async(), 1);
    assert_eq!(*ran.borrow(), 1);
}

#[test]
fn backend_calls_are_recorded_and_queryable() {
    let server = MockServer::with_native_plugin("game", || {
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("player-join", Priority::Normal, false)?;
                Ok(())
            })
            .on_event(|api, _h, e| {
                if let Event::PlayerJoin(j) = e {
                    let p = j.player;
                    let w = "minecraft:overworld";
                    backend_call(|b| {
                        b.players_send_action_bar(p, &Text::plain("bar"))?;
                        b.players_send_title(p, &Text::plain("T"), &Text::plain("S"), 1, 2, 3)?;
                        b.players_play_sound(p, "entity.experience_orb.pickup", 1.0, 1.0)?;
                        b.inventories_give(p, &ItemStack::new("apple", 3))?;
                        b.players_set_food_level(p, 10)?;
                        b.scoreboards_create_objective("kills", &Text::plain("Kills"), "dummy")?;
                        b.scoreboards_set_score("kills", &j.name, 7)?;
                        b.scoreboards_set_display(DisplaySlot::Sidebar, Some("kills"))?;
                        b.scoreboards_create_team("red")?;
                        b.scoreboards_team_add("red", &j.name)?;
                        b.tablist_set_header_footer(p, &Text::plain("H"), &Text::plain("F"))?;
                        b.tablist_set_display_name(p, Some(&Text::plain("[A] Steve")))?;
                        b.worlds_set_block(
                            w,
                            BlockPos { x: 0, y: 80, z: 0 },
                            &BlockState {
                                id: "gold_block".into(),
                                properties: vec![],
                            },
                            true,
                        )?;
                        b.worlds_spawn_particle(
                            w,
                            "flame",
                            "",
                            Vec3::default(),
                            5,
                            Vec3::default(),
                            0.1,
                        )?;
                        b.worlds_spawn_entity(
                            w,
                            "zombie",
                            Vec3 {
                                x: 1.0,
                                y: 65.0,
                                z: 1.0,
                            },
                            "",
                        )?;
                        b.messaging_send(p, "my:chan", b"hi")?;
                        b.server_broadcast(&Text::plain("announcement"))
                    })
                    .map_err(|e| rusticmc_core::lifecycle::PluginFault::other(e.to_string()))?;
                    let bar = api
                        .bossbars_create(
                            &Text::plain("Boss"),
                            BarColor::Red,
                            BarStyle::Progress,
                            0.5,
                        )
                        .unwrap();
                    api.bossbars_show(bar, p).unwrap();
                    api.messaging_register_incoming("my:chan").unwrap();
                }
                Ok(e.clone())
            })
    });
    let steve = server.add_player("Steve");
    assert_eq!(steve.last_action_bar().as_deref(), Some("bar"));
    assert_eq!(steve.last_title().unwrap().stay, 2);
    steve.assert_heard("entity.experience_orb.pickup");
    assert_eq!(steve.count("apple"), 3);
    assert_eq!(steve.food(), 10);
    assert_eq!(server.score("kills", "Steve"), Some(7));
    assert_eq!(
        server.displayed_objective("sidebar").as_deref(),
        Some("kills")
    );
    assert!(server.team("red").unwrap().members.contains("Steve"));
    assert_eq!(
        steve.tab_header_footer(),
        Some(("H".to_owned(), "F".to_owned()))
    );
    assert_eq!(steve.tab_display_name().as_deref(), Some("[A] Steve"));
    let world = server.overworld();
    assert_eq!(world.block_id((0, 80, 0)), "minecraft:gold_block");
    assert_eq!(world.highest_block_y(0, 0), 80);
    assert_eq!(world.physics_updates(), 1);
    assert_eq!(world.particles()[0].particle, "minecraft:flame");
    assert_eq!(world.entities_of("zombie").len(), 1);
    assert_eq!(
        steve.plugin_messages(),
        vec![("my:chan".to_owned(), b"hi".to_vec())]
    );
    steve.assert_message_contains("announcement");
    assert_eq!(steve.boss_bars()[0].progress, 0.5);
    assert_eq!(server.incoming_channels(), vec!["my:chan"]);
    // Disabling the plugin releases its boss bar and channel.
    server.disable("game").unwrap();
    assert!(server.boss_bars().is_empty());
    assert!(server.incoming_channels().is_empty());
    assert_eq!(server.disabled_plugins(), vec!["game"]);
}

#[test]
fn plugin_teleport_and_game_mode_fire_events_for_other_plugins() {
    let seen: Log = Rc::default();
    let s = seen.clone();
    let server = MockServer::builder()
        .native_plugin("mover", || {
            NativePlugin::new()
                .on_enable(|api| {
                    api.register_command(command("go", vec![usage(vec![])]))
                        .map(|_| ())
                })
                .on_command(|_api, _id, ctx| {
                    let p = ctx.sender.player().unwrap();
                    let _ = backend_call(|b| {
                        b.players_teleport(
                            p,
                            &Location {
                                world: "minecraft:the_nether".into(),
                                x: 1.0,
                                y: 70.0,
                                z: 1.0,
                                yaw: 0.0,
                                pitch: 0.0,
                            },
                        )?;
                        b.players_set_game_mode(p, GameMode::Creative)
                    });
                    Ok(Ok(()))
                })
        })
        .native_plugin("watcher", move || {
            let s = s.clone();
            NativePlugin::new()
                .on_enable(|api| {
                    for e in [
                        "player-teleport",
                        "player-change-world",
                        "player-game-mode-change",
                    ] {
                        api.subscribe(e, Priority::Normal, false)?;
                    }
                    Ok(())
                })
                .on_event(move |_api, _h, e| {
                    s.borrow_mut().push(e.name().to_owned());
                    let mut e = e.clone();
                    e.set_cancelled(true); // refuses the game-mode change
                    Ok(e)
                })
        })
        .build();
    let steve = server.add_player("Steve");
    assert!(steve.command("/go").executed());
    assert_eq!(steve.location().world, "minecraft:the_nether");
    assert_eq!(
        steve.game_mode(),
        GameMode::Survival,
        "cancelled by watcher"
    );
    assert_eq!(
        seen.borrow().as_slice(),
        [
            "player-teleport",
            "player-change-world",
            "player-game-mode-change"
        ]
    );
}

#[test]
fn kick_and_health_zero_from_a_plugin_are_processed_after_its_call() {
    let server = MockServer::with_native_plugin("admin", || {
        NativePlugin::new()
            .on_enable(|api| {
                api.register_command(command("smite", vec![usage(vec![])]))
                    .map(|_| ())?;
                api.register_command(command("boot", vec![usage(vec![])]))
                    .map(|_| ())
            })
            .on_command(|_api, _id, ctx| {
                let p = ctx.sender.player().unwrap();
                let _ = backend_call(|b| match ctx.label.as_str() {
                    "smite" => b.players_set_health(p, 0.0),
                    _ => b.players_kick(p, &Text::plain("Bye!")),
                });
                Ok(Ok(()))
            })
    });
    let steve = server.add_player("Steve");
    steve.command("/smite");
    assert!(steve.is_dead());
    steve.respawn();
    steve.command("/boot");
    assert!(!steve.is_online());
    steve.assert_kicked();
    assert_eq!(steve.kick_reason().as_deref(), Some("Bye!"));
}

#[test]
fn plugin_messages_reach_only_registered_channels() {
    let got: Log = Rc::default();
    let g = got.clone();
    let server = MockServer::with_native_plugin("proxy", move || {
        let g = g.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.messaging_register_incoming("bungeecord:main")?;
                api.subscribe("plugin-message", Priority::Normal, false)?;
                Ok(())
            })
            .on_event(move |_api, _h, e| {
                if let Event::PluginMessage(m) = e {
                    g.borrow_mut()
                        .push(String::from_utf8_lossy(&m.data).into_owned());
                }
                Ok(e.clone())
            })
    });
    let steve = server.add_player("Steve");
    assert!(steve.send_plugin_message("BungeeCord:main", b"Connect"));
    assert!(!steve.send_plugin_message("other:chan", b"x"));
    assert_eq!(got.borrow().as_slice(), ["Connect"]);
}

#[test]
fn server_list_ping_and_chunks() {
    let server = MockServer::with_native_plugin(
        "motd",
        listener(&["server-list-ping", "chunk-unload"], |e| {
            let mut e = e.clone();
            if let Event::ServerListPing(p) = &mut e {
                p.motd = Text::plain("Custom MOTD");
                p.max_players = 100;
            }
            e
        }),
    );
    let ping = server.ping("1.2.3.4");
    assert_eq!(ping.motd.to_plain(), "Custom MOTD");
    assert_eq!(ping.max_players, 100);
    let w = server.overworld();
    assert!(w.is_chunk_loaded(0, 0));
    w.load_chunk(10, 10, true);
    assert!(w.unload_chunk(10, 10));
    assert!(!w.is_chunk_loaded(10, 10));
}

#[test]
fn faults_disable_the_plugin_and_the_server_goes_on() {
    let server =
        MockServer::with_native_plugin("buggy", listener(&["player-chat"], |_e| panic!("boom")));
    let steve = server.add_player("Steve");
    assert!(steve.chat("hello"), "the chat still goes through");
    assert_eq!(server.plugin_state("buggy"), Some(PluginState::Failed));
    server.assert_console_contains("boom");
    let info = server.plugin("buggy").unwrap();
    assert!(info.failure.unwrap().contains("panic"));
}

#[test]
fn reload_reinstantiates_and_storage_survives() {
    let server = MockServer::with_native_plugin("counter", || {
        NativePlugin::new().on_enable(|api| {
            let n: u32 = api
                .storage_get("enables")
                .and_then(|v| serde_json::from_slice(&v).ok())
                .unwrap_or(0);
            api.storage_set("enables", serde_json::to_vec(&(n + 1)).unwrap());
            Ok(())
        })
    });
    assert_eq!(server.storage_json::<u32>("counter", "enables"), Some(1));
    server.reload("counter").unwrap();
    assert_eq!(server.storage_json::<u32>("counter", "enables"), Some(2));
    server.assert_enabled("counter");
}

#[test]
fn plugin_config_is_written_before_enable() {
    let server = MockServer::builder()
        .plugin_config("cfg", "greeting = \"hi\"\n")
        .native_plugin("cfg", || {
            NativePlugin::new().on_enable(|api| {
                let dir = api.data_dir().ok_or("no data dir")?;
                let text =
                    std::fs::read_to_string(dir.join("config.toml")).map_err(|e| e.to_string())?;
                if text.contains("hi") {
                    Ok(())
                } else {
                    Err("config not found".into())
                }
            })
        })
        .build();
    server.assert_enabled("cfg");
}

#[test]
fn per_player_data_is_readable() {
    let server = MockServer::with_native_plugin("data", || {
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("player-join", Priority::Normal, false)?;
                Ok(())
            })
            .on_event(|api, _h, e| {
                if let Event::PlayerJoin(j) = e {
                    api.player_data_set(j.player, "visits", b"1".to_vec());
                }
                Ok(e.clone())
            })
    });
    let steve = server.add_player("Steve");
    assert_eq!(steve.data_json::<u32>("data", "visits"), Some(1));
}

#[test]
fn log_lines_are_captured_per_server() {
    let server = MockServer::with_native_plugin("chatty", || {
        NativePlugin::new().on_enable(|api| {
            api.log(LogLevel::Info, "hello from chatty");
            Ok(())
        })
    });
    server.assert_console_contains("[chatty] hello from chatty");
    let other = MockServer::new();
    assert!(!other.console().iter().any(|l| l.contains("chatty")));
}
