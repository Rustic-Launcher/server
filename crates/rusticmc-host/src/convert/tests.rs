//! Round trips core → WIT → core for every converted type, plus field-mapping spot checks
//! from hand-built WIT values (a swapped field would survive a round trip, not these).

use super::*;
use rusticmc_core::model::Event;

fn rt<W, C>(c: C)
where
    C: Cv<W> + Clone + PartialEq + std::fmt::Debug,
    W: Cv<C>,
{
    let back: C = c.clone().cv().cv();
    assert_eq!(back, c);
}

fn uuid(n: u64) -> m::Uuid {
    m::Uuid::new(n, n.wrapping_mul(31))
}

fn loc(w: &str) -> m::Location {
    m::Location {
        world: w.into(),
        x: 1.5,
        y: -2.0,
        z: 3.25,
        yaw: 90.0,
        pitch: -10.0,
    }
}

fn item(id: &str) -> m::ItemStack {
    m::ItemStack {
        id: id.into(),
        count: 3,
        components: "{x:1}".into(),
    }
}

fn block() -> m::BlockState {
    m::BlockState {
        id: "minecraft:oak_log".into(),
        properties: vec![("axis".into(), "y".into())],
    }
}

fn entity() -> m::EntityRef {
    m::EntityRef {
        id: 7,
        uuid: uuid(9),
        kind: "minecraft:zombie".into(),
    }
}

#[test]
fn types_round_trip() {
    rt::<wt::Uuid, _>(uuid(1));
    rt::<wt::Vec3, _>(m::Vec3 {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    });
    rt::<wt::BlockPos, _>(m::BlockPos { x: -1, y: 2, z: 3 });
    rt::<wt::Location, _>(loc("minecraft:the_nether"));
    for g in [
        m::GameMode::Survival,
        m::GameMode::Creative,
        m::GameMode::Adventure,
        m::GameMode::Spectator,
    ] {
        rt::<wt::GameMode, _>(g);
    }
    for p in m::Priority::ALL {
        rt::<wt::Priority, _>(p);
    }
    rt::<wt::ItemStack, _>(item("minecraft:diamond"));
    rt::<wt::BlockState, _>(block());
    for l in [
        m::LogLevel::Trace,
        m::LogLevel::Debug,
        m::LogLevel::Info,
        m::LogLevel::Warn,
        m::LogLevel::Error,
    ] {
        rt::<wt::LogLevel, _>(l);
    }
    rt::<wt::CommandSender, _>(m::CommandSender::Console);
    rt::<wt::CommandSender, _>(m::CommandSender::Player(uuid(4)));
    rt::<wt::EntityRef, _>(entity());
    rt::<wt::Hand, _>(m::Hand::MainHand);
    rt::<wt::Hand, _>(m::Hand::OffHand);
    for f in [
        m::BlockFace::Down,
        m::BlockFace::Up,
        m::BlockFace::North,
        m::BlockFace::South,
        m::BlockFace::West,
        m::BlockFace::East,
    ] {
        rt::<wt::BlockFace, _>(f);
    }
    for w in [m::Weather::Clear, m::Weather::Rain, m::Weather::Thunder] {
        rt::<wt::Weather, _>(w);
    }
    for d in [
        m::PermissionDefault::Allow,
        m::PermissionDefault::Deny,
        m::PermissionDefault::Op,
        m::PermissionDefault::NotOp,
    ] {
        rt::<wt::PermissionDefault, _>(d);
    }
    for e in [
        m::HostError::NoSuchPlayer,
        m::HostError::NoSuchWorld,
        m::HostError::CapabilityDenied("network".into()),
        m::HostError::MainThreadOnly,
        m::HostError::AsyncOnly,
        m::HostError::Invalid("x".into()),
        m::HostError::Unsupported("y".into()),
        m::HostError::Other("z".into()),
    ] {
        rt::<wt::HostError, _>(e);
    }
    let t = m::Text::colored("hi", "gold");
    let w: String = t.clone().cv();
    let back: m::Text = w.cv();
    assert_eq!(back, t);
}

