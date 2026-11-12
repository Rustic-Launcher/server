//! {{name}}: {{description}}
//!
//! `/menu` opens a chest menu: a teleport to spawn, a daylight switch (operators only), a free
//! item to take, and a close button. Clicks in a plugin menu are cancelled unless the handler
//! calls `click.allow()`, so nothing can be stolen from it.

use rustic::prelude::*;

#[rustic::plugin]
#[derive(Default)]
struct {{struct_name}};

impl Plugin for {{struct_name}} {
    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        // `/menu` is registered by #[rustic::handlers] below.
        Ok(())
    }
}

#[rustic::handlers]
impl {{struct_name}} {
    #[command(name = "menu", description = "Open the {{name}} menu")]
    fn menu(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let player = cmd.player()?;
        open_menu(&player)?;
        Ok(())
    }
}

fn open_menu(player: &Player) -> Result<()> {
    Menu::new(3, Text::mini("<dark_gray>{{name}}"))
        .item(
            11,
            ItemStack::new("minecraft:compass").name(Text::mini("<green>Spawn")),
            |click| {
                click.close();
                let player = click.player();
                if let Ok(spawn) = World::overworld().spawn_location() {
                    let _ = player.teleport(&spawn);
                    player.send_message(Text::mini("<green>Teleported to spawn."));
                }
            },
        )
        .item(
            13,
            ItemStack::new("minecraft:clock").name(Text::mini("<yellow>Make it day")),
            |click| {
                let player = click.player();
                if !player.is_op() {
                    player.send_message(Text::mini("<red>Operators only."));
                    return;
                }
                let _ = World::overworld().set_time(1000);
                player.send_message(Text::mini("<yellow>Good morning!"));
            },
        )
        .item(
            15,
            ItemStack::new("minecraft:bread").count(3).name("Free bread"),
            // Allowed: the click goes through, the player takes the bread.
            |click| click.allow(),
        )
        .item(
            26,
            ItemStack::new("minecraft:barrier").name(Text::mini("<red>Close")),
            |click| click.close(),
        )
        .fill(ItemStack::new("minecraft:gray_stained_glass_pane").name(""))
        .open(player)?;
    Ok(())
}
