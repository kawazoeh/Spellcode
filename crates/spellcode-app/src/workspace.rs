//! The root view: a tab bar with the Spellcode wordmark, plus the panes.

use gpui::{
    Context, Entity, FontWeight, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point, Render,
    SharedString, Subscription, Window, div, prelude::*, px,
};
use spellcode_term::{SpawnSpec, TermSize};

use crate::{
    config::{AppEntry, Config},
    icon::{self, Icon},
    overlay::{MenuAction, MenuItem, OverlayEvent, OverlayView, PickerKind},
    terminal::{TerminalPane, Viewport},
    theme::{self, Palette, Theme},
};

const TAB_BAR_HEIGHT: f32 = 40.;
/// Room for the window buttons, which the system draws over our header.
const HEADER_LEADING: f32 = 82.;
/// Opacity of the black wash over the macOS blur. The window is translucent,
/// so the tint has to cover everything the opaque terminal card does not.
/// How far below the click a menu opens, so it clears the tab bar.
const MENU_DROP: f32 = 22.;
/// Inset around the terminal so its corners can be rounded.
const CORNER_MARGIN: f32 = 6.;
/// A tighter top inset, so the terminal sits close under the tab bar.
const CORNER_MARGIN_TOP: f32 = 2.;

pub struct Workspace {
    config: Config,
    palette: Palette,
    theme: Theme,

    overlay: Entity<OverlayView>,
    panes: Vec<Pane>,
    active: Option<usize>,
    viewport: Viewport,
    last_viewport: (f32, f32),
    /// A window is not usable while it is being built, so the first frame is
    /// what actually opens the starting shell and hands it the focus.
    start_shell_on_first_frame: bool,
    subscriptions: Vec<Subscription>,
}

struct Pane {
    view: Entity<TerminalPane>,
    /// The name on the tab, set when the session is created and only ever
    /// changed by an explicit rename.
    label: SharedString,
    /// Icon shown on the left of the tab, if the user picked one.
    icon: Option<Icon>,
    /// Background colour of the tab, as `#rrggbb`. Defaults to the theme.
    color: Option<gpui::Rgba>,
}

impl Workspace {
    pub fn new(config: Config, cx: &mut Context<Self>) -> Self {
        let palette = Palette::dark();
        let theme = Theme::monochrome();
        let overlay = cx.new(|cx| OverlayView::new(theme, cx));

        Self {
            config,
            palette,
            theme,
            overlay,
            panes: Vec::new(),
            active: None,
            viewport: Viewport::new(),
            last_viewport: (0., 0.),
            start_shell_on_first_frame: true,
            subscriptions: Vec::new(),
        }
    }

