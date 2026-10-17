//! Serveur HTTP local de ressources HTML des plugins, sans listing ni traversée.
use std::{fs,io::{Read,Write},net::{TcpListener,TcpStream},path::{Path,PathBuf},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},thread,time::Duration};

pub type Inspectors=Arc<Mutex<std::collections::HashMap<String,String>>>;
pub struct HtmlServer { pub port:u16, pub inspectors:Inspectors, stop:Arc<AtomicBool>, worker:Option<thread::JoinHandle<()>> }

/// Décode %XX (UTF-8 strict) : les chemins de manifeste contiennent souvent des espaces (« property inspector/… »).
fn percent_decode(s:&str)->Option<String>{
    let b=s.as_bytes();let mut out=Vec::with_capacity(b.len());let mut i=0;
    while i<b.len(){ if b[i]==b'%' { let h=std::str::from_utf8(b.get(i+1..i+3)?).ok()?; out.push(u8::from_str_radix(h,16).ok()?); i+=3; } else { out.push(b[i]); i+=1; } }
    String::from_utf8(out).ok()
}
/// Habillage DeckForge des pages de réglages (Property Inspector) : mêmes couleurs que l'application,
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
    if rel.is_empty() || rel.contains('\\') || rel.contains(':') || rel.contains('\0') {return None;}
    let path=Path::new(&rel);
    if !path.components().all(|p| matches!(p,std::path::Component::Normal(_))) {return None;}
    let real=root.join(path).canonicalize().ok()?;
    if real.is_file() && real.starts_with(root.canonicalize().ok()?) {Some(real)} else {None}
}
fn mime(path:&Path)->&'static str {match path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase().as_str(){
    "html"|"htm"=>"text/html; charset=utf-8", "js"|"mjs"=>"text/javascript; charset=utf-8", "css"=>"text/css; charset=utf-8", "svg"=>"image/svg+xml", "png"=>"image/png", "jpg"|"jpeg"=>"image/jpeg", "json"=>"application/json", "woff"=>"font/woff", "woff2"=>"font/woff2", _=>"application/octet-stream"}}
