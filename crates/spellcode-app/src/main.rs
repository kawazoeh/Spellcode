//! Spellcode: a black and white tabbed terminal, built with GPUI.
//!
//! Sessions run in real PTYs; a pane is only redrawn when it is notified, so an
//! idle tab costs nothing.

mod config;
mod icon;
mod keys;
mod overlay;
mod terminal;
mod theme;
mod workspace;

use gpui::{
    App, AppContext, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowOptions, point, px, size,
};

fn main() -> anyhow::Result<()> {
    config::Config::ensure_starter_file();
    let config = config::Config::load();

    let config = std::sync::Arc::new(config);

    Application::new().run(move |cx: &mut App| {
        icon::install(cx);
        let bounds = Bounds::centered(None, size(px(1180.), px(760.)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // The tab bar is the title bar: hide the system one and pull
                // the window buttons into it.
                titlebar: Some(TitlebarOptions {
                    title: Some("Spellcode".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.), px(14.))),
                }),
                // The window itself is translucent; the tab bar only tints it,
                // so the macOS blur shows through the header.
                window_background: WindowBackgroundAppearance::Blurred,
                focus: true,
                ..Default::default()
            },
            |window, cx| {
                let workspace = cx.new(|cx| workspace::Workspace::new((*config).clone(), cx));
                workspace.update(cx, |workspace, cx| workspace.attach(window, cx));
                workspace
            },
        )
        .expect("failed to open the Spellcode window");

        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);
    });

    Ok(())
}
