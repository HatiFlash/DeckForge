//! Isolated PI WebView2 process. The parent supplies a single URL on stdin, never argv.
//! Untrusted local markup is denied network resources before its first navigation.
use std::io::Read;
use raw_window_handle::HasWindowHandle;
use tao::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};
use wry::{NewWindowResponse, PermissionResponse, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};
use webview2_com::{CoTaskMemPWSTR, WebResourceRequestedEventHandler};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2_22, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
};
use windows62::{core::{HSTRING, Interface, PWSTR}, Win32::System::Com::IStream};

/// Accept only an exact, canonical loopback URL. The UUID is a manifest identifier, not a
/// hexadecimal GUID; the remaining path is a real, segment-encoded relative resource name.
pub(crate) fn allowed_initial(raw: &str) -> bool {
    if raw.len() > 2048 || !raw.is_ascii() || raw.bytes().any(|b| b.is_ascii_control()) {
        return false;
    }
    let Some(rest) = raw.strip_prefix("http://127.0.0.1:") else { return false };
    let Some((port, resource)) = rest.split_once('/') else { return false };
    if port.is_empty() || port.starts_with('0') || port.parse::<u16>().ok().filter(|p| *p > 0).is_none()
        || port.parse::<u16>().ok().is_none_or(|p| p.to_string() != port) { return false; }
    let Some((path, nonce)) = resource.split_once("?pi=") else { return false };
    if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) { return false; }
    let mut segments = path.split('/');
    let Some(uuid) = segments.next() else { return false };
    if uuid.is_empty() || uuid.len() > 128 || !uuid.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        || uuid == "." || uuid == ".." { return false; }
    let mut count = 0;
    for segment in segments {
        count += 1;
        if count > 32 || segment.is_empty() || segment == "." || segment == ".." { return false; }
        let mut decoded = Vec::new(); let bytes = segment.as_bytes(); let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                let Some(hex) = bytes.get(i+1..i+3).and_then(|s| std::str::from_utf8(s).ok()) else { return false };
                let Ok(b) = u8::from_str_radix(hex, 16) else { return false };
                // Only spaces may be percent-escaped; reject double encoding, slashes,
                // encoded dots and ambiguous aliases accepted differently by URL/WebView2/HTTP.
                if b != b' ' || hex != "20" { return false; }
                decoded.push(b); i += 3;
            } else {
                if !bytes[i].is_ascii_alphanumeric() && !matches!(bytes[i],b'.'|b'-'|b'_'|b' ') { return false; }
                // Spaces must be encoded in a URL.
                if bytes[i] == b' ' { return false; }
                decoded.push(bytes[i]); i += 1;
            }
        }
        if decoded == b"." || decoded == b".." || decoded.last() == Some(&b'.') || decoded.last() == Some(&b' ') { return false; }
    }
    if count == 0 { return false; }
    let Ok(url) = url::Url::parse(raw) else { return false };
    url.scheme() == "http" && url.host_str() == Some("127.0.0.1")
        && url.port().is_some() && url.username().is_empty() && url.password().is_none()
        && url.fragment().is_none() && url.as_str() == raw
}

/// The WebView2 resource event is the enforcement boundary; CSP and navigation are extra layers.
/// A PI may only talk to the loopback server that created its session (including SDK WebSocket).
/// Read-only BarRaider EasyPI files on jsDelivr, the most common remote PI dependency. It has no
/// licence allowing redistribution, so NovyDeck cannot bundle it; only these exact library
/// paths may be fetched by the page, never another CDN path, host or API.
pub(crate) fn allowed_cdn_library(raw: &str) -> bool {
    let Ok(u)=url::Url::parse(raw) else { return false };
    let path=u.path().to_ascii_lowercase();
    u.scheme()=="https" && u.host_str()==Some("cdn.jsdelivr.net") && u.port().is_none()
        && u.username().is_empty() && u.password().is_none() && u.query().is_none() && u.fragment().is_none()
        && (path.starts_with("/gh/barraider/streamdeck-easypi@") || path.starts_with("/gh/barraider/streamdeck-easypi-v2@"))
        && !path.contains("..") && (path.ends_with(".js") || path.ends_with(".css") || path.ends_with(".svg") || path.ends_with(".png"))
}

fn allowed_resource(raw: &str, initial: &str, ws_port: u16) -> bool {
    if raw == "about:blank" || raw.starts_with("data:") || allowed_cdn_library(raw) { return true; }
    let (Ok(target),Ok(origin))=(url::Url::parse(raw),url::Url::parse(initial)) else { return false };
    ((target.scheme()=="http" && target.port()==origin.port())
        || (target.scheme()=="ws" && target.port()==Some(ws_port)))
        && target.host_str()==Some("127.0.0.1") && target.username().is_empty()
        && target.password().is_none() && target.fragment().is_none()
        && target.as_str()==raw
}

