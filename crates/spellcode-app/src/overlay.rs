//! The floating menu, the rename dialog and the icon and colour pickers,
//! rendered as an overlay above the workspace.

use gpui::{
    Context, EventEmitter, FocusHandle, Hsla, KeyDownEvent, MouseButton, MouseDownEvent, Pixels,
    Point, Render, SharedString, Window, div, prelude::*, px,
};

use crate::{
    icon::{self, Icon},
    theme::{self, Theme},
};

/// Minimum width of a floating menu.
const MENU_WIDTH: f32 = 180.;

/// What a menu row asks the workspace to do.
#[derive(Clone)]
pub enum MenuAction {
    NewShell,
    /// Index into the configured app list.
    NewApp(usize),
    Rename(usize),
    PickIcon(usize),
    PickColor(usize),
    ResetTab(usize),
    Close(usize),
}

/// Which list the picker is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Icons,
    Colors,
}

struct PickerState {
    tab: usize,
    kind: PickerKind,
}

/// One selectable row.
#[derive(Clone)]
pub struct MenuItem {
    pub label: SharedString,
    pub hint: Option<SharedString>,
    /// Destructive rows are dimmed and get a hairline above them.
    pub destructive: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            hint: None,
            destructive: false,
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
}

struct MenuState {
    position: Point<Pixels>,
    items: Vec<MenuItem>,
    actions: Vec<MenuAction>,
}

struct PromptState {
    /// The tab being renamed.
    tab: usize,
    value: String,
}

enum Overlay {
    Menu(MenuState),
    Rename(PromptState),
    Picker(PickerState),
}

/// What the overlay asks the workspace to do.
pub enum OverlayEvent {
    Action(MenuAction),
    /// A rename dialog was confirmed for a tab.
    Renamed(usize, String),
    /// An icon or a background colour was chosen for a tab.
    IconPicked(usize, Icon),
    ColorPicked(usize, SharedString),
    Dismissed,
}

impl EventEmitter<OverlayEvent> for OverlayView {}

pub struct OverlayView {
    overlay: Option<Overlay>,
    focus: FocusHandle,
    theme: Theme,
}

impl OverlayView {
    pub fn new(theme: Theme, cx: &mut Context<Self>) -> Self {
        Self {
            overlay: None,
            focus: cx.focus_handle(),
            theme,
        }
    }

    pub fn open_menu(
        &mut self,
        position: Point<Pixels>,
        items: Vec<MenuItem>,
        actions: Vec<MenuAction>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if items.is_empty() {
            return;
        }

        // Keep the menu on screen when the click was near the right edge.
        let width = f32::from(window.viewport_size().width);
        let menu_width = MENU_WIDTH
            .max(8. + items.iter().map(|item| item.label.len()).max().unwrap_or(8) as f32 * 7.);
        let x = position.x.min(px((width - menu_width - 12.).max(12.)));

        self.overlay = Some(Overlay::Menu(MenuState {
            position: Point { x, y: position.y },
            items,
            actions,
        }));
        self.focus.focus(window);
        cx.notify();
    }

    pub fn open_rename(
        &mut self,
        tab: usize,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overlay = Some(Overlay::Rename(PromptState { tab, value }));
        self.focus.focus(window);
        cx.notify();
    }

    /// Opens the icon or colour list for a tab.
    pub fn open_picker(
        &mut self,
        tab: usize,
        kind: PickerKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overlay = Some(Overlay::Picker(PickerState { tab, kind }));
        self.focus.focus(window);
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.overlay.take().is_some() {
            cx.emit(OverlayEvent::Dismissed);
            cx.notify();
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if !matches!(self.overlay, Some(Overlay::Rename(_))) {
            // Any key dismisses a menu or a picker.
            self.close(cx);
            return;
        }
        let Some(Overlay::Rename(prompt)) = self.overlay.as_mut() else {
            return;
        };

        match event.keystroke.key.as_str() {
            "escape" => self.close(cx),
            "enter" => {
                if let Some(Overlay::Rename(prompt)) = self.overlay.take() {
                    let value = prompt.value.trim().to_string();
                    cx.emit(OverlayEvent::Renamed(prompt.tab, value));
                    cx.notify();
                }
            }
            "backspace" => {
                prompt.value.pop();
                cx.notify();
            }
            key => {
                if event.keystroke.modifiers.platform || event.keystroke.modifiers.control {
                    return;
                }
                let text = event.keystroke.key_char.as_deref().unwrap_or(key);
                if let Some(character) = text.chars().next().filter(|c| !c.is_control()) {
                    prompt.value.push(character);
                    cx.notify();
                }
            }
        }
    }
}

/// A copy of the overlay state, so rendering never borrows the view.
enum Snapshot {
    None,
    Menu {
        position: Point<Pixels>,
        items: Vec<(MenuItem, MenuAction)>,
    },
    Rename {
        tab: usize,
        value: String,
    },
    Picker {
        tab: usize,
        kind: PickerKind,
    },
}

impl OverlayView {
    fn snapshot(&self) -> Snapshot {
        match self.overlay.as_ref() {
            None => Snapshot::None,
            Some(Overlay::Menu(menu)) => Snapshot::Menu {
                position: menu.position,
                items: menu
                    .items
                    .iter()
                    .cloned()
                    .zip(menu.actions.iter().cloned())
                    .collect(),
            },
            Some(Overlay::Rename(prompt)) => Snapshot::Rename {
                tab: prompt.tab,
                value: prompt.value.clone(),
            },
            Some(Overlay::Picker(picker)) => Snapshot::Picker {
                tab: picker.tab,
                kind: picker.kind,
            },
        }
    }
}

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let snapshot = self.snapshot();

