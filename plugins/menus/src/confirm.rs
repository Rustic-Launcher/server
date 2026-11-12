//! A reusable "Are you sure?" dialog: a 3-row menu with Yes / No buttons around a question
//! item. Closing it without answering counts as "no".
//!
//! ```ignore
//! Confirm::new(lang)
//!     .question(ItemStack::new("tnt").name("Delete your home?"))
//!     .on_yes(|p| delete_home(p))
//!     .on_no(|p| reopen_previous(p))
//!     .open(&player)?;
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use rustic::{ClickKind, ItemStack, Menu, MenuHandle, Player, Result, Text};

use crate::lang::{Lang, Msg, tr};
use crate::ui;

type Answer = Box<dyn FnOnce(Player)>;
/// Both answers, taken by whichever button (or the close) comes first.
type Answers = Rc<RefCell<Option<(Option<Answer>, Option<Answer>)>>>;

/// Slot of the Yes button.
pub const YES_SLOT: u32 = ui::slot(1, 2);
/// Slot of the question item.
pub const QUESTION_SLOT: u32 = ui::slot(1, 4);
/// Slot of the No button.
pub const NO_SLOT: u32 = ui::slot(1, 6);

/// Only deliberate clicks answer (not drops, number keys or double clicks).
fn answers_click(kind: ClickKind) -> bool {
    matches!(
        kind,
        ClickKind::Left | ClickKind::Right | ClickKind::ShiftLeft | ClickKind::ShiftRight
    )
}

/// A confirmation dialog under construction.
pub struct Confirm {
    title: Text,
    question: ItemStack,
    yes: ItemStack,
    no: ItemStack,
    on_yes: Option<Answer>,
    on_no: Option<Answer>,
}

impl Confirm {
    /// A dialog with the default title and buttons in `lang`.
    pub fn new(lang: Lang) -> Confirm {
        Confirm {
            title: tr(lang, Msg::ConfirmTitle, &[]),
            question: ItemStack::new("minecraft:paper").name(tr(lang, Msg::ConfirmTitle, &[])),
            yes: ItemStack::new("minecraft:lime_concrete").name(tr(lang, Msg::Yes, &[])),
            no: ItemStack::new("minecraft:red_concrete").name(tr(lang, Msg::No, &[])),
            on_yes: None,
            on_no: None,
        }
    }

    /// The menu title.
    pub fn title(mut self, title: impl Into<Text>) -> Confirm {
        self.title = title.into();
        self
    }

    /// The item in the middle that says what is being confirmed.
    pub fn question(mut self, item: ItemStack) -> Confirm {
        self.question = item;
        self
    }

    /// Runs after the player clicks Yes (the dialog is closed first).
    pub fn on_yes(mut self, f: impl FnOnce(Player) + 'static) -> Confirm {
        self.on_yes = Some(Box::new(f));
        self
    }

    /// Runs after the player clicks No or closes the dialog.
    pub fn on_no(mut self, f: impl FnOnce(Player) + 'static) -> Confirm {
        self.on_no = Some(Box::new(f));
        self
    }

    /// Opens it. Exactly one of the answers runs, at most once.
    pub fn open(self, player: &Player) -> Result<MenuHandle> {
        // Shared by the three closures: whoever answers first takes both callbacks.
        let answers: Answers = Rc::new(RefCell::new(Some((self.on_yes, self.on_no))));
        let (a_yes, a_no, a_close) = (answers.clone(), answers.clone(), answers);
        Menu::new(3, self.title)
            .item(YES_SLOT, self.yes, move |click| {
                if !answers_click(click.kind()) {
                    return;
                }
                let taken = a_yes.borrow_mut().take();
                click.close();
                if let Some((Some(yes), _)) = taken {
                    yes(click.player());
                }
            })
            .display(QUESTION_SLOT, self.question)
            .item(NO_SLOT, self.no, move |click| {
                if !answers_click(click.kind()) {
                    return;
                }
                let taken = a_no.borrow_mut().take();
                click.close();
                if let Some((_, Some(no))) = taken {
                    no(click.player());
                }
            })
            .fill(ui::filler())
            .on_close(move |player| {
                if let Some((_, Some(no))) = a_close.borrow_mut().take() {
                    no(player);
                }
            })
            .open(player)
    }
}
