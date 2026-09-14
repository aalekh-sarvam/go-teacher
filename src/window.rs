//! Native window hosting the web UI in a WebView (tao + wry), with a macOS menu bar.

use muda::{accelerator::{Accelerator, Code, Modifiers}, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tokio_util::sync::CancellationToken;
use wry::{NewWindowResponse, WebViewBuilder};

#[derive(Debug)]
enum UserEvent {
    Quit,
}

fn open_external(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

/// Show the UI at `url` in a native window and block forever. `shutdown` is cancelled when the
/// app should quit (from the web UI, Ctrl-C, or the window); `on_quit` runs once before exit.
pub fn run(url: String, shutdown: CancellationToken, runtime: tokio::runtime::Handle, on_quit: impl FnOnce() + 'static) -> ! {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // Quit requested elsewhere (web UI "quit" link, Ctrl-C): close the window too.
    {
        let proxy = proxy.clone();
        let shutdown = shutdown.clone();
        runtime.spawn(async move {
            shutdown.cancelled().await;
            let _ = proxy.send_event(UserEvent::Quit);
        });
    }

    let window = WindowBuilder::new()
        .with_title("Go Teacher")
        .with_inner_size(LogicalSize::new(1040.0, 800.0))
        .with_min_inner_size(LogicalSize::new(520.0, 400.0))
        .build(&event_loop)
        .expect("cannot create window");

    // Menu bar: an app menu with a clean-shutdown Quit, an Edit menu so copy/paste work in the webview.
    let quit_item = MenuItem::with_id("quit", "Quit Go Teacher", true, Some(Accelerator::new(Modifiers::META, Code::KeyQ)));
    let menu = Menu::new();
    let app_menu = Submenu::with_items(
        "Go Teacher",
        true,
        &[
            &PredefinedMenuItem::about(Some("About Go Teacher"), None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::separator(),
            &quit_item,
        ],
    )
    .expect("menu");
    let edit_menu = Submenu::with_items(
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ],
    )
    .expect("menu");
    let window_menu = Submenu::with_items("Window", true, &[&PredefinedMenuItem::minimize(None), &PredefinedMenuItem::close_window(None)]).expect("menu");
    menu.append(&app_menu).expect("menu");
    menu.append(&edit_menu).expect("menu");
    menu.append(&window_menu).expect("menu");
    #[cfg(target_os = "macos")]
    menu.init_for_nsapp();

    {
        let proxy = proxy.clone();
        let quit_id = quit_item.id().clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id == quit_id {
                let _ = proxy.send_event(UserEvent::Quit);
            }
        }));
    }

    let _webview = WebViewBuilder::new()
        .with_url(url)
        // Links that want a new tab (none in windowed mode, but be safe) go to the system browser.
        .with_new_window_req_handler(|url, _features| {
            open_external(&url);
            NewWindowResponse::Deny
        })
        // WebViews cannot download; hand the URL to the system browser instead.
        .with_download_started_handler(|url, _path| {
            open_external(&url);
            false
        })
        .build(&window)
        .expect("cannot create webview");

    let mut on_quit = Some(on_quit);
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let quit = matches!(
            event,
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } | Event::UserEvent(UserEvent::Quit)
        );
        if quit {
            shutdown.cancel();
            if let Some(f) = on_quit.take() {
                f();
            }
            *control_flow = ControlFlow::Exit;
        }
    })
}
