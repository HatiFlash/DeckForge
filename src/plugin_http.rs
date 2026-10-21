//! Serveur HTTP local de ressources HTML des plugins, sans listing ni traversée.
use std::{fs,io::{Read,Write},net::{TcpListener,TcpStream},path::{Path,PathBuf},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},thread,time::Duration};

/// Registry marker for an HTML *plugin runtime* session (CodePath .html), distinct from PI sessions.
pub const RUNTIME_SESSION:&str="\u{1}runtime";
pub type Inspectors=Arc<Mutex<std::collections::HashMap<String,String>>>;
pub struct HtmlServer { pub port:u16, pub inspectors:Inspectors, stop:Arc<AtomicBool>, worker:Option<thread::JoinHandle<()>> }

/// Décode %XX (UTF-8 strict) : les chemins de manifeste contiennent souvent des espaces (« property inspector/… »).
fn percent_decode(s:&str)->Option<String>{
    let b=s.as_bytes();let mut out=Vec::with_capacity(b.len());let mut i=0;
    while i<b.len(){ if b[i]==b'%' { let h=std::str::from_utf8(b.get(i+1..i+3)?).ok()?; out.push(u8::from_str_radix(h,16).ok()?); i+=3; } else { out.push(b[i]); i+=1; } }
    String::from_utf8(out).ok()
}
/// Habillage NovyDeck des pages de réglages (Property Inspector) : mêmes couleurs que l'application,
/// champs plus lisibles. Purement visuel, injecté après les styles du plugin.
const PI_THEME:&str=r#"<style id="deckforge-pi-theme">
:root{color-scheme:dark}
html,body{background:#171a21!important;color:#e7e9ee!important;font-family:"Segoe UI Variable Text","Segoe UI",system-ui,sans-serif!important;font-size:14px!important}
body{padding:14px 16px!important;margin:0!important}
.sdpi-wrapper{max-width:none!important;padding:0!important}
.sdpi-item{display:flex!important;flex-direction:column!important;align-items:stretch!important;gap:6px!important;margin:0 0 14px!important;min-height:0!important}
.sdpi-item-label{width:auto!important;min-width:0!important;max-width:none!important;text-align:left!important;padding:0!important;color:#9aa3b2!important;font-size:12px!important;font-weight:600!important;letter-spacing:.02em!important;text-transform:uppercase!important}
.sdpi-item-value,.sdpi-item>*:not(.sdpi-item-label){width:100%!important;max-width:none!important;margin:0!important;padding:0!important}
select,input[type=text],input[type=number],input[type=password],textarea{background:#232835!important;color:#e7e9ee!important;border:1px solid #343b4c!important;border-radius:8px!important;padding:8px 10px!important;min-height:36px!important;font-size:14px!important;width:100%!important;box-sizing:border-box!important;outline:none!important}
select:focus,input:focus,textarea:focus{border-color:#4f8cff!important}
option{background:#232835;color:#e7e9ee}
input[type=radio],input[type=checkbox]{accent-color:#4f8cff}
input[type=range]{accent-color:#4f8cff;width:100%!important}
label{color:#e7e9ee!important}
button,.sdpi-item-value button{background:#4f8cff!important;color:#fff!important;border:0!important;border-radius:8px!important;padding:8px 14px!important;cursor:pointer}
a{color:#7aa7ff!important}
::-webkit-scrollbar{width:10px}::-webkit-scrollbar-thumb{background:#343b4c;border-radius:6px}
</style>"#;
fn safe_file(root:&Path,request:&str,uuid:&str)->Option<PathBuf>{
    let url=request.split('?').next()?;
    let rel=percent_decode(url.strip_prefix('/')?.strip_prefix(uuid)?.strip_prefix('/')?)?;
    // No second decoding pass, Windows alternate streams, implicit trailing-dot aliases or
    // browser/HTTP interpretations of the same resource under a different URL.
    if rel.is_empty() || rel.contains('%') || rel.contains('\\') || rel.contains(':') || rel.contains('\0')
        || rel.chars().any(|c| c.is_control() || !c.is_ascii()) {return None;}
    let path=Path::new(&rel);
    if rel.split('/').any(|part| part.is_empty() || part=="." || part==".." || part.ends_with('.') || part.ends_with(' ')) {return None;}
    if !path.components().all(|p| matches!(p,std::path::Component::Normal(_))) {return None;}
    // Refuser tout lien symbolique dans chaque composant, même s’il pointe dans la racine.
    let mut cursor=root.to_path_buf();
    for part in path.components(){cursor.push(part);if fs::symlink_metadata(&cursor).ok()?.file_type().is_symlink(){return None;}}
    let real=cursor.canonicalize().ok()?;
    if real.is_file() && real.starts_with(root.canonicalize().ok()?) {Some(real)} else {None}
}
#[cfg(windows)]
fn opened_within_root(root:&Path,file:&fs::File)->bool{
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
    let mut name=vec![0u16;32768];
    let n=unsafe{GetFinalPathNameByHandleW(file.as_raw_handle() as _,name.as_mut_ptr(),name.len() as u32,0)} as usize;
    if n==0 || n>=name.len(){return false;}
    let Ok(actual)=String::from_utf16(&name[..n]) else{return false};
    let Ok(root)=root.canonicalize() else{return false};
    fn norm(s:&str)->String{s.strip_prefix(r"\\?\").unwrap_or(s).replace('/',"\\").trim_end_matches('\\').to_ascii_lowercase()}
    let root=norm(&root.to_string_lossy());
    norm(&actual).starts_with(&format!("{root}\\"))
}
#[cfg(not(windows))]
fn opened_within_root(root:&Path,file:&fs::File)->bool{
    use std::os::fd::AsRawFd;
    // Linux test bench only: file descriptor resolves the opened inode rather than re-opening by name.
    let Ok(path)=fs::read_link(format!("/proc/self/fd/{}",file.as_raw_fd())) else{return false};
    root.canonicalize().is_ok_and(|r|path.starts_with(r))
}
fn mime(path:&Path)->&'static str {match path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase().as_str(){
    "html"|"htm"=>"text/html; charset=utf-8", "js"|"mjs"=>"text/javascript; charset=utf-8", "css"=>"text/css; charset=utf-8", "svg"=>"image/svg+xml", "png"=>"image/png", "jpg"|"jpeg"=>"image/jpeg", "gif"=>"image/gif", "webp"=>"image/webp", "ico"=>"image/x-icon", "webm"=>"video/webm", "mp4"=>"video/mp4", "mp3"=>"audio/mpeg", "wav"=>"audio/wav", "ogg"=>"audio/ogg", "json"=>"application/json", "woff"=>"font/woff", "woff2"=>"font/woff2", "ttf"=>"font/ttf", "otf"=>"font/otf", _=>"application/octet-stream"}}
fn response(root:&Path,uuid:&str,port:u16,info:&str,inspectors:&Inspectors,request:&str)->(&'static str,&'static str,Vec<u8>){
    let Some(path)=safe_file(root,request,uuid) else {return ("404 Not Found","text/plain",b"Not found".to_vec())};
    let content_type=mime(&path);
    // Cap the response before allocation; this endpoint must never stream an unbounded plugin file.
    let Ok(file)=fs::File::open(path) else {return ("404 Not Found","text/plain",b"Not found".to_vec())};
    let Ok(meta)=file.metadata() else {return ("404 Not Found","text/plain",b"Not found".to_vec())};
    if !opened_within_root(root,&file){return ("404 Not Found","text/plain",b"Not found".to_vec())};
    if !meta.is_file() || meta.len()>1024*1024 {return ("413 Content Too Large","text/plain",b"Resource too large".to_vec())}
    let mut bytes=Vec::new();
    if file.take(1024*1024+1).read_to_end(&mut bytes).is_err() || bytes.len()>1024*1024 {
        return ("413 Content Too Large","text/plain",b"Resource too large".to_vec());
    }
    if !content_type.starts_with("text/html") {return ("200 OK",content_type,bytes)}
    // Only an active PI session can load HTML. An invalid query must not fall back to registerPlugin.
    let pi=request.split_once('?').and_then(|(_,q)|q.strip_prefix("pi="))
        .filter(|id|id.len()==32 && id.bytes().all(|b|b.is_ascii_hexdigit()))
        .and_then(|id|inspectors.lock().ok()?.get(id).cloned().map(|data|(id.to_owned(),data)));
    let Some((pi_id,data))=pi else {return ("404 Not Found","text/plain",b"Not found".to_vec())};
    let Ok(html)=String::from_utf8(bytes) else {return ("415 Unsupported Media Type","text/plain",b"Invalid HTML".to_vec())};
    let mut html=rewrite_known_libraries(&html);
    let info=&with_default_colors(info);
    let runtime=data==RUNTIME_SESSION;
    let script=if runtime {
        // Plugin backend page: Stream Deck calls connectElgatoStreamDeckSocket(port, uuid, event, info).
        let token=serde_json::to_string(&pi_id).unwrap_or_default();
        format!(r#"<script>window.addEventListener('load',function(){{var f=window.connectElgatoStreamDeckSocket||window.connectElgatoSdkSocket||window.connectSocket;if(typeof f==='function')f({port},{token},'registerPlugin',{info});}});</script>"#,
            info=serde_json::to_string(info).unwrap_or_default())
    } else {
        let pi_id=serde_json::to_string(&pi_id).unwrap_or_default();
        format!(r#"<script>window.addEventListener('load',function(){{var f=window.connectElgatoStreamDeckSocket||window.connectElgatoSdkSocket||window.connectSocket;if(typeof f==='function')f({port},{pi_id},'registerPropertyInspector',{info},{data});if(!f||(f.length===0&&String(f).replace(/\s+/g,'').slice(-2)==='{{}}')){{var ws=new WebSocket('ws://127.0.0.1:{port}');ws.onopen=function(){{ws.send(JSON.stringify({{event:'registerPropertyInspector',uuid:{pi_id}}}));}};}}}});</script>{PI_THEME}"#,
            info=serde_json::to_string(&info).unwrap_or_default())
    };
    if let Some(i)=html.to_ascii_lowercase().rfind("</body>"){html.insert_str(i,&script);}else{html.push_str(&script);}
    if diag_enabled() {
        // Debug builds only: report page errors to the local server log for compatibility triage.
        let probe=r#"<script>(function(){function r(m){try{new Image().src='/__diag?'+encodeURIComponent(String(m).slice(0,900));}catch(e){}}window.addEventListener('error',function(e){r('error '+e.message+' @'+(e.filename||(e.target&&(e.target.src||e.target.href))||'')+':'+(e.lineno||'')+' '+((e.error&&e.error.stack)||'').slice(0,400));},true);var ce=console.error;console.error=function(){r('console '+Array.prototype.join.call(arguments,' '));return ce.apply(console,arguments);};window.addEventListener('load',function(){r('load connect='+typeof window.connectElgatoStreamDeckSocket);});})();</script>"#;
        let lower=html.to_ascii_lowercase();
        let at=lower.find("<head>").map(|i|i+6).unwrap_or(0);html.insert_str(at,probe);
    }
    ("200 OK",content_type,html.into_bytes())
}
/// MIT-licensed Elgato PI component library, vendored so common PIs work fully offline.
/// Pages referencing the official CDN are rewritten to these same-origin copies.
const SDPI_V3:&[u8]=include_bytes!("../third_party/sdpi-components/v3/sdpi-components.js");
const SDPI_V4:&[u8]=include_bytes!("../third_party/sdpi-components/v4/sdpi-components.js");
fn vendored(target:&str)->Option<&'static [u8]>{match target{"/__deckforge/sdpi-components-v3.js"=>Some(SDPI_V3),"/__deckforge/sdpi-components-v4.js"=>Some(SDPI_V4),_=>None}}
fn rewrite_known_libraries(html:&str)->String{
    let mut out=html.to_owned();
    for v in ["v3","v4"] {
        for prefix in ["https://sdpi-components.dev","http://sdpi-components.dev","//sdpi-components.dev"] {
            out=out.replace(&format!("{prefix}/releases/{v}/sdpi-components.js"),&format!("/__deckforge/sdpi-components-{v}.js"));
        }
    }
    out
}
/// Stream Deck always passes theme colors; EasyPI/sdtools crash on `colors.*.replace` without them.
fn with_default_colors(info:&str)->String{
    let Ok(mut value)=serde_json::from_str::<serde_json::Value>(info) else {return info.to_owned()};
    if let Some(obj)=value.as_object_mut() {
        let colors=obj.entry("colors").or_insert_with(||serde_json::json!({}));
        if !colors.is_object() {*colors=serde_json::json!({});}
        let colors=colors.as_object_mut().unwrap();
        for (k,v) in [("buttonPressedBackgroundColor","#303030FF"),("buttonPressedBorderColor","#646464FF"),
            ("buttonMouseOverBackgroundColor","#464646FF"),("buttonPressedTextColor","#969696FF"),
            ("disabledColor","#4F8CFF59"),("highlightColor","#4F8CFFFF"),("mouseDownColor","#2F6FE0FF")] {
            colors.entry(k).or_insert_with(||serde_json::json!(v));
        }
    }
    value.to_string()
}
fn diag_enabled()->bool{cfg!(debug_assertions) && std::env::var_os("DECKFORGE_PI_DIAG").is_some()}
fn serve(mut stream:TcpStream,root:&Path,uuid:&str,port:u16,info:&str,inspectors:&Inspectors){
      let _=stream.set_nonblocking(false); // Hérité de l'écouteur sous Windows : sinon une requête lente est perdue.
    let _=stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _=stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut buf=[0u8;8193];let Ok(n)=stream.read(&mut buf) else{return};
    if n==buf.len() || !buf[..n].windows(4).any(|s|s==b"\r\n\r\n") {return;}
    let request=String::from_utf8_lossy(&buf[..n]);
    let mut fields=request.lines().next().unwrap_or("").split_whitespace();
    let method=fields.next().unwrap_or("");let target=fields.next().unwrap_or("");
    if diag_enabled() && method=="GET" && target.starts_with("/__diag?") {
        eprintln!("PI_DIAG {}",percent_decode(&target[8..]).unwrap_or_default());
        let _=stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");return;
    }
    let (status,mime,body)=if method=="GET" && vendored(target).is_some() {("200 OK","text/javascript; charset=utf-8",vendored(target).unwrap().to_vec())}
        else if method=="GET" {response(root,uuid,port,info,inspectors,target)}else{("405 Method Not Allowed","text/plain",b"GET only".to_vec())};
    // Offline: only packaged resources and the exact SDK WebSocket port, never external services.
    // HTML plugin runtime pages have the same network trust as Node/native plugins.
    let runtime=target.split_once("?pi=").map(|(_,id)|id).filter(|id|id.len()==32)
        .is_some_and(|id|inspectors.lock().ok().is_some_and(|r|r.get(id).is_some_and(|v|v==RUNTIME_SESSION)));
    let csp=if runtime {"default-src 'self' 'unsafe-inline' 'unsafe-eval' data: blob: http: https: ws: wss:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'".to_owned()}
        else {format!("default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'; frame-src 'none'; connect-src 'self' ws://127.0.0.1:{port} ws://localhost:{port}; script-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net/gh/barraider/ https://cdn.jsdelivr.net/gh/BarRaider/; style-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net/gh/barraider/ https://cdn.jsdelivr.net/gh/BarRaider/; img-src 'self' data: https://cdn.jsdelivr.net/gh/barraider/ https://cdn.jsdelivr.net/gh/BarRaider/; media-src 'self' data: blob:; font-src 'self' data:; worker-src 'none'; manifest-src 'none'")};
    let header=format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nContent-Security-Policy: {csp}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len());
    let _=stream.write_all(header.as_bytes());let _=stream.write_all(&body);
}
impl HtmlServer {
    pub fn start(root:PathBuf,uuid:String,info:String,ws_port:u16)->std::io::Result<Self>{
        let listener=TcpListener::bind("127.0.0.1:0")?;listener.set_nonblocking(true)?;
        let port=listener.local_addr()?.port();let inspectors:Inspectors=Arc::default();let registry=inspectors.clone();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
        let worker=thread::spawn(move||while !flag.load(Ordering::Relaxed){match listener.accept(){Ok((stream,_))=>serve(stream,&root,&uuid,ws_port,&info,&registry),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>thread::sleep(Duration::from_millis(25)),Err(_)=>break}});
        Ok(Self{port,inspectors,stop,worker:Some(worker)})
    }
}
impl Drop for HtmlServer {fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);if let Some(worker)=self.worker.take(){let _=worker.join();}}}

#[cfg(test)] mod tests {use super::*;
 #[test] fn property_inspector_injection(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("pi.html"),"<body>PI</body>").unwrap();let inspectors=Inspectors::default();let id="a".repeat(32);inspectors.lock().unwrap().insert(id.clone(),"JSON.stringify({action:'test',context:'ctx'})".into());let (_,_,body)=response(tmp.path(),"plugin",345,"{}",&inspectors,&format!("/plugin/pi.html?pi={id}"));let html=String::from_utf8(body).unwrap();assert!(html.contains("connectElgatoSdkSocket"));assert!(html.contains("registerPropertyInspector"));assert!(html.contains("action:'test'"));assert!(!html.contains("registerPlugin"));assert_eq!(response(tmp.path(),"plugin",345,"{}",&inspectors,"/plugin/pi.html?pi=unknown").0,"404 Not Found");}
 #[test] fn html_injected_and_confined(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("index.html"),"<body>Bonjour</body>").unwrap();fs::write(tmp.path().join("style.css"),"a{}").unwrap();
 let inspectors=Inspectors::default();let id="b".repeat(32);inspectors.lock().unwrap().insert(id.clone(),"JSON.stringify({})".into());let (status,mime,body)=response(tmp.path(),"p",123,"{}",&inspectors,&format!("/p/index.html?pi={id}"));assert_eq!(status,"200 OK");assert_eq!(mime,"text/html; charset=utf-8");let body=String::from_utf8(body).unwrap();assert!(body.contains("connectElgatoStreamDeckSocket"));assert!(body.find("<script>").unwrap()<body.find("</body>").unwrap());
 assert_eq!(response(tmp.path(),"p",123,"{}",&inspectors,"/p/style.css").1,"text/css; charset=utf-8");
 for url in ["/p/../index.html","/p/%2e%2e/index.html","/p/","/another/index.html","/p/..\\index.html"] {assert_eq!(response(tmp.path(),"p",123,"{}",&inspectors,url).0,"404 Not Found", "{url}");}
 }
 #[test] fn spaces_in_path_are_decoded(){let tmp=tempfile::tempdir().unwrap();fs::create_dir(tmp.path().join("property inspector")).unwrap();fs::write(tmp.path().join("property inspector").join("pi.html"),"<body>x</body>").unwrap();let inspectors=Inspectors::default();let id="c".repeat(32);inspectors.lock().unwrap().insert(id.clone(),"JSON.stringify({})".into());assert_eq!(response(tmp.path(),"p",1,"{}",&inspectors,&format!("/p/property%20inspector/pi.html?pi={id}")).0,"200 OK");for bad in ["/p/property%20inspector/..%2fpi.html","/p/%2e%2e%2fx","/p/a%5cb","/p/%zz"]{assert_eq!(response(tmp.path(),"p",1,"{}",&inspectors,bad).0,"404 Not Found","{bad}");}}
#[test] fn slow_client_request_is_still_served(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("style.css"),"a{}").unwrap();let server=HtmlServer::start(tmp.path().to_owned(),"p".into(),"{}".into(),1).unwrap();let mut s=TcpStream::connect(("127.0.0.1",server.port)).unwrap();thread::sleep(Duration::from_millis(300));s.write_all(b"GET /p/style.css HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();let mut out=String::new();s.read_to_string(&mut out).unwrap();assert!(out.starts_with("HTTP/1.1 200"),"{out}");}
#[test] fn http_serves_without_listing(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("style.css"),"a{}").unwrap();let server=HtmlServer::start(tmp.path().to_owned(),"p".into(),"{}".into(),123).unwrap();let mut stream=TcpStream::connect(("127.0.0.1",server.port)).unwrap();stream.write_all(b"GET /p/style.css HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();let mut bytes=Vec::new();stream.read_to_end(&mut bytes).unwrap();assert!(String::from_utf8(bytes).unwrap().contains("HTTP/1.1 200 OK"));}
#[test] fn oversized_resource_and_bad_paths_are_rejected(){
 let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("big.js"),vec![b'a';1024*1024+1]).unwrap();
 let inspectors=Inspectors::default();
 assert_eq!(response(tmp.path(),"p",123,"{}",&inspectors,"/p/big.js").0,"413 Content Too Large");
 for path in ["/p/%2e%2e/big.js","/p/%00big.js","/p/%2Fbig.js","/p/a%5cbig.js","/q/big.js","/p/%252e%252e/big.js","/p/%255c/big.js","/p/big.js%20","/p/a./big.js","/p//big.js","/p/%c0%af/big.js"]{
     assert_eq!(response(tmp.path(),"p",123,"{}",&inspectors,path).0,"404 Not Found","{path}");
 }
}
#[test] fn listener_sets_deny_csp_and_no_cors(){
 let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("pi.html"),"<body>x</body>").unwrap();fs::write(tmp.path().join("v.webm"),b"w").unwrap();
 let server=HtmlServer::start(tmp.path().to_owned(),"p".into(),"{}".into(),123).unwrap();
 assert_eq!(response(tmp.path(),"p",123,"{}",&server.inspectors,"/p/v.webm").1,"video/webm");
 let mut s=TcpStream::connect(("127.0.0.1",server.port)).unwrap();
 s.write_all(b"GET /p/pi.html HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
 let mut out=String::new();s.read_to_string(&mut out).unwrap();
 assert!(out.contains("Content-Security-Policy: default-src 'none'"));assert!(out.contains("media-src 'self' data: blob:"));assert!(out.contains("connect-src 'self' ws://127.0.0.1:123 ws://localhost:123"));assert!(!out.replace("https://cdn.jsdelivr.net/gh/barraider/","").replace("https://cdn.jsdelivr.net/gh/BarRaider/","").contains("https:"));
 assert!(!out.contains("Access-Control-Allow-Origin"));
}
 #[test] fn known_cdn_component_library_is_served_locally_and_colors_are_supplied(){
  let html=rewrite_known_libraries(r#"<script src="https://sdpi-components.dev/releases/v4/sdpi-components.js"></script><script src="https://sdpi-components.dev/releases/v3/sdpi-components.js"></script>"#);
  assert!(!html.contains("sdpi-components.dev"));assert!(html.contains("/__deckforge/sdpi-components-v4.js"));
  assert!(vendored("/__deckforge/sdpi-components-v4.js").is_some_and(|b|b.starts_with(b"/**!")));
  assert!(vendored("/__deckforge/../x").is_none());
  let info:serde_json::Value=serde_json::from_str(&with_default_colors(r#"{"application":{},"colors":{}}"#)).unwrap();
  assert!(info["colors"]["highlightColor"].as_str().unwrap().starts_with('#'));
  let kept:serde_json::Value=serde_json::from_str(&with_default_colors(r##"{"colors":{"highlightColor":"#000000FF"}}"##)).unwrap();
  assert_eq!(kept["colors"]["highlightColor"],"#000000FF");
 }
 #[test] fn html_runtime_page_registers_as_plugin_only_with_runtime_token(){
  let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("app.html"),"<body>app</body>").unwrap();
  let server=HtmlServer::start(tmp.path().to_owned(),"p".into(),"{}".into(),321).unwrap();
  let id="d".repeat(32);server.inspectors.lock().unwrap().insert(id.clone(),RUNTIME_SESSION.into());
  let get=|path:&str|{let mut s=TcpStream::connect(("127.0.0.1",server.port)).unwrap();s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();let mut out=String::new();s.read_to_string(&mut out).unwrap();out};
  let out=get(&format!("/p/app.html?pi={id}"));
  assert!(out.starts_with("HTTP/1.1 200"),"{out}");assert!(out.contains("'registerPlugin'"));assert!(!out.contains("registerPropertyInspector"));
  assert!(out.contains("connect-src")==false && out.contains("https: ws: wss:"));
  assert!(get("/p/app.html").starts_with("HTTP/1.1 404"));assert!(get(&format!("/p/app.html?pi={}","e".repeat(32))).starts_with("HTTP/1.1 404"));
 }
}