#[test]
fn commands_round_trip() {
    let kinds = [
        m::ArgKind::Word,
        m::ArgKind::String,
        m::ArgKind::Greedy,
        m::ArgKind::Integer,
        m::ArgKind::Number,
        m::ArgKind::Boolean,
        m::ArgKind::Player,
        m::ArgKind::Players,
        m::ArgKind::Position,
        m::ArgKind::BlockPos,
        m::ArgKind::Item,
        m::ArgKind::World,
        m::ArgKind::Duration,
    ];
    for k in kinds {
        rt::<wc::ArgKind, _>(k);
    }
    let spec = m::CommandSpec {
        name: "give".into(),
        aliases: vec!["g".into()],
        description: "d".into(),
        permission: Some("p.give".into()),
        players_only: true,
        usages: vec![m::Usage {
            nodes: vec![
                m::NodeSpec::Literal("to".into()),
                m::NodeSpec::Argument(m::ArgSpec {
                    name: "n".into(),
                    kind: m::ArgKind::Integer,
                    min: Some(1.0),
                    max: None,
                    suggestions: vec!["1".into()],
                    dynamic_suggestions: true,
                }),
            ],
            permission: None,
            description: "u".into(),
        }],
    };
    rt::<wc::CommandSpec, _>(spec);
    let values = vec![
        ("a".to_owned(), m::ArgValue::Word("w".into())),
        ("b".into(), m::ArgValue::String("s".into())),
        ("c".into(), m::ArgValue::Greedy("g g".into())),
        ("d".into(), m::ArgValue::Integer(-5)),
        ("e".into(), m::ArgValue::Number(0.5)),
        ("f".into(), m::ArgValue::Boolean(true)),
        ("g".into(), m::ArgValue::Player(uuid(2))),
        ("h".into(), m::ArgValue::Players(vec![uuid(2), uuid(3)])),
        ("i".into(), m::ArgValue::Position(loc("w"))),
        (
            "j".into(),
            m::ArgValue::BlockPos(m::BlockPos { x: 1, y: 2, z: 3 }),
        ),
        ("k".into(), m::ArgValue::Item(item("minecraft:stone"))),
        ("l".into(), m::ArgValue::World("minecraft:overworld".into())),
        ("m".into(), m::ArgValue::Duration(40)),
    ];
    rt::<wc::CommandContext, _>(m::CommandContext {
        sender: m::CommandSender::Player(uuid(1)),
        label: "give".into(),
        usage: 2,
        args: values,
        raw: "give to 5".into(),
    });
    rt::<wc::CompleteContext, _>(m::CompleteContext {
        sender: m::CommandSender::Console,
        raw: "give to ".into(),
        argument: "n".into(),
        partial: "".into(),
    });
}

#[test]
fn game_and_services_round_trip() {
    rt::<wsv::PluginMeta, _>(m::PluginMeta {
        id: "a".into(),
        name: "A".into(),
        version: "1.0.0".into(),
        enabled: true,
    });
    rt::<wsv::TickStats, _>(m::TickStats {
        mspt: 12.5,
        tps: 20.0,
    });
    rt::<wm::MenuSpec, _>(m::MenuSpec {
        rows: 3,
        title: m::Text::plain("menu"),
        items: vec![(4, item("minecraft:apple"))],
    });
    for s in [
        m::DisplaySlot::Sidebar,
        m::DisplaySlot::PlayerList,
        m::DisplaySlot::BelowName,
    ] {
        rt::<ws::DisplaySlot, _>(s);
    }
    for c in [
        m::TeamColor::Black,
        m::TeamColor::DarkBlue,
        m::TeamColor::DarkGreen,
        m::TeamColor::DarkAqua,
        m::TeamColor::DarkRed,
        m::TeamColor::DarkPurple,
        m::TeamColor::Gold,
        m::TeamColor::Gray,
        m::TeamColor::DarkGray,
        m::TeamColor::Blue,
        m::TeamColor::Green,
        m::TeamColor::Aqua,
        m::TeamColor::Red,
        m::TeamColor::LightPurple,
        m::TeamColor::Yellow,
        m::TeamColor::White,
        m::TeamColor::Reset,
    ] {
        rt::<ws::TeamColor, _>(c);
    }
    for c in [
        m::BarColor::Pink,
        m::BarColor::Blue,
        m::BarColor::Red,
        m::BarColor::Green,
        m::BarColor::Yellow,
        m::BarColor::Purple,
        m::BarColor::White,
    ] {
        rt::<wb::BarColor, _>(c);
    }
    for s in [
        m::BarStyle::Progress,
        m::BarStyle::Notched6,
        m::BarStyle::Notched10,
        m::BarStyle::Notched12,
        m::BarStyle::Notched20,
    ] {
        rt::<wb::BarStyle, _>(s);
    }
    rt::<wh::Request, _>(m::HttpRequest {
        method: "POST".into(),
        url: "https://a.example/x".into(),
        headers: vec![("a".into(), "b".into())],
        body: vec![1, 2, 3],
        timeout_ms: 500,
    });
    rt::<wh::Response, _>(m::HttpResponse {
        status: 404,
        headers: vec![],
        body: b"nope".to_vec(),
    });
    rt::<wco::Uuid, _>(uuid(77));
    for e in [
        m::ServiceError::NoProvider,
        m::ServiceError::ProviderFailed,
        m::ServiceError::Refused("poor".into()),
    ] {
        rt::<wco::ServiceError, _>(e);
    }
    for t in [
        m::Tristate::Allow,
        m::Tristate::Deny,
        m::Tristate::Undefined,
    ] {
        rt::<pp_import::Tristate, _>(t);
        rt::<pp_export::Tristate, _>(t);
        let i: pp_import::Tristate = t.cv();
        let back: m::Tristate = tristate_from_export(tristate_to_export(i)).cv();
        assert_eq!(back, t);
    }
}

