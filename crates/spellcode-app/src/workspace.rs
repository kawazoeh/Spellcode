//! The root view: a tab bar with the Spellcode wordmark, plus the panes.

use gpui::{
    Context, Entity, FocusHandle, FontWeight, KeyDownEvent, MouseButton, MouseDownEvent, Pixels,
    Point, Render, SharedString, Subscription, Window, div, prelude::*, px,
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
/// Width of one caption button, the size Windows itself uses.
const CAPTION_BUTTON_WIDTH: f32 = 46.;
/// Whether the app has to draw the window buttons itself. macOS draws its
/// traffic lights over the transparent title bar, Windows has no equivalent.
const SELF_DRAWN_CAPTION: bool = cfg!(target_os = "windows");
/// Left inset of the tab bar. macOS needs room for the traffic lights; on
/// Windows that space would be dead.
const HEADER_LEADING_WINDOWS: f32 = 12.;
/// How many caption buttons the app draws: minimise, zoom, close.
const CAPTION_BUTTONS: usize = 3;
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
    /// One focus handle per caption button, so the window controls can be
    /// reached with the keyboard on Windows.
    caption_focus: Vec<FocusHandle>,
    /// Which caption button the pointer is on, if any. The glyph has to be
    /// repainted with it, a hover style cannot reach into a canvas.
    caption_hover: Option<usize>,
}

/// What a caption button does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Caption {
    Minimize,
    Zoom { maximized: bool },
    Close,
}

/// Size of the drawn glyph inside a caption button, in points.
const CAPTION_GLYPH: f32 = 10.;

