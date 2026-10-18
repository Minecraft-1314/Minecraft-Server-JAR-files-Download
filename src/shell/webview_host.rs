use std::path::{Path, PathBuf};
use std::sync::Arc;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Icon, Window, WindowBuilder};
use wry::http::{Request, Response, StatusCode};
use wry::{WebView, WebViewBuilder};

use crate::bridge::command_router::{CommandRouter, EventSink};
use crate::core::app_log;
use crate::core::error::AppError;
use crate::core::url_guard::is_public_url;

const PROTOCOL: &str = "app";
const VIRTUAL_HOST: &str = "app.localhost";
const ENTRY_PAGE: &str = "http://app.localhost/index.html";
const MAX_MESSAGE_CHARACTERS: usize = 2_000_000;

const BRIDGE_SHIM: &str = r#"(function () {
  var listeners = [];
  window.__hostPush = function (text) {
    var data = typeof text === "string" ? JSON.parse(text) : text;
    for (var index = 0; index < listeners.length; index += 1) {
      try { listeners[index]({ data: data }); } catch (error) { }
    }
  };
  var chromeRef = window.chrome || {};
  var nativeWebview = chromeRef.webview;
  var nativePost = nativeWebview && typeof nativeWebview.postMessage === "function"
    ? nativeWebview.postMessage.bind(nativeWebview)
    : null;
  window.chrome = chromeRef;
  window.chrome.webview = {
    postMessage: function (message) {
      if (!nativePost) throw new Error("bridge_unavailable");
      nativePost(typeof message === "string" ? message : JSON.stringify(message));
    },
    addEventListener: function (type, handler) {
      if (type === "message" && typeof handler === "function") listeners.push(handler);
    },
    removeEventListener: function (type, handler) {
      var index = listeners.indexOf(handler);
      if (index >= 0) listeners.splice(index, 1);
    }
  };
})();"#;
const WINDOW_WIDTH: f64 = 1320.0;
const WINDOW_HEIGHT: f64 = 860.0;
const MIN_WIDTH: f64 = 360.0;
const MIN_HEIGHT: f64 = 640.0;

pub enum UserEvent {
    Response(String),
    Exit,
}

pub struct WebViewHost {
    router: Arc<CommandRouter>,
}

impl WebViewHost {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            router: Arc::new(CommandRouter::new(Arc::new(ProxySink { proxy }))),
        }
    }
}

struct ProxySink {
    proxy: EventLoopProxy<UserEvent>,
}

impl EventSink for ProxySink {
    fn post(&self, payload: serde_json::Value) {
        if let Ok(text) = serde_json::to_string(&payload) {
            let _ = self.proxy.send_event(UserEvent::Response(text));
        }
    }
}

pub fn run() -> i32 {
    app_log::bind();
    let _ = crate::core::config_store::load();
    let mut builder = EventLoopBuilder::<UserEvent>::with_user_event();
    let event_loop = builder.build();
    let proxy = event_loop.create_proxy();
    let host = WebViewHost::new(proxy.clone());
    let router = host.router.clone();
    let web_directory = match resolve_web_directory() {
        Ok(value) => value,
        Err(error) => {
            app_log::handle_unhandled(error.message());
            show_fallback(&error);
            return 1;
        }
    };
    let window = match build_window(&event_loop) {
        Ok(value) => value,
        Err(error) => {
            app_log::handle_unhandled(&error.to_string());
            return 1;
        }
    };
    let webview = match build_webview(
        &window,
        web_directory.clone(),
        router.clone(),
        proxy.clone(),
    ) {
        Ok(value) => value,
        Err(error) => {
            app_log::handle_unhandled(&error.to_string());
            show_fallback(&AppError::general(error.to_string()));
            return 1;
        }
    };
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                router.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Response(text)) => {
                let _ = webview.evaluate_script(&format!("window.__hostPush({text});"));
            }
            Event::UserEvent(UserEvent::Exit) => {
                router.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn build_window(
    event_loop: &tao::event_loop::EventLoop<UserEvent>,
) -> Result<Window, tao::error::OsError> {
    let mut builder = WindowBuilder::new()
        .with_title("Minecraft Server Downloader")
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
        .with_min_inner_size(LogicalSize::new(MIN_WIDTH, MIN_HEIGHT));
    if let Some(icon) = load_icon() {
        builder = builder.with_window_icon(Some(icon));
    }
    builder.build(event_loop)
}

fn load_icon() -> Option<Icon> {
    let path = std::env::current_exe().ok()?.parent()?.join("app.ico");
    if !path.is_file() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let directory = ico::IconDir::read(std::io::Cursor::new(bytes)).ok()?;
    let entry = directory
        .entries()
        .iter()
        .max_by_key(|entry| entry.width() * entry.height())?;
    let image = entry.decode().ok()?;
    Icon::from_rgba(image.rgba_data().to_vec(), image.width(), image.height()).ok()
}

fn resolve_user_data_directory() -> PathBuf {
    std::env::temp_dir()
        .join("MinecraftServerDownloader")
        .join("WebView2")
}

fn resolve_web_directory() -> Result<PathBuf, AppError> {
    let executable =
        std::env::current_exe().map_err(|error| AppError::general(error.to_string()))?;
    let base = executable
        .parent()
        .ok_or_else(|| AppError::general("bridge.failed"))?;
    let directory = base.join("Ui").join("Web");
    if !directory.is_dir() {
        return Err(AppError::general(format!(
            "resource.missing: {}",
            directory.to_string_lossy()
        )));
    }
    Ok(directory)
}

fn build_webview(
    window: &Window,
    web_directory: PathBuf,
    router: Arc<CommandRouter>,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<WebView, wry::Error> {
    let directory = web_directory.clone();
    let handler_router = router.clone();
    let handler_proxy = proxy.clone();
    let mut context = wry::WebContext::new(Some(resolve_user_data_directory()));
    WebViewBuilder::with_web_context(&mut context)
        .with_custom_protocol(PROTOCOL.to_string(), move |_id, request| {
            serve(&directory, request)
        })
        .with_initialization_script(BRIDGE_SHIM)
        .with_ipc_handler(move |request| {
            let response = handle_message(&handler_router, request.body());
            if let Ok(text) = serde_json::to_string(&response) {
                let _ = handler_proxy.send_event(UserEvent::Response(text));
            }
        })
        .with_navigation_handler(move |url| {
            if is_internal_url(&url) {
                return true;
            }
            if is_public_url(&url) {
                let _ = crate::bridge::external_launcher::open(&url);
            }
            false
        })
        .with_devtools(false)
        .with_url(ENTRY_PAGE)
        .build(window)
}

fn is_internal_url(url: &str) -> bool {
    let prefix = format!("http://{VIRTUAL_HOST}/");
    let secure = format!("https://{VIRTUAL_HOST}/");
    let bare = format!("{PROTOCOL}://");
    url.starts_with(&prefix) || url.starts_with(&secure) || url.starts_with(&bare)
}

fn serve(directory: &Path, request: Request<Vec<u8>>) -> Response<std::borrow::Cow<'static, [u8]>> {
    let relative = request_path(request.uri());
    if relative.contains("..") {
        return forbidden();
    }
    let target = directory.join(&relative);
    match std::fs::read(&target) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", content_type(&relative))
            .header("Access-Control-Allow-Origin", "*")
            .body(std::borrow::Cow::Owned(bytes))
            .unwrap_or_else(|_| not_found()),
        Err(_) => not_found(),
    }
}