fn response(root:&Path,uuid:&str,port:u16,info:&str,inspectors:&Inspectors,request:&str)->(&'static str,&'static str,Vec<u8>){
    let Some(path)=safe_file(root,request,uuid) else {return ("404 Not Found","text/plain",b"Not found".to_vec())};
    let content_type=mime(&path);
    let Ok(bytes)=fs::read(path) else {return ("404 Not Found","text/plain",b"Not found".to_vec())};
    if !content_type.starts_with("text/html") {return ("200 OK",content_type,bytes)}
    let Ok(mut html)=String::from_utf8(bytes) else {return ("415 Unsupported Media Type","text/plain",b"Invalid HTML".to_vec())};
    let pi=request.split_once('?').and_then(|(_,q)|q.strip_prefix("pi=")).filter(|id|id.len()==32 && id.bytes().all(|b|b.is_ascii_hexdigit()))
        .and_then(|id|inspectors.lock().ok()?.get(id).cloned().map(|data|(id.to_owned(),data)));
    let script=if let Some((pi_id,data))=pi {
        let pi_id=serde_json::to_string(&pi_id).unwrap_or_default();
        format!(r#"<script>window.addEventListener('load',function(){{var f=window.connectElgatoStreamDeckSocket||window.connectElgatoSdkSocket||window.connectSocket;if(typeof f==='function')f({port},{pi_id},'registerPropertyInspector',{info},{data});}});</script>{PI_THEME}"#,
            info=serde_json::to_string(info).unwrap_or_default())
    }else{
        format!(r#"<script>window.addEventListener('load',function(){{var f=window.connectElgatoStreamDeckSocket||window.connectSocket;if(typeof f==='function')f({port},{uuid},'registerPlugin',{info});}});</script>"#,
            uuid=serde_json::to_string(uuid).unwrap_or_default(),info=serde_json::to_string(info).unwrap_or_default())
    };
    if let Some(i)=html.to_ascii_lowercase().rfind("</body>"){html.insert_str(i,&script);}else{html.push_str(&script);}
    ("200 OK",content_type,html.into_bytes())
}
fn serve(mut stream:TcpStream,root:&Path,uuid:&str,port:u16,info:&str,inspectors:&Inspectors){
      let _=stream.set_nonblocking(false); // Hérité de l'écouteur sous Windows : sinon une requête lente est perdue.
    let _=stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _=stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut buf=[0u8;8192];let Ok(n)=stream.read(&mut buf) else{return};
    let request=String::from_utf8_lossy(&buf[..n]);
    let mut fields=request.lines().next().unwrap_or("").split_whitespace();
    let method=fields.next().unwrap_or("");let target=fields.next().unwrap_or("");
    let (status,mime,body)=if method=="GET" {response(root,uuid,port,info,inspectors,target)}else{("405 Method Not Allowed","text/plain",b"GET only".to_vec())};
    let header=format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len());
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
 #[test] fn property_inspector_injection(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("pi.html"),"<body>PI</body>").unwrap();let inspectors=Inspectors::default();let id="a".repeat(32);inspectors.lock().unwrap().insert(id.clone(),"JSON.stringify({action:'test',context:'ctx'})".into());let (_,_,body)=response(tmp.path(),"plugin",345,"{}",&inspectors,&format!("/plugin/pi.html?pi={id}"));let html=String::from_utf8(body).unwrap();assert!(html.contains("connectElgatoSdkSocket"));assert!(html.contains("registerPropertyInspector"));assert!(html.contains("action:'test'"));assert!(!html.contains("registerPlugin"));let (_,_,body)=response(tmp.path(),"plugin",345,"{}",&inspectors,"/plugin/pi.html?pi=unknown");assert!(String::from_utf8(body).unwrap().contains("registerPlugin"));}
 #[test] fn html_injected_and_confined(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("index.html"),"<body>Bonjour</body>").unwrap();fs::write(tmp.path().join("style.css"),"a{}").unwrap();
 let inspectors=Inspectors::default();let (status,mime,body)=response(tmp.path(),"p",123,"{}",&inspectors,"/p/index.html");assert_eq!(status,"200 OK");assert_eq!(mime,"text/html; charset=utf-8");let body=String::from_utf8(body).unwrap();assert!(body.contains("connectElgatoStreamDeckSocket"));assert!(body.find("<script>").unwrap()<body.find("</body>").unwrap());
 assert_eq!(response(tmp.path(),"p",123,"{}",&inspectors,"/p/style.css").1,"text/css; charset=utf-8");
 for url in ["/p/../index.html","/p/%2e%2e/index.html","/p/","/another/index.html","/p/..\\index.html"] {assert_eq!(response(tmp.path(),"p",123,"{}",&inspectors,url).0,"404 Not Found", "{url}");}
 }
 #[test] fn spaces_in_path_are_decoded(){let tmp=tempfile::tempdir().unwrap();fs::create_dir(tmp.path().join("property inspector")).unwrap();fs::write(tmp.path().join("property inspector").join("pi.html"),"<body>x</body>").unwrap();let inspectors=Inspectors::default();assert_eq!(response(tmp.path(),"p",1,"{}",&inspectors,"/p/property%20inspector/pi.html").0,"200 OK");for bad in ["/p/property%20inspector/..%2fpi.html","/p/%2e%2e%2fx","/p/a%5cb","/p/%zz"]{assert_eq!(response(tmp.path(),"p",1,"{}",&inspectors,bad).0,"404 Not Found","{bad}");}}
#[test] fn slow_client_request_is_still_served(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("index.html"),"hello").unwrap();let server=HtmlServer::start(tmp.path().to_owned(),"p".into(),"{}".into(),1).unwrap();let mut s=TcpStream::connect(("127.0.0.1",server.port)).unwrap();thread::sleep(Duration::from_millis(300));s.write_all(b"GET /p/index.html HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();let mut out=String::new();s.read_to_string(&mut out).unwrap();assert!(out.starts_with("HTTP/1.1 200"),"{out}");}
#[test] fn http_serves_without_listing(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("index.html"),"hello").unwrap();let server=HtmlServer::start(tmp.path().to_owned(),"p".into(),"{}".into(),123).unwrap();let mut stream=TcpStream::connect(("127.0.0.1",server.port)).unwrap();stream.write_all(b"GET /p/index.html HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();let mut bytes=Vec::new();stream.read_to_end(&mut bytes).unwrap();assert!(String::from_utf8(bytes).unwrap().contains("HTTP/1.1 200 OK"));}
}
