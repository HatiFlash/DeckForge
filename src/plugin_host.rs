//! Hôte local WebSocket pour les plugins Stream Deck classiques (exécutables et Node).
use std::{collections::HashMap, net::TcpListener, process::{Child,Command,Stdio}, sync::{mpsc::{self,Receiver,Sender},Arc,atomic::{AtomicBool,Ordering}}, thread, time::Duration};
use serde_json::{json,Value};
use crate::plugins::{Plugin,Runtime};

fn node_executable()->Option<std::path::PathBuf>{
 let mut candidates=Vec::new();
 if let Some(paths)=std::env::var_os("PATH"){candidates.extend(std::env::split_paths(&paths).map(|p|p.join("node.exe")));}
 if let Some(p)=std::env::var_os("ProgramFiles(x86)"){candidates.push(std::path::PathBuf::from(p).join("fifine Control Deck/node/node20.exe"));}
 if let Some(p)=std::env::var_os("APPDATA"){if let Ok(entries)=std::fs::read_dir(std::path::PathBuf::from(p).join("Elgato/StreamDeck/NodeJS")){for entry in entries.flatten().take(32){candidates.push(entry.path().join("node.exe"));}}}
 candidates.into_iter().find(|p|p.is_file())
}
pub struct Host { pub incoming:Receiver<Value>, outgoing:Sender<Value>, pub ws_port:u16, pub http_port:Option<u16>, pub inspectors:Option<crate::plugin_http::Inspectors>, pub folder:std::path::PathBuf, pub uuid:String, child:Child, worker:Option<thread::JoinHandle<()>>, stop:Arc<AtomicBool>, _http:Option<crate::plugin_http::HtmlServer> }
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
  let needs_http=matches!(plugin.runtime,Runtime::Html(_)) || plugin.inspector.is_some() || plugin.actions.iter().any(|a|a.inspector.is_some());
  let http=if needs_http {Some(crate::plugin_http::HtmlServer::start(plugin.folder.clone(),plugin.uuid.clone(),info.to_string(),port)?)}else{None};
  if let (Runtime::Html(path),Some(server))=(&plugin.runtime,&http){let name=path.strip_prefix(&plugin.folder)?.to_string_lossy().replace(std::path::MAIN_SEPARATOR,"/");let profile=std::env::current_exe()?.parent().ok_or_else(||anyhow::anyhow!("Dossier exécutable absent"))?.join("plugin-browser").join(plugin.uuid.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_'{c}else{'_'}).collect::<String>());process.arg("--headless=new").arg("--disable-gpu").arg("--no-first-run").arg("--no-default-browser-check").arg(format!("--user-data-dir={}",profile.display())).arg(format!("http://127.0.0.1:{}/{}/{}",server.port,plugin.uuid,name));}
  if !matches!(plugin.runtime,Runtime::Html(_)){process.arg("-port").arg(port.to_string()).arg("-pluginUUID").arg(&plugin.uuid).arg("-registerEvent").arg("registerPlugin").arg("-info").arg(info.to_string());}
  let child=process.current_dir(&plugin.folder).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn()?;
  let (outgoing,commands)=mpsc::channel::<Value>();let (messages,incoming)=mpsc::channel();
  let stop=Arc::new(AtomicBool::new(false));let stopped=stop.clone();
  let registry=http.as_ref().map(|server|server.inspectors.clone());
  let worker=thread::spawn(move||socket_loop(listener,commands,messages,stopped,registry));
  Ok(Self{incoming,outgoing,ws_port:port,http_port:http.as_ref().map(|h|h.port),inspectors:http.as_ref().map(|h|h.inspectors.clone()),folder:plugin.folder.clone(),uuid:plugin.uuid.clone(),child,worker:Some(worker),stop,_http:http})
 }
 pub fn open_inspector(&self,path:&str,action_info:Value)->anyhow::Result<String> {
  let server=self.http_port.ok_or_else(||anyhow::anyhow!("Serveur HTML indisponible"))?;
  let registry=self.inspectors.as_ref().ok_or_else(||anyhow::anyhow!("Inspecteur indisponible"))?;
  let file=self.folder.join(path).canonicalize()?;
  anyhow::ensure!(file.is_file() && file.starts_with(self.folder.canonicalize()?),"Chemin d'inspecteur invalide");
  let mut random=[0u8;16];getrandom::fill(&mut random)?;
  let id=random.iter().map(|b|format!("{b:02x}")).collect::<String>();
  let data=format!("JSON.stringify({})",action_info);
  registry.lock().unwrap().insert(id.clone(),data);
  let edge=std::path::Path::new(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");
  anyhow::ensure!(edge.is_file(),"Microsoft Edge introuvable");
  let profile=std::env::current_exe()?.parent().ok_or_else(||anyhow::anyhow!("Dossier exécutable absent"))?.join("plugin-browser/pi");
  let path=path.replace(std::path::MAIN_SEPARATOR,"/");
  let result=Command::new(edge).arg(format!("--app=http://127.0.0.1:{server}/{}/{path}?pi={id}",self.uuid))
    .arg("--window-size=440,680").arg("--no-first-run").arg(format!("--user-data-dir={}",profile.display()))
    .creation_flags(0x08000000).spawn();
  if result.is_err(){registry.lock().unwrap().remove(&id);}
  result?;
  Ok(id)
 }
 pub fn exited(&mut self)->bool{self.child.try_wait().ok().flatten().is_some()}
 pub fn send(&self,message:Value)->Result<(),mpsc::SendError<Value>>{self.outgoing.send(message)}
}
fn socket_loop(listener:TcpListener,commands:Receiver<Value>,messages:Sender<Value>,stop:Arc<AtomicBool>,registry:Option<crate::plugin_http::Inspectors>){
    let _=listener.set_nonblocking(true);
    let mut plugin:Option<tungstenite::WebSocket<std::net::TcpStream>>=None;
    let mut pending=Vec::new();
    let mut inspectors:HashMap<String,tungstenite::WebSocket<std::net::TcpStream>>=HashMap::new();
    while !stop.load(Ordering::Relaxed){
        match listener.accept(){Ok((stream,_))=>{let _=stream.set_read_timeout(Some(Duration::from_secs(2)));let _=stream.set_write_timeout(Some(Duration::from_secs(1)));if let Ok(ws)=tungstenite::accept(stream){let _=ws.get_ref().set_read_timeout(Some(Duration::from_millis(50)));pending.push(ws);}},Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{},Err(_)=>break}
        for i in (0..pending.len()).rev(){
            if let Ok(tungstenite::Message::Text(text))=pending[i].read(){
                if let Ok(value)=serde_json::from_str::<Value>(&text){
                    match value["event"].as_str(){
                        Some("registerPlugin") if plugin.is_none()=>{plugin=Some(pending.swap_remove(i));let _=messages.send(value);},
                        Some("registerPropertyInspector")=>{if let Some(id)=value["uuid"].as_str().filter(|id|registry.as_ref().is_some_and(|r|r.lock().unwrap().contains_key(*id))){let id=id.to_owned();inspectors.insert(id.clone(),pending.swap_remove(i));let _=messages.send(json!({"event":"propertyInspectorDidAppear","__pi":id}));}},
                        _=>{}
                    }
                }
            }
        }
        while let Ok(mut cmd)=commands.try_recv(){
            let target=cmd.get("__pi").and_then(Value::as_str).map(str::to_owned);
            if let Some(obj)=cmd.as_object_mut(){obj.remove("__pi");}
            if let Some(id)=target{if let Some(ws)=inspectors.get_mut(&id){let _=ws.send(tungstenite::Message::Text(cmd.to_string().into()));}}
            else if let Some(ws)=plugin.as_mut(){let _=ws.send(tungstenite::Message::Text(cmd.to_string().into()));}
        }
        if let Some(ws)=plugin.as_mut(){match ws.read(){Ok(tungstenite::Message::Text(text))=>{if let Ok(value)=serde_json::from_str::<Value>(&text){let _=messages.send(value);}},Ok(tungstenite::Message::Close(_))=>{plugin=None;},Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},Err(_)=>{plugin=None;},_=>{}}}
        let mut gone=Vec::new();
        for (id,ws) in &mut inspectors {match ws.read(){Ok(tungstenite::Message::Text(text))=>{if let Ok(mut value)=serde_json::from_str::<Value>(&text){if let Some(obj)=value.as_object_mut(){obj.insert("__pi".into(),json!(id));let _=messages.send(value);}}},Ok(tungstenite::Message::Close(_))=>gone.push(id.clone()),Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},Err(_)=>gone.push(id.clone()),_=>{}}}
        for id in gone {inspectors.remove(&id);if let Some(r)=&registry{r.lock().unwrap().remove(&id);}let _=messages.send(json!({"event":"propertyInspectorDidDisappear","__pi":id}));}
        thread::sleep(Duration::from_millis(10));
    }
}
impl Drop for Host {fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);
 #[cfg(windows)] {let _=Command::new("taskkill").args(["/F","/T","/PID",&self.child.id().to_string()]).creation_flags(0x08000000).output();}
 let _=self.child.kill();let _=self.child.wait(); // Le thread se termine au timeout de lecture et fermeture du canal.
 let _=self.worker.take();
}}
#[cfg(windows)] use std::os::windows::process::CommandExt;
#[cfg(test)] mod tests {use super::*;
 #[test] fn two_websocket_clients_exchange_inspector_events(){
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let id="a".repeat(32);let registry=crate::plugin_http::Inspectors::default();registry.lock().unwrap().insert(id.clone(),"JSON.stringify({})".into());
  let (tx,commands)=mpsc::channel();let (messages,rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let worker=thread::spawn(move||socket_loop(listener,commands,messages,flag,Some(registry)));
  let (mut plugin,_)=tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
  plugin.send(tungstenite::Message::Text(json!({"event":"registerPlugin","uuid":"p"}).to_string().into())).unwrap();
  assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap()["event"],"registerPlugin");
  let (mut pi,_)=tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
  pi.send(tungstenite::Message::Text(json!({"event":"registerPropertyInspector","uuid":id}).to_string().into())).unwrap();
  assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap()["event"],"propertyInspectorDidAppear");
  pi.send(tungstenite::Message::Text(json!({"event":"sendToPlugin","payload":{"hello":1}}).to_string().into())).unwrap();
  let mut from_pi=rx.recv_timeout(Duration::from_secs(3)).unwrap();assert_eq!(from_pi["payload"]["hello"],1);assert_eq!(from_pi["__pi"],id);
  from_pi.as_object_mut().unwrap().remove("__pi");tx.send(from_pi).unwrap();
  assert_eq!(serde_json::from_str::<Value>(&plugin.read().unwrap().to_string()).unwrap()["event"],"sendToPlugin");
  plugin.send(tungstenite::Message::Text(json!({"event":"sendToPropertyInspector","context":"ctx","payload":{"ok":true}}).to_string().into())).unwrap();
  let mut from_plugin=rx.recv_timeout(Duration::from_secs(3)).unwrap();assert_eq!(from_plugin["event"],"sendToPropertyInspector");from_plugin["__pi"]=json!(id);tx.send(from_plugin).unwrap();
  assert_eq!(serde_json::from_str::<Value>(&pi.read().unwrap().to_string()).unwrap()["payload"]["ok"],true);
  stop.store(true,Ordering::Relaxed);worker.join().unwrap();
 }
 #[test] fn node_fixture_exchanges_key_messages(){
  let fixture=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sdplugin");
  let p=crate::plugins::parse(&fixture,&std::fs::read(fixture.join("manifest.json")).unwrap()).unwrap();
  let node=node_executable().expect("Node.js required for the plugin fixture test");
  let support=Command::new(&node).args(["-p","typeof WebSocket"]).output().expect("Node.js fixture preflight failed");
  assert!(support.status.success() && support.stdout==b"function
", "Node.js fixture needs global WebSocket: {} {}", String::from_utf8_lossy(&support.stdout), String::from_utf8_lossy(&support.stderr));
  let host=Host::start(&p).expect("Failed to start plugin fixture");
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