    /// Wires the overlay to the workspace. Called once the window exists.
    pub fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.subscribe_in(
            &self.overlay,
            window,
            |this, _, event: &OverlayEvent, window, cx| match event {
                OverlayEvent::Action(action) => this.run(action.clone(), window, cx),
                OverlayEvent::IconPicked(index, icon) => {
                    if let Some(pane) = this.panes.get_mut(*index) {
                        pane.icon = Some(*icon);
                    }
                    this.focus_active(window, cx);
                }
                OverlayEvent::ColorPicked(index, value) => {
                    let color = theme::parse_hex(value);
                    if let Some(pane) = this.panes.get_mut(*index) {
                        pane.color = color;
                    }
                    this.focus_active(window, cx);
                }
                OverlayEvent::Renamed(index, value) => {
                    if let Some(pane) = this.panes.get_mut(*index) {
                        pane.label = SharedString::from(value.clone());
                    }
                    this.focus_active(window, cx);
                }
                OverlayEvent::Dismissed => this.focus_active(window, cx),
            },
        ));
    }

    fn spawn_spec(&self, entry: &AppEntry) -> SpawnSpec {
        let mut spec = SpawnSpec::new(entry.command.clone(), TermSize::default());
        spec.args = entry.args.clone();

        if !entry.cwd.is_empty() {
            spec.cwd = Some(std::path::PathBuf::from(expand_tilde(&entry.cwd)));
        } else {
            spec.cwd = self.config.cwd();
        }

        spec.env = vec![
            ("TERM".into(), "xterm-256color".into()),
            ("COLORTERM".into(), "truecolor".into()),
            ("TERM_PROGRAM".into(), "Spellcode".into()),
        ];
        spec
    }

    /// Builds a session for `entry` and makes it the active one.
    fn open(&mut self, entry: AppEntry, window: &mut Window, cx: &mut Context<Self>) {
        let spec = self.spawn_spec(&entry);
        let general = self.config.general.clone();
        let palette = self.palette.clone();
        let theme = self.theme;
        let viewport = self.viewport.clone();
        let label = SharedString::from(entry.name.clone());

        let view = cx.new(|cx| {
            TerminalPane::new(
                spec,
                palette,
                theme,
                &general.font_family,
                general.font_size,
                general.line_height,
                general.padding,
                general.scrollback,
                viewport,
                cx,
            )
        });

        // A pane that exits changes the dot on its tab.
        self.subscriptions
            .push(cx.subscribe(&view, |_this, _pane, _event, cx| cx.notify()));

        let index = self.panes.len();

        self.panes.push(Pane {
            view,
            label,
            icon: None,
            color: None,
        });
        self.activate(index, window, cx);
    }

    fn open_shell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entry = AppEntry {
            name: "Shell".to_string(),
            command: self.config.general.shell.clone(),
            ..Default::default()
        };
        self.open(entry, window, cx);
    }

    fn run(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            MenuAction::NewShell => self.open_shell(window, cx),
            MenuAction::NewApp(index) => {
                if let Some(entry) = self
                    .config
                    .visible_apps()
                    .get(index)
                    .map(|entry| (*entry).clone())
                {
                    self.open(entry, window, cx);
                }
            }
            MenuAction::PickIcon(index) => {
                self.overlay.update(cx, |overlay, cx| {
                    overlay.open_picker(index, PickerKind::Icons, window, cx);
                });
            }
            MenuAction::PickColor(index) => {
                self.overlay.update(cx, |overlay, cx| {
                    overlay.open_picker(index, PickerKind::Colors, window, cx);
                });
            }
            MenuAction::ResetTab(index) => {
                if let Some(pane) = self.panes.get_mut(index) {
                    pane.icon = None;
                    pane.color = None;
                }
                self.focus_active(window, cx);
            }
            MenuAction::Rename(index) => {
                if let Some(label) = self.panes.get(index).map(|pane| pane.label.to_string()) {
                    self.overlay.update(cx, |overlay, cx| {
                        overlay.open_rename(index, label, window, cx);
                    });
                }
            }
            MenuAction::Close(index) => self.close(index, window, cx),
        }
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.panes.get(index).map(|pane| pane.view.clone()) else {
            return;
        };
        self.active = Some(index);
        view.update(cx, |pane, _| pane.invalidate());
        view.read(cx).focus(window);
        cx.notify();
    }

    fn focus_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self
            .active
            .and_then(|index| self.panes.get(index))
            .map(|pane| pane.view.clone())
        {
            view.read(cx).focus(window);
        }
    }

    fn close(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.panes.len() {
            return;
        }
        self.panes.remove(index);
        self.active = None;

        if self.panes.is_empty() {
            self.open_shell(window, cx);
        } else {
            self.activate(index.min(self.panes.len() - 1), window, cx);
        }
        cx.notify();
    }

    fn close_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.active {
            self.close(index, window, cx);
        }
    }

    fn cycle(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.panes.is_empty() {
            return;
        }
        let current = self.active.unwrap_or(0) as isize;
        let count = self.panes.len() as isize;
        self.activate((current + delta).rem_euclid(count) as usize, window, cx);
    }

    fn adjust_font_size(&mut self, delta: f32, cx: &mut Context<Self>) {
        let general = &mut self.config.general;
        let previous = general.font_size;
        general.font_size = (previous + delta).clamp(8., 32.);
        let (size, height) = (general.font_size, general.line_height);

        for pane in &self.panes {
            pane.view
                .update(cx, |pane, cx| pane.set_font_size(size, height, cx));
        }
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !event.keystroke.modifiers.platform {
            return;
        }

        match event.keystroke.key.as_str() {
            "t" => self.open_shell(window, cx),
            "w" => self.close_active(window, cx),
            "n" => self.cycle(1, window, cx),
            "p" => self.cycle(-1, window, cx),
            "=" | "+" => self.adjust_font_size(1., cx),
            "-" => self.adjust_font_size(-1., cx),
            "0" => {
                let base = self.config.general.font_size;
                self.adjust_font_size(13. - base, cx);
            }
            _ => {}
        }
    }

    /// The "+" menu: a plain shell, then whatever is configured.
    fn new_tab_menu(&self) -> (Vec<MenuItem>, Vec<MenuAction>) {
        let mut items = vec![MenuItem::new("Shell").hint("cmd+t")];
        let mut actions = vec![MenuAction::NewShell];

        for (index, entry) in self.config.visible_apps().iter().enumerate() {
            let item = MenuItem::new(entry.name.clone());
            items.push(if entry.is_available() {
                item
            } else {
                item.hint("not installed")
            });
            actions.push(MenuAction::NewApp(index));
        }

        (items, actions)
    }

    fn tab_menu(&self, index: usize) -> (Vec<MenuItem>, Vec<MenuAction>) {
        (
            vec![
                MenuItem::new("Rename").hint("enter"),
                MenuItem::new("Icon").hint("fa"),
                MenuItem::new("Background").hint("colour"),
                MenuItem::new("Reset"),
                MenuItem::new("Close Tab").hint("cmd+w").destructive(),
            ],
            vec![
                MenuAction::Rename(index),
                MenuAction::PickIcon(index),
                MenuAction::PickColor(index),
                MenuAction::ResetTab(index),
                MenuAction::Close(index),
            ],
        )
    }

    fn render_header(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;

        let tabs: Vec<TabInfo> = self
            .panes
            .iter()
            .enumerate()
            .map(|(index, pane)| {
                let view = pane.view.read(cx);
                TabInfo {
                    index,
                    label: pane.label.clone(),
                    active: index == self.active.unwrap_or(0),
                    exited: view.is_exited(),
                    scrolled: view.is_scrolled(),
                    icon: pane.icon,
                    color: pane.color,
                }
            })
            .collect();

        let new_tab = div()
            .id("new-tab")
            .size(px(22.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .text_sm()
            .text_color(theme.text_dim)
            .cursor_pointer()
            .hover(|style| style.bg(theme.accent_soft).text_color(theme.text))
            .child("+")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    this.open_shell(window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.open_new_tab_menu(event.position, window, cx);
                }),
            );

        let mut header = div()
            .h(px(TAB_BAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .pl(px(HEADER_LEADING))
            .pr_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child("Spellcode"),
            )
            .child(div().w(px(3.)))
            .child(div().w(px(1.)).h(px(18.)).bg(theme.border))
            .child(new_tab);

        // No air between the new-tab button and the first tab: the button
        // already reads as its own thing.
        for tab in tabs {
            header = header.child(tab.element(theme, cx));
        }

        header.into_any_element()
    }

    fn open_new_tab_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (items, actions) = self.new_tab_menu();
        self.overlay.update(cx, |overlay, cx| {
            overlay.open_menu(drop_below(position), items, actions, window, cx);
        });
    }

    fn open_tab_menu(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (items, actions) = self.tab_menu(index);
        self.overlay.update(cx, |overlay, cx| {
            overlay.open_menu(drop_below(position), items, actions, window, cx);
        });
    }
}

