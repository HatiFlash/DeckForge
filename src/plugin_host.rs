//! Hôte local WebSocket pour les plugins Stream Deck classiques (exécutables et Node).
use std::{net::TcpListener, process::{Child,Command,Stdio}, sync::{mpsc::{self,Receiver,Sender},Arc,atomic::{AtomicBool,Ordering}}, thread, time::Duration};
use serde_json::{json,Value};
use crate::plugins::{Plugin,Runtime};

fn node_executable()->Option<std::path::PathBuf>{
 let mut candidates=Vec::new();
 if let Some(paths)=std::env::var_os("PATH"){candidates.extend(std::env::split_paths(&paths).map(|p|p.join("node.exe")));}
 if let Some(p)=std::env::var_os("ProgramFiles(x86)"){candidates.push(std::path::PathBuf::from(p).join("fifine Control Deck/node/node20.exe"));}
 if let Some(p)=std::env::var_os("APPDATA"){if let Ok(entries)=std::fs::read_dir(std::path::PathBuf::from(p).join("Elgato/StreamDeck/NodeJS")){for entry in entries.flatten().take(32){candidates.push(entry.path().join("node.exe"));}}}
 candidates.into_iter().find(|p|p.is_file())
}
pub struct Host { pub incoming:Receiver<Value>, outgoing:Sender<Value>, child:Child, worker:Option<thread::JoinHandle<()>>, stop:Arc<AtomicBool>, _http:Option<crate::plugin_http::HtmlServer> }
impl Host {
 pub fn start(plugin:&Plugin)->anyhow::Result<Self>{
  let listener=TcpListener::bind("127.0.0.1:0")?;
  let port=listener.local_addr()?.port();
  let mut process=match &plugin.runtime {
   Runtime::Exe(p)=>Command::new(p),
   Runtime::Node(p)=>{let node=node_executable().ok_or_else(||anyhow::anyhow!("Node.js introuvable (système, FIFINE ou Elgato)"))?;let mut c=Command::new(node);c.arg(p);c},
   Runtime::Html(_)=>{let edge=std::path::Path::new(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");anyhow::ensure!(edge.is_file(),"Microsoft Edge introuvable");Command::new(edge)},
   _=>anyhow::bail!("Plugin protégé ou type non pris en charge"),
  };
  let info=json!({"application":{"font":"","language":"fr","platform":"windows","platformVersion":"10","version":"6.9.0"},"plugin":{"uuid":plugin.uuid,"version":plugin.version},"devicePixelRatio":1,"colors":{},"devices":[{"id":"deckforge-d6","name":"FIFINE D6","size":{"columns":5,"rows":3},"type":0}]});
  let http=if matches!(plugin.runtime,Runtime::Html(_)){Some(crate::plugin_http::HtmlServer::start(plugin.folder.clone(),plugin.uuid.clone(),info.to_string(),port)?)}else{None};
  if let (Runtime::Html(path),Some(server))=(&plugin.runtime,&http){let name=path.strip_prefix(&plugin.folder)?.to_string_lossy().replace(std::path::MAIN_SEPARATOR,"/");let profile=std::env::current_exe()?.parent().ok_or_else(||anyhow::anyhow!("Dossier exécutable absent"))?.join("plugin-browser").join(plugin.uuid.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_'{c}else{'_'}).collect::<String>());process.arg("--headless=new").arg("--disable-gpu").arg("--no-first-run").arg("--no-default-browser-check").arg(format!("--user-data-dir={}",profile.display())).arg(format!("http://127.0.0.1:{}/{}/{}",server.port,plugin.uuid,name));}
  if !matches!(plugin.runtime,Runtime::Html(_)){process.arg("-port").arg(port.to_string()).arg("-pluginUUID").arg(&plugin.uuid).arg("-registerEvent").arg("registerPlugin").arg("-info").arg(info.to_string());}
  let child=process.current_dir(&plugin.folder).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn()?;
  let (outgoing,commands)=mpsc::channel::<Value>();let (messages,incoming)=mpsc::channel();
  let stop=Arc::new(AtomicBool::new(false));let stopped=stop.clone();
  let worker=thread::spawn(move||{
   listener.set_nonblocking(true).ok();
   let mut socket=loop {if stopped.load(Ordering::Relaxed){return;} match listener.accept(){Ok((stream,_))=>{let _=stream.set_read_timeout(Some(Duration::from_millis(50)));let _=stream.set_write_timeout(Some(Duration::from_secs(1)));match tungstenite::accept(stream){Ok(ws)=>break ws,Err(_)=>return}},Err(_)=>{thread::sleep(Duration::from_millis(25));}}};
   loop {if stopped.load(Ordering::Relaxed){return;}
    while let Ok(cmd)=commands.try_recv(){if socket.send(tungstenite::Message::Text(cmd.to_string().into())).is_err(){return}}
    match socket.read(){Ok(tungstenite::Message::Text(text))=>{if let Ok(v)=serde_json::from_str::<Value>(&text){if messages.send(v).is_err(){return}}},Ok(tungstenite::Message::Close(_))=>return,Err(tungstenite::Error::Io(ref e)) if e.kind()==std::io::ErrorKind::WouldBlock || e.kind()==std::io::ErrorKind::TimedOut=>{},Err(_)=>return,_=>{}}
   }
  });
  Ok(Self{incoming,outgoing,child,worker:Some(worker),stop,_http:http})
 }
 pub fn exited(&mut self)->bool{self.child.try_wait().ok().flatten().is_some()}
 pub fn send(&self,message:Value)->Result<(),mpsc::SendError<Value>>{self.outgoing.send(message)}
}
impl Drop for Host {fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);
 #[cfg(windows)] {let _=Command::new("taskkill").args(["/F","/T","/PID",&self.child.id().to_string()]).creation_flags(0x08000000).output();}
 let _=self.child.kill();let _=self.child.wait(); // Le thread se termine au timeout de lecture et fermeture du canal.
 let _=self.worker.take();
}}
#[cfg(windows)] use std::os::windows::process::CommandExt;
#[cfg(test)] mod tests {use super::*;
 #[test] fn node_fixture_exchanges_key_messages(){
  let fixture=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sdplugin");
  let p=crate::plugins::parse(&fixture,&std::fs::read(fixture.join("manifest.json")).unwrap()).unwrap();
  let Ok(host)=Host::start(&p) else {eprintln!("Node.js absent : fixture ignorée");return};
  let register=host.incoming.recv_timeout(Duration::from_secs(15)).unwrap();
  assert_eq!(register["event"],"registerPlugin");
  let payload=json!({"settings":{},"coordinates":{"column":0,"row":0},"state":0,"isInMultiAction":false,"controller":"Keypad"});
  let event=|name|json!({"event":name,"action":"test.deckforge.fixture.action","context":"ctx-1","device":"deckforge-d6","payload":payload});
  host.send(event("willAppear")).unwrap();host.send(event("keyDown")).unwrap();
  let mut names=std::collections::HashSet::new();
  for _ in 0..4 {let message=host.incoming.recv_timeout(Duration::from_secs(15)).unwrap();assert_eq!(message["context"],"ctx-1");if message["event"]=="setTitle"{assert_eq!(message["payload"]["title"],"OK 1");}if message["event"]=="setSettings"{assert_eq!(message["payload"]["settings"]["presses"],1);}names.insert(message["event"].as_str().unwrap().to_string());}
  assert_eq!(names,std::collections::HashSet::from(["setTitle".into(),"setImage".into(),"setSettings".into(),"showOk".into()]));
 } #[test] fn rejected_protected_plugin_does_not_spawn(){let d=tempfile::tempdir().unwrap();let p=crate::plugins::parse(d.path(),b"ELGATO\x01payload").unwrap();assert!(Host::start(&p).is_err());}
}