/// Draws a caption glyph as strokes, so the buttons stay monochrome and need
/// no font: a bar, a square or a cross.
fn glyph_shape(caption: Caption, color: gpui::Hsla) -> gpui::AnyElement {
    let size = CAPTION_GLYPH;
    let half = size / 2.;
    let stroke = px(1.);

    // A square outline, drawn as four sides.
    let square = |left: f32, top: f32, right: f32, bottom: f32| {
        vec![
            (left, top),
            (right, top),
            (right, bottom),
            (left, bottom),
            (left, top),
        ]
    };
    // The strokes of a path, as offset pairs relative to the canvas origin.
    let sides = |points: Vec<(f32, f32)>| {
        points
            .windows(2)
            .map(|pair| (pair[0], pair[1]))
            .collect::<Vec<_>>()
    };

    let strokes: Vec<((f32, f32), (f32, f32))> = match caption {
        // A bar sitting on the baseline, as Windows draws minimise.
        Caption::Minimize => sides(vec![(half - 4., half + 3.), (half + 4., half + 3.)]),
        Caption::Zoom { maximized: false } => {
            sides(square(half - 4., half - 4., half + 4., half + 4.))
        }
        // Restore is two squares: the front one, and the back one peeking out
        // from behind its top left corner.
        Caption::Zoom { maximized: true } => {
            let mut front = sides(square(half - 4., half - 1., half + 4., half + 4.));
            // The back square only shows its top and left edges, the rest is
            // hidden behind the front one.
            front.extend(sides(vec![(half - 1., half - 4.), (half - 1., half - 1.)]));
            front.extend(sides(vec![(half - 1., half - 4.), (half + 4., half - 4.)]));
            front.extend(sides(vec![(half + 4., half - 4.), (half + 4., half - 1.)]));
            front
        }
        Caption::Close => sides(vec![(half - 3.5, half - 3.5), (half + 3.5, half + 3.5)])
            .into_iter()
            .chain(sides(vec![
                (half + 3.5, half - 3.5),
                (half - 3.5, half + 3.5),
            ]))
            .collect(),
    };

    gpui::canvas(
        |_bounds, _window, _cx| (),
        move |bounds, (), window, _cx| {
            let origin = bounds.origin;
            for (from, to) in strokes {
                let mut path = gpui::PathBuilder::stroke(stroke);
                path.move_to(gpui::point(origin.x + px(from.0), origin.y + px(from.1)));
                path.line_to(gpui::point(origin.x + px(to.0), origin.y + px(to.1)));
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .w(px(size))
    .h(px(size))
    .into_any_element()
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
        let caption_focus = (0..CAPTION_BUTTONS).map(|_| cx.focus_handle()).collect();

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
            caption_focus,
            caption_hover: None,
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
            spec.cwd = Some(std::path::PathBuf::from(expand_tilde(
                &entry.cwd,
                crate::config::home_dir().as_deref(),
            )));
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

    fn render_header(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let maximized = window.is_maximized();

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

        // The tab list takes the slack, so a long list scrolls instead of
        // pushing the window buttons off the right edge.
        let mut strip = div()
            .id("tab-strip")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .gap_1()
            .overflow_x_scroll();

        // No air between the new-tab button and the first tab: the button
        // already reads as its own thing.
        for tab in tabs {
            strip = strip.child(tab.element(theme, cx));
        }

        let leading = if SELF_DRAWN_CAPTION {
            HEADER_LEADING_WINDOWS
        } else {
            HEADER_LEADING
        };

        let mut header = div()
            .h(px(TAB_BAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .pl(px(leading))
            .pr(px(if SELF_DRAWN_CAPTION { 0. } else { 16. }))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child("Spellcode"),
            )
            .child(div().w(px(3.)))
            .child(div().w(px(1.)).h(px(18.)).bg(theme.border))
            .child(new_tab)
            .child(strip);

        if SELF_DRAWN_CAPTION {
            header = header.child(self.window_buttons(maximized, cx));
        }

        header.into_any_element()
    }

    /// The caption buttons Windows never draws for us: the same three squares
    /// as the system ones, in the app's monochrome.
    ///
    /// macOS needs none of this, its traffic lights are already there.
    fn window_buttons(&mut self, maximized: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let captions = [
            Caption::Minimize,
            Caption::Zoom { maximized },
            Caption::Close,
        ];

        let mut row = div().flex_shrink_0().h_full().flex().items_center();

        for (index, caption) in captions.into_iter().enumerate() {
            row = row.child(self.caption_button(index, caption, cx));
        }

        row.into_any_element()
    }

    fn caption_button(
        &mut self,
        index: usize,
        caption: Caption,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.theme;
        let focus = self.caption_focus[index].clone();
        let id = SharedString::from(format!("window-button:{index}"));
        let hovered = self.caption_hover == Some(index);

        // Close is the one button that has to stand out, so it inverts on
        // hover instead of taking a grey, the way the system one does.
        let destructive = matches!(caption, Caption::Close);
        let (idle_bg, hover_bg) = if destructive {
            (gpui::transparent_black(), theme.text)
        } else {
            (gpui::transparent_black(), theme.accent_soft)
        };
        let (idle_fg, hover_fg) = if destructive {
            (theme.text_dim, theme.background)
        } else {
            (theme.text_dim, theme.text)
        };
        let glyph_fg = if hovered { hover_fg } else { idle_fg };

        div()
            .id(id)
            .w(px(CAPTION_BUTTON_WIDTH))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(idle_bg)
            .hover(move |style| style.bg(hover_bg))
            .active(move |style| style.bg(hover_bg))
            // Keyboard reachable: GPUI turns Enter and Space into a click on a
            // focused element, and tab navigation walks the three buttons.
            .track_focus(&focus)
            .tab_stop(true)
            .tab_index(index as isize)
            .focus(move |style| style.border_1().border_color(theme.text_dim))
            .on_hover(cx.listener(move |this, is_hovered, _, cx| {
                let hovered = if *is_hovered { Some(index) } else { None };
                if this.caption_hover != hovered {
                    this.caption_hover = hovered;
                    cx.notify();
                }
            }))
            .child(glyph_shape(caption, glyph_fg))
            .on_click(cx.listener(move |this, _, window, cx| match caption {
                Caption::Minimize => window.minimize_window(),
                Caption::Zoom { .. } => window.zoom_window(),
                Caption::Close => {
                    // Hand the focus back before the window goes away, so the
                    // terminal does not keep a dead focus.
                    this.focus_active(window, cx);
                    window.remove_window();
                }
            }))
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

/// Expands a leading `~` using the platform home directory. Both `~/` and
/// `~\` are accepted so a Windows-style path works there too. With no home
/// available the path is returned unchanged.
fn expand_tilde(path: &str, home: Option<&std::path::Path>) -> String {
    let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"));
    match (rest, home) {
        (Some(rest), Some(home)) => home.join(rest).to_string_lossy().into_owned(),
        _ => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::expand_tilde;
    use std::path::Path;

    #[test]
    fn tilde_expands_with_the_home_directory() {
        assert_eq!(
            expand_tilde("~/project", Some(Path::new("/Users/kaito"))),
            Path::new("/Users/kaito").join("project").to_string_lossy()
        );
    }

    #[test]
    fn tilde_accepts_a_windows_separator() {
        let expanded = expand_tilde("~\\project", Some(Path::new("C:\\Users\\Zenax")));
        assert!(expanded.ends_with("project"), "got {expanded}");
        assert!(expanded.contains("Zenax"), "got {expanded}");
    }

    #[test]
    fn tilde_is_left_alone_without_a_home() {
        assert_eq!(expand_tilde("~/project", None), "~/project");
    }

    #[test]
    fn a_path_without_a_tilde_is_untouched() {
        assert_eq!(
            expand_tilde("/absolute/path", Some(Path::new("/Users/kaito"))),
            "/absolute/path"
        );
    }
}
