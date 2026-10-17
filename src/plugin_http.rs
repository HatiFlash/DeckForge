//! Serveur HTTP local de ressources HTML des plugins, sans listing ni traversée.
use std::{fs,io::{Read,Write},net::{TcpListener,TcpStream},path::{Path,PathBuf},sync::{Arc,atomic::{AtomicBool,Ordering}},thread,time::Duration};

pub struct HtmlServer { pub port:u16, stop:Arc<AtomicBool>, worker:Option<thread::JoinHandle<()>> }

fn safe_file(root:&Path,request:&str,uuid:&str)->Option<PathBuf>{
    let url=request.split('?').next()?;
    let rel=url.strip_prefix('/')?.strip_prefix(uuid)?.strip_prefix('/')?;
    if rel.is_empty() || rel.contains('%') || rel.contains(std::path::MAIN_SEPARATOR) {return None;}
    let path=Path::new(rel);
    if !path.components().all(|p| matches!(p,std::path::Component::Normal(_))) {return None;}
    let real=root.join(path).canonicalize().ok()?;
    if real.is_file() && real.starts_with(root.canonicalize().ok()?) {Some(real)} else {None}
}
fn mime(path:&Path)->&'static str {match path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase().as_str(){
    "html"|"htm"=>"text/html; charset=utf-8", "js"|"mjs"=>"text/javascript; charset=utf-8", "css"=>"text/css; charset=utf-8", "svg"=>"image/svg+xml", "png"=>"image/png", "jpg"|"jpeg"=>"image/jpeg", "json"=>"application/json", "woff"=>"font/woff", "woff2"=>"font/woff2", _=>"application/octet-stream"}}
fn response(root:&Path,uuid:&str,port:u16,info:&str,request:&str)->(&'static str,&'static str,Vec<u8>){
    let Some(path)=safe_file(root,request,uuid) else {return ("404 Not Found","text/plain",b"Not found".to_vec())};
    let content_type=mime(&path);
    let Ok(bytes)=fs::read(path) else {return ("404 Not Found","text/plain",b"Not found".to_vec())};
    if !content_type.starts_with("text/html") {return ("200 OK",content_type,bytes)}
    let Ok(mut html)=String::from_utf8(bytes) else {return ("415 Unsupported Media Type","text/plain",b"Invalid HTML".to_vec())};
    let info=serde_json::to_string(info).unwrap_or_default();
    let uuid=serde_json::to_string(uuid).unwrap_or_default();
    let script=format!(r#"<script>window.addEventListener('load',function(){{var f=window.connectElgatoStreamDeckSocket||window.connectSocket;if(typeof f==='function')f({port},{uuid},'registerPlugin',{info});}});</script>"#);
    if let Some(i)=html.to_ascii_lowercase().rfind("</body>"){html.insert_str(i,&script);}else{html.push_str(&script);}
    ("200 OK",content_type,html.into_bytes())
}
fn serve(mut stream:TcpStream,root:&Path,uuid:&str,port:u16,info:&str){
    let _=stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _=stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut buf=[0u8;8192];let Ok(n)=stream.read(&mut buf) else{return};
    let request=String::from_utf8_lossy(&buf[..n]);
    let mut fields=request.lines().next().unwrap_or("").split_whitespace();
    let method=fields.next().unwrap_or("");let target=fields.next().unwrap_or("");
    let (status,mime,body)=if method=="GET" {response(root,uuid,port,info,target)}else{("405 Method Not Allowed","text/plain",b"GET only".to_vec())};
    let header=format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len());
    let _=stream.write_all(header.as_bytes());let _=stream.write_all(&body);
}
impl HtmlServer {
    pub fn start(root:PathBuf,uuid:String,info:String,ws_port:u16)->std::io::Result<Self>{
        let listener=TcpListener::bind("127.0.0.1:0")?;listener.set_nonblocking(true)?;
        let port=listener.local_addr()?.port();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
        let worker=thread::spawn(move||while !flag.load(Ordering::Relaxed){match listener.accept(){Ok((stream,_))=>serve(stream,&root,&uuid,ws_port,&info),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>thread::sleep(Duration::from_millis(25)),Err(_)=>break}});
        Ok(Self{port,stop,worker:Some(worker)})
    }
}
impl Drop for HtmlServer {fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);if let Some(worker)=self.worker.take(){let _=worker.join();}}}

#[cfg(test)] mod tests {use super::*;
 #[test] fn html_injected_and_confined(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("index.html"),"<body>Bonjour</body>").unwrap();fs::write(tmp.path().join("style.css"),"a{}").unwrap();
 let (status,mime,body)=response(tmp.path(),"p",123,"{}","/p/index.html");assert_eq!(status,"200 OK");assert_eq!(mime,"text/html; charset=utf-8");let body=String::from_utf8(body).unwrap();assert!(body.contains("connectElgatoStreamDeckSocket"));assert!(body.find("<script>").unwrap()<body.find("</body>").unwrap());
 assert_eq!(response(tmp.path(),"p",123,"{}","/p/style.css").1,"text/css; charset=utf-8");
 for url in ["/p/../index.html","/p/%2e%2e/index.html","/p/","/another/index.html","/p/..\\index.html"] {assert_eq!(response(tmp.path(),"p",123,"{}",url).0,"404 Not Found", "{url}");}
 }
 #[test] fn http_serves_without_listing(){let tmp=tempfile::tempdir().unwrap();fs::write(tmp.path().join("index.html"),"hello").unwrap();let server=HtmlServer::start(tmp.path().to_owned(),"p".into(),"{}".into(),123).unwrap();let mut stream=TcpStream::connect(("127.0.0.1",server.port)).unwrap();stream.write_all(b"GET /p/index.html HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();let mut bytes=Vec::new();stream.read_to_end(&mut bytes).unwrap();assert!(String::from_utf8(bytes).unwrap().contains("HTTP/1.1 200 OK"));}
}