struct TabInfo {
    index: usize,
    label: SharedString,
    active: bool,
    exited: bool,
    scrolled: bool,
    icon: Option<Icon>,
    color: Option<gpui::Rgba>,
}

impl TabInfo {
    fn element(self, theme: Theme, cx: &mut Context<Workspace>) -> gpui::AnyElement {
        let index = self.index;
        let background = self
            .color
            .map(gpui::Hsla::from)
            .unwrap_or(theme.accent_soft);

        div()
            .id(SharedString::from(format!("tab:{index}")))
            .h(px(26.))
            // A hair tighter on the left than the right, so the dot does not
            // look pushed away from the edge of the tab background.
            .pr(px(12.))
            .pl(px(4.))
            .flex()
            .items_center()
            .gap_2()
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .bg(background)
            .when(self.active, |this| this.text_color(theme.text))
            .when(!self.active, |this| {
                this.text_color(theme.text_faint)
                    .hover(|style| style.text_color(theme.text_dim))
            })
            .child(div().size_1_5().rounded_full().bg(if self.exited {
                theme.text_faint
            } else if self.scrolled {
                theme.text_dim
            } else {
                theme.text
            }))
            .when_some(self.icon, |this, icon| {
                this.child(
                    div()
                        .font(icon::font())
                        .text_color(theme.text_dim)
                        .child(icon::glyph(icon)),
                )
            })
            .child(self.label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    this.activate(index, window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.open_tab_menu(index, event.position, window, cx);
                }),
            )
            .into_any_element()
    }
}

/// Pushes a menu just below the tab bar.
fn drop_below(position: Point<Pixels>) -> Point<Pixels> {
    Point {
        x: position.x,
        y: position.y + px(MENU_DROP),
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.start_shell_on_first_frame) {
            self.open_shell(window, cx);
        }

        let theme = self.theme;
        let viewport = window.viewport_size();
        let terminal_height =
            (viewport.height - px(TAB_BAR_HEIGHT + CORNER_MARGIN + CORNER_MARGIN_TOP)).max(px(1.));
        self.viewport.set(viewport.width, terminal_height);

        // A pane only re-renders when it is notified, so a resize has to be
        // pushed to it explicitly.
        let size = (f32::from(viewport.width), f32::from(terminal_height));
        if size != self.last_viewport {
            self.last_viewport = size;
            if let Some(view) = self
                .active
                .and_then(|index| self.panes.get(index))
                .map(|pane| pane.view.clone())
            {
                view.update(cx, |pane, cx| {
                    pane.invalidate();
                    cx.notify();
                });
            }
        }

        let active_pane = self
            .active
            .and_then(|index| self.panes.get(index))
            .map(|pane| pane.view.clone())
            .map(|view| view.into_any_element())
            .unwrap_or_else(|| div().size_full().into_any_element());

        let header = self.render_header(window, cx);

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(theme::fade(
                theme.background,
                self.config.general.window_tint,
            ))
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.on_key(event, window, cx)
                }),
            )
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .mx(px(CORNER_MARGIN))
                    .mb(px(CORNER_MARGIN))
                    .mt(px(CORNER_MARGIN_TOP))
                    .relative()
                    .rounded_lg()
                    .bg(theme::fade(
                        theme.background,
                        self.config.general.terminal_opacity,
                    ))
                    .border_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    .child(active_pane),
            )
            .child(self.overlay.clone())
    }
}

fn expand_tilde(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => match std::env::var_os("HOME") {
            Some(home) => std::path::Path::new(&home)
                .join(rest)
                .to_string_lossy()
                .into_owned(),
            None => path.to_string(),
        },
        None => path.to_string(),
    }
}