        // While an overlay is open the backdrop swallows clicks, so clicking
        // away closes it instead of reaching the terminal underneath.
        // The background is what gives this element a hitbox: without it the
        // clicks land on whatever is behind the dialog.
        let backdrop = div()
            .absolute()
            .inset_0()
            .bg(gpui::transparent_black())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.close(cx)),
            );

        let content = match snapshot {
            Snapshot::None => div().into_any_element(),

            Snapshot::Menu { position, items } => {
                let mut previous_destructive = false;
                let rows: Vec<gpui::AnyElement> = items
                    .into_iter()
                    .enumerate()
                    .map(|(index, (item, action))| {
                        let destructive = item.destructive;
                        let row = div()
                            .id(SharedString::from(format!("menu:{index}")))
                            .h(px(26.))
                            .px_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .text_sm()
                            .text_color(if destructive {
                                theme.text_dim
                            } else {
                                theme.text
                            })
                            .cursor_pointer()
                            .when(destructive && !previous_destructive, |this| {
                                this.border_t_1().border_color(theme.border)
                            })
                            .hover(|style| style.bg(theme.accent_soft).text_color(theme.text))
                            .child(item.label)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_faint)
                                    .child(item.hint.unwrap_or_default()),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.close(cx);
                                    cx.emit(OverlayEvent::Action(action.clone()));
                                }),
                            );
                        previous_destructive = destructive;
                        row.into_any_element()
                    })
                    .collect();

                div()
                    .absolute()
                    .left(position.x)
                    .top(position.y)
                    .min_w(px(MENU_WIDTH))
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .overflow_hidden()
                    .shadow_lg()
                    .children(rows)
                    .into_any_element()
            }

            Snapshot::Rename { tab, value } => div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(360.))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .px_4()
                        .py_3()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.surface)
                        .shadow_lg()
                        // The dialog sits on top of a backdrop that covers the
                        // whole window. Without stopping propagation, a click
                        // here would still reach the backdrop and close it.
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                            }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_faint)
                                .child(format!("Rename tab {}", tab + 1)),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(theme.text_faint)
                                .bg(theme.background)
                                .text_sm()
                                .text_color(theme.text)
                                .child(if value.is_empty() {
                                    div().text_color(theme.text_faint).child("Untitled")
                                } else {
                                    div().child(value)
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_faint)
                                .child("Enter to confirm    esc to cancel"),
                        ),
                )
                .into_any_element(),

            Snapshot::Picker { tab, kind } => div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(384.))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .px_4()
                        .py_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.surface)
                        .shadow_lg()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                            }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_faint)
                                .child(match kind {
                                    PickerKind::Icons => "Icon",
                                    PickerKind::Colors => "Background colour",
                                }),
                        )
                        .child(match kind {
                            PickerKind::Icons => render_icon_grid(tab, theme, cx),
                            PickerKind::Colors => render_swatch_grid(tab, theme, cx),
                        }),
                )
                .into_any_element(),
        };

        div()
            .absolute()
            .inset_0()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.on_key(event, cx)))
            .child(backdrop)
            .child(content)
    }
}

/// The grid of Font Awesome icons.
fn render_icon_grid(tab: usize, theme: Theme, cx: &mut Context<OverlayView>) -> gpui::AnyElement {
    let cells: Vec<gpui::AnyElement> = Icon::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, icon)| {
            div()
                .id(SharedString::from(format!("icon:{index}")))
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .text_color(theme.text)
                .cursor_pointer()
                .hover(|style| style.bg(theme.accent_soft))
                .child(
                    div()
                        .font(icon::font())
                        .text_color(theme.text)
                        .child(icon::glyph(icon)),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.close(cx);
                        cx.emit(OverlayEvent::IconPicked(tab, icon));
                    }),
                )
                .into_any_element()
        })
        .collect();

    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .children(cells)
        .into_any_element()
}

/// The grid of background colours.
fn render_swatch_grid(tab: usize, theme: Theme, cx: &mut Context<OverlayView>) -> gpui::AnyElement {
    let cells: Vec<gpui::AnyElement> = icon::SWATCHES
        .iter()
        .enumerate()
        .map(|(index, swatch)| {
            let Some(color) = theme::parse_hex(swatch.value) else {
                return div().into_any_element();
            };
            let value: SharedString = swatch.value.into();
            div()
                .id(SharedString::from(format!("swatch:{index}")))
                .size(px(30.))
                .rounded_md()
                .cursor_pointer()
                .border_1()
                .border_color(theme.border)
                .hover(|style| style.border_color(theme.text))
                .child(div().size_full().rounded_md().bg(Hsla::from(color)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.close(cx);
                        cx.emit(OverlayEvent::ColorPicked(tab, value.clone()));
                    }),
                )
                .into_any_element()
        })
        .collect();

    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .children(cells)
        .into_any_element()
}