/// HTML plugin backends may use the network like Node/native plugins, but never local files.
fn allowed_runtime_resource(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|u| matches!(u.scheme(), "http" | "https" | "ws" | "wss" | "data" | "blob" | "about"))
}

pub(crate) fn clean_title(title:&str)->String {
    title.chars().filter(|c|!c.is_control()).take(180).collect::<String>().trim().to_owned()
}

/// Associate the isolated window with NovyDeck before showing it. Never trust a stale HWND.
fn attach_owner(window:&tao::window::Window, owner:isize,parent_pid:u32) {
    use windows_sys::Win32::{Foundation::{HWND,RECT}, UI::WindowsAndMessaging::{GetWindowRect,GetWindowThreadProcessId,IsWindow,SetWindowLongPtrW,GWLP_HWNDPARENT}};
    let hwnd=owner as HWND;
    if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 { return; }
    let mut pid=0;
    unsafe { GetWindowThreadProcessId(hwnd,&mut pid); }
    if pid == 0 || pid != parent_pid { return; }
    if let Ok(handle)=window.window_handle() {
        if let raw_window_handle::RawWindowHandle::Win32(raw)=handle.as_raw() {
            unsafe { SetWindowLongPtrW(raw.hwnd.get() as HWND,GWLP_HWNDPARENT,owner); }
        }
    }
    let mut rect=RECT::default();
    if unsafe { GetWindowRect(hwnd,&mut rect) } != 0 {
        let size=window.outer_size();
        window.set_outer_position(tao::dpi::PhysicalPosition::new(
            rect.left+(rect.right-rect.left-size.width as i32)/2,
            rect.top+(rect.bottom-rect.top-size.height as i32)/2));
    }
}