fn every_event() -> Vec<Event> {
    use rusticmc_core::model::*;
    vec![
        Event::PlayerPreLogin(PlayerPreLogin {
            player: uuid(1),
            name: "n".into(),
            address: "1.2.3.4".into(),
            cancelled: true,
            kick_message: Some(Text::plain("no")),
        }),
        Event::PlayerJoin(PlayerJoin {
            player: uuid(1),
            name: "n".into(),
            join_message: None,
        }),
        Event::PlayerQuit(PlayerQuit {
            player: uuid(1),
            name: "n".into(),
            quit_message: Some(Text::plain("bye")),
        }),
        Event::PlayerChat(PlayerChat {
            player: uuid(1),
            name: "n".into(),
            message: "m".into(),
            cancelled: false,
            display: Some(Text::plain("d")),
        }),
        Event::PlayerCommand(PlayerCommand {
            player: uuid(1),
            name: "n".into(),
            command: "c".into(),
            cancelled: true,
        }),
        Event::ServerCommand(ServerCommand {
            command: "c".into(),
            cancelled: false,
        }),
        Event::PlayerMove(PlayerMove {
            player: uuid(1),
            from: loc("a"),
            to: loc("b"),
            cancelled: true,
        }),
        Event::PlayerTeleport(PlayerTeleport {
            player: uuid(1),
            from: loc("a"),
            to: loc("b"),
        }),
        Event::PlayerChangeWorld(PlayerChangeWorld {
            player: uuid(1),
            from_world: "a".into(),
            to_world: "b".into(),
        }),
        Event::BlockBreak(BlockBreak {
            player: uuid(1),
            world: "w".into(),
            pos: BlockPos { x: 1, y: 2, z: 3 },
            block: block(),
            cancelled: false,
            drop_items: true,
        }),
        Event::BlockPlace(BlockPlace {
            player: uuid(1),
            world: "w".into(),
            pos: BlockPos { x: 1, y: 2, z: 3 },
            block: block(),
            hand: Hand::OffHand,
            cancelled: true,
        }),
        Event::PlayerInteract(PlayerInteract {
            player: uuid(1),
            action: InteractAction::RightClickBlock,
            hand: Hand::MainHand,
            item: item("minecraft:stick"),
            world: "w".into(),
            block: Some(BlockPos { x: 0, y: 0, z: 0 }),
            face: Some(BlockFace::Up),
            cancelled: false,
        }),
        Event::PlayerInteractEntity(PlayerInteractEntity {
            player: uuid(1),
            target: entity(),
            hand: Hand::MainHand,
            cancelled: false,
        }),
        Event::EntityDamage(EntityDamage {
            target: entity(),
            cause: "minecraft:fall".into(),
            attacker: Some(entity()),
            amount: 4.5,
            cancelled: false,
        }),
        Event::PlayerDeath(PlayerDeath {
            player: uuid(1),
            cause: "c".into(),
            death_message: None,
            keep_inventory: true,
            keep_level: false,
        }),
        Event::EntityDeath(EntityDeath {
            entity: entity(),
            killer: Some(uuid(2)),
            drops: vec![item("minecraft:bone")],
            dropped_exp: 5,
        }),
        Event::PlayerRespawn(PlayerRespawn {
            player: uuid(1),
            location: loc("w"),
        }),
        Event::InventoryClick(InventoryClick {
            player: uuid(1),
            menu: Some(3),
            slot: -1,
            click: ClickKind::ShiftRight,
            hotbar_button: 2,
            item: item("a"),
            cursor: item("b"),
            cancelled: true,
        }),
        Event::InventoryClose(InventoryClose {
            player: uuid(1),
            menu: None,
        }),
        Event::PlayerGameModeChange(PlayerGameModeChange {
            player: uuid(1),
            old_mode: GameMode::Survival,
            new_mode: GameMode::Creative,
            cancelled: false,
        }),
        Event::ServerListPing(ServerListPing {
            address: "a".into(),
            motd: Text::plain("motd"),
            max_players: 20,
            online_players: 3,
            show_players: true,
        }),
        Event::PluginMessage(PluginMessage {
            player: uuid(1),
            channel: "a:b".into(),
            data: vec![9, 8],
        }),
        Event::ChunkLoad(ChunkLoad {
            world: "w".into(),
            x: 1,
            z: -1,
            new_chunk: true,
        }),
        Event::ChunkUnload(ChunkUnload {
            world: "w".into(),
            x: 1,
            z: -1,
        }),
        Event::CustomEvent(CustomEvent {
            name: "a:b".into(),
            source_plugin: "a".into(),
            payload: vec![1],
            cancelled: true,
        }),
    ]
}