fn request_path(uri: &wry::http::Uri) -> String {
    let raw = uri.to_string();
    let authority = raw.find("://").map(|index| index + 3).unwrap_or(0);
    let after = &raw[authority..];
    let start = after.find('/').unwrap_or(after.len());
    let path = &after[start..];
    let path = match path.find(['?', '#']) {
        Some(index) => &path[..index],
        None => path,
    };
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        "index.html".to_string()
    } else {
        trimmed.to_string()
    }
}

fn not_found() -> Response<std::borrow::Cow<'static, [u8]>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(std::borrow::Cow::Borrowed(&b"not found"[..]))
        .expect("valid response")
}

fn forbidden() -> Response<std::borrow::Cow<'static, [u8]>> {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(std::borrow::Cow::Borrowed(&b"forbidden"[..]))
        .expect("valid response")
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "map" => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn handle_message(router: &CommandRouter, raw: &str) -> serde_json::Value {
    if raw.chars().count() > MAX_MESSAGE_CHARACTERS {
        return envelope(0, false, serde_json::Value::Null, "bridge.tooLarge");
    }
    let root: serde_json::Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(_) => {
            app_log::warn("bridge.json.invalid");
            return envelope(0, false, serde_json::Value::Null, "bridge.invalidJson");
        }
    };
    let id = crate::bridge::json::long(&root, "id").unwrap_or(0);
    let action = crate::bridge::json::text(&root, "action", "");
    let payload = crate::bridge::json::object(&root, "payload");
    match router.execute(&action, &payload) {
        Ok(result) => envelope(id, true, result, ""),
        Err(error) if error.is_validation() => {
            envelope(id, false, serde_json::Value::Null, error.message())
        }
        Err(error) => {
            if error.message() == "operation.cancelled" {
                envelope(id, false, serde_json::Value::Null, "operation.cancelled")
            } else {
                app_log::handle_unhandled(error.message());
                envelope(id, false, serde_json::Value::Null, "bridge.failed")
            }
        }
    }
}

fn envelope(id: i64, ok: bool, result: serde_json::Value, error: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "ok": ok,
        "result": result,
        "error": if error.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(error.to_string()) },
    })
}

fn show_fallback(error: &AppError) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let chinese = std::env::var("LANG")
        .map(|value| value.to_ascii_lowercase().starts_with("zh"))
        .unwrap_or(true);
    let message = if chinese {
        format!(
            "界面无法启动。\n\n{}\n\n请确认 WebView2 运行时已安装，界面资源完整。",
            error.message()
        )
    } else {
        format!(
            "Interface failed to start.\n\n{}\n\nVerify the WebView2 Runtime and the bundled UI resources.",
            error.message()
        )
    };
    let text: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "Minecraft Server Downloader"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