pub fn run() -> anyhow::Result<()> {
    // One line only, bounded; parent closes the pipe after writing. Never echo nonce or URL.
    let mut input = String::new();
    std::io::stdin().take(4097).read_to_string(&mut input)?;
    let lines: Vec<&str> = input.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    anyhow::ensure!(lines.len() == 5 || (lines.len() == 3 && lines[2] == "runtime"), "invalid isolated inspector input");
    // "runtime": hidden backend page of an HTML plugin (same network trust as Node/native plugins).
    let runtime = lines.len() == 3;
    let url = lines[0];
    let ws = lines[1];
    anyhow::ensure!(input.len() <= 4096 && allowed_initial(url)
        && ws.parse::<u16>().ok().filter(|port|*port>0).is_some()
        && ws.parse::<u16>().unwrap().to_string()==ws,"invalid isolated inspector URL");
    let ws_port=ws.parse::<u16>()?;let url=url.to_owned();
    let title=if runtime { "NovyDeck HTML plugin".to_owned() } else { clean_title(lines[2]) };
    anyhow::ensure!(runtime || (!title.is_empty() && title==lines[2] && title.chars().count()<=180),"invalid inspector title");
    let owner=if runtime { 0 } else { lines[3].parse::<isize>()? };
    let parent_pid=if runtime { 0 } else { lines[4].parse::<u32>()? };
    let event_loop = EventLoop::new();
    let icon=image::load_from_memory(include_bytes!("../res/branding/novydeck-32.png"))?.into_rgba8();
    let window_icon=tao::window::Icon::from_rgba(icon.into_raw(),32,32)?;
    let window = WindowBuilder::new().with_title(title)
        .with_visible(false)
        .with_window_icon(Some(window_icon))
        .with_inner_size(tao::dpi::LogicalSize::new(520.0, 560.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(420.0, 360.0)).build(&event_loop)?;
    if !runtime { attach_owner(&window,owner,parent_pid); }
    let expected = url.clone();
    let proxy=if runtime {None} else {Some(crate::plugin_inspector_proxy::LocalProxy::start(&url,ws_port)?)};
    // Each isolated WebView2 process gets its own user-data directory so multiple instances
    // (e.g. a hidden HTML plugin runtime + an open PI with its own proxy) never collide on
    // CoreWebView2EnvironmentOptions.
    let data_dir=tempfile::Builder::new().prefix("deckforge-wv2-").tempdir()?;
    let mut web_context=wry::WebContext::new(Some(data_dir.path().to_path_buf()));
    // Build a blank view: no untrusted script runs before the native WebResourceRequested
    // filter is in place. A navigation handler alone can fire AFTER a network connection.
    let builder = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_incognito(true)
        .with_navigation_handler(move |target| target == expected)
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_permission_handler(|_| PermissionResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_drag_drop_handler(|_| true)
        .with_devtools(false)
        .with_browser_accelerator_keys(false)
        .with_default_context_menus(false);
    // Chromium's implicit loopback bypass must be disabled. The only proxy destination
    // allowed by LocalProxy is this PI's own HTTP server or its SDK WebSocket port.
    let builder=match &proxy {
        Some(proxy)=>builder.with_additional_browser_args(format!(
            "--proxy-server=http://127.0.0.1:{} --proxy-bypass-list=<-loopback> --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection",proxy.port)),
        None=>builder,
    };
    let webview=builder.build(&window)?;
    let core=webview.webview();
    let core22: ICoreWebView2_22=core.cast()?;
    unsafe {
        core22.AddWebResourceRequestedFilterWithRequestSourceKinds(
            &HSTRING::from("*"),COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL)?;
        let environment=webview.environment();let expected=url.clone();let mut token=0i64;
        core.add_WebResourceRequested(&WebResourceRequestedEventHandler::create(Box::new(move |_,args|{
            let Some(args)=args else {return Ok(())};
            let request=args.Request()?;let mut ptr=PWSTR::null();request.Uri(&mut ptr)?;
            let request_url=CoTaskMemPWSTR::from(ptr).to_string();
            let allowed=if runtime {allowed_runtime_resource(&request_url)} else {allowed_resource(&request_url,&expected,ws_port)};
            if !allowed {
                // Return a local 403; do not let WebView2 contact the denied origin.
                let response=environment.CreateWebResourceResponse(None::<&IStream>,403,
                    &HSTRING::from("Forbidden"),&HSTRING::from("Cache-Control: no-store"))?;
                args.SetResponse(&response)?;
            }
            Ok(())
        })),&mut token)?;
    }
    webview.load_url(&url)?;
    if !runtime { window.set_visible(true); }
    // Never print the URL or session key, including on error.
    println!("INSPECTOR_WEBVIEW_READY pid={}", std::process::id());
    event_loop.run(move |event, _, flow| {
        let _ = (&data_dir, &proxy);
        *flow = ControlFlow::Wait;
        if let Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
            *flow = ControlFlow::Exit;
        }
    });
}

#[cfg(test)]
mod cdn_tests { use super::*;
    #[test] fn only_easypi_library_files_are_allowed(){
        assert!(allowed_cdn_library("https://cdn.jsdelivr.net/gh/barraider/streamdeck-easypi-v2@latest/src/sdtools.common.js"));
        assert!(allowed_cdn_library("https://cdn.jsdelivr.net/gh/BarRaider/streamdeck-easypi@latest/src/sdpi.css"));
        assert!(allowed_runtime_resource("https://pi.hole/admin/api.php") && !allowed_runtime_resource("file:///C:/Windows/win.ini"));
        for bad in ["https://cdn.jsdelivr.net/gh/evil/x@1/a.js","https://cdn.jsdelivr.net/npm/x/a.js","http://cdn.jsdelivr.net/gh/barraider/streamdeck-easypi@latest/src/a.js",
            "https://cdn.jsdelivr.net/gh/barraider/streamdeck-easypi@latest/src/a.js?x=1","https://evil.test/gh/barraider/streamdeck-easypi@latest/a.js",
            "https://cdn.jsdelivr.net/gh/barraider/streamdeck-easypi@latest/src/data.json","https://user@cdn.jsdelivr.net/gh/barraider/streamdeck-easypi@latest/a.js"]{
            assert!(!allowed_cdn_library(bad),"{bad}");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::allowed_initial;
    const VALID: &str = "http://127.0.0.1:45678/com.elgato.counter/property%20inspector/index.html?pi=0123456789abcdef0123456789abcdef";
    #[test] fn clean_title_removes_controls_and_caps_length() {
        assert_eq!(super::clean_title(" Écran\r\n Titre\t"),"Écran Titre");
        assert_eq!(super::clean_title(&"A".repeat(400)).chars().count(),180);
    }
    #[test]
    fn pi_url_allowlist() {
        assert!(allowed_initial(VALID));
        for candidate in [
            "https://127.0.0.1:45678/com.elgato.counter/property%20inspector/index.html?pi=0123456789abcdef0123456789abcdef",
            "http://localhost:45678/com.elgato.counter/property%20inspector/index.html?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.2:45678/com.elgato.counter/property%20inspector/index.html?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.1/com.elgato.counter/property%20inspector/index.html?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:45678/com.elgato.counter/path?pi=short",
            "http://127.0.0.1:45678/com.elgato.counter/path?pi=0123456789abcdef0123456789abcdef&x=1",
            "http://127.0.0.1:45678/com.elgato.counter/path?pi=0123456789abcdef0123456789abcdef#frag",
            "http://evil@127.0.0.1:45678/com.elgato.counter/path?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:45678/com.elgato.counter/../path?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:45678/com.elgato.counter/%2e%2e/path?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:45678/com.elgato.counter/%252e%252e/path?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:45678/com.elgato.counter/a%2fb?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:45678/com.elgato.counter/a\\b?pi=0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:45678/com.elgato.counter/a%20b%20?pi=0123456789abcdef0123456789abcdef",
            "file:///C:/secret",
        ] { assert!(!allowed_initial(candidate), "accepted {candidate}"); }
    }
}