#[test]
fn every_event_round_trips() {
    let events = every_event();
    assert_eq!(events.len(), rusticmc_core::model::EventKind::COUNT);
    for e in events {
        macro_rules! check {
            ($($v:ident),*) => {
                match &e {
                    $(Event::$v(x) => rt::<we::$v, _>(x.clone()),)*
                }
            };
        }
        check!(
            PlayerPreLogin,
            PlayerJoin,
            PlayerQuit,
            PlayerChat,
            PlayerCommand,
            ServerCommand,
            PlayerMove,
            PlayerTeleport,
            PlayerChangeWorld,
            BlockBreak,
            BlockPlace,
            PlayerInteract,
            PlayerInteractEntity,
            EntityDamage,
            PlayerDeath,
            EntityDeath,
            PlayerRespawn,
            InventoryClick,
            InventoryClose,
            PlayerGameModeChange,
            ServerListPing,
            PluginMessage,
            ChunkLoad,
            ChunkUnload,
            CustomEvent
        );
    }
    for a in [
        m::InteractAction::LeftClickBlock,
        m::InteractAction::RightClickBlock,
        m::InteractAction::LeftClickAir,
        m::InteractAction::RightClickAir,
    ] {
        rt::<we::InteractAction, _>(a);
    }
    for c in [
        m::ClickKind::Left,
        m::ClickKind::Right,
        m::ClickKind::ShiftLeft,
        m::ClickKind::ShiftRight,
        m::ClickKind::NumberKey,
        m::ClickKind::Drop,
        m::ClickKind::ControlDrop,
        m::ClickKind::DoubleClick,
        m::ClickKind::Middle,
        m::ClickKind::Other,
    ] {
        rt::<we::ClickKind, _>(c);
    }
}

#[test]
fn field_mapping_from_wit() {
    let w = wt::Location {
        world: "w".into(),
        x: 1.0,
        y: 2.0,
        z: 3.0,
        yaw: 4.0,
        pitch: 5.0,
    };
    let c: m::Location = w.clone().cv();
    assert_eq!((c.x, c.y, c.z, c.yaw, c.pitch), (1.0, 2.0, 3.0, 4.0, 5.0));
    let u: m::Uuid = wt::Uuid { hi: 1, lo: 2 }.cv();
    assert_eq!((u.hi, u.lo), (1, 2));
    let mv = we::PlayerMove {
        player: wt::Uuid { hi: 1, lo: 2 },
        from: w.clone(),
        to: wt::Location {
            world: "x".into(),
            ..w
        },
        cancelled: true,
    };
    let c: m::PlayerMove = mv.cv();
    assert_eq!(c.from.world, "w");
    assert_eq!(c.to.world, "x");
    assert!(c.cancelled);
    let bb: m::BlockBreak = we::BlockBreak {
        player: wt::Uuid { hi: 0, lo: 0 },
        world: "w".into(),
        pos: wt::BlockPos { x: 1, y: 2, z: 3 },
        block: wt::BlockState {
            id: "b".into(),
            properties: vec![],
        },
        cancelled: false,
        drop_items: true,
    }
    .cv();
    assert!(!bb.cancelled && bb.drop_items);
    assert_eq!((bb.pos.x, bb.pos.y, bb.pos.z), (1, 2, 3));
}
