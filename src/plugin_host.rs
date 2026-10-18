//! Hôte local WebSocket pour les plugins Stream Deck classiques (exécutables et Node).
use std::{collections::HashMap, net::TcpListener, process::{Child,Command,Stdio}, sync::{mpsc::{self,Receiver,Sender},Arc,atomic::{AtomicBool,Ordering}}, thread, time::Duration};
use std::io::Write;
use serde_json::{json,Value};
use crate::plugins::{Plugin,Runtime};

fn node_executable()->Option<std::path::PathBuf>{
 let mut candidates=Vec::new();
 if let Some(paths)=std::env::var_os("PATH"){candidates.extend(std::env::split_paths(&paths).map(|p|p.join("node.exe")));}
 if let Some(p)=std::env::var_os("ProgramFiles"){candidates.push(std::path::PathBuf::from(p).join("nodejs/node.exe"));}
 if let Some(p)=std::env::var_os("ProgramFiles(x86)"){candidates.push(std::path::PathBuf::from(p).join("fifine Control Deck/node/node20.exe"));}
 if let Some(p)=std::env::var_os("APPDATA"){if let Ok(entries)=std::fs::read_dir(std::path::PathBuf::from(p).join("Elgato/StreamDeck/NodeJS")){for entry in entries.flatten().take(32){candidates.push(entry.path().join("node.exe"));}}}
 candidates.into_iter().find(|p|p.is_file())
}
fn launch_info(plugin:&Plugin)->Value {
 let platform_version=std::env::var("DECKFORGE_PLATFORM_VERSION").unwrap_or_else(|_|"10.0".into());
 json!({"application":{"font":"","language":"fr","platform":"windows","platformVersion":platform_version,"version":env!("CARGO_PKG_VERSION")},"plugin":{"uuid":plugin.uuid,"version":plugin.version},"devicePixelRatio":1,"colors":{},"devices":[{"id":"deckforge-d6","name":"FIFINE D6","size":{"columns":5,"rows":3},"type":0}]})
}
// Trace opt-in : uniquement le sens, le nom de commande et les identifiants non secrets. Jamais payload/settings.
struct Trace { file:std::fs::File, entries:usize }
impl Trace {fn open(uuid:&str)->Option<Self>{
 if std::env::var("DECKFORGE_PLUGIN_TRACE_UUID").ok().as_deref()!=Some(uuid){return None;}
 let path=std::env::temp_dir().join("deckforge-plugin-traces");std::fs::create_dir_all(&path).ok()?;
 let name=uuid.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_'{c}else{'_'}).take(128).collect::<String>();
 let file=std::fs::OpenOptions::new().create(true).append(true).open(path.join(format!("{name}-{}.jsonl",std::process::id()))).ok()?;
 Some(Self{file,entries:0})
}}
fn supported_command(message:&Value)->bool {
 matches!(message.get("event").and_then(Value::as_str),Some("registerPlugin"|"registerPropertyInspector"|"logMessage"|"setTitle"|"setImage"|"setState"|"showOk"|"showAlert"|"setSettings"|"getSettings"|"setGlobalSettings"|"getGlobalSettings"|"openUrl"|"sendToPlugin"|"sendToPropertyInspector"))
}
fn trace_message(trace:&mut Option<Trace>,direction:&str,message:&Value,unsupported:bool){
 let Some(trace)=trace.as_mut() else{return};if trace.entries>=1024{return;}
 let label=message.get("event").and_then(Value::as_str).unwrap_or("<absent>");
 let label=label.chars().take(96).collect::<String>();
 let record=json!({"direction":direction,"event":label,"unsupported":unsupported});
 let _=writeln!(trace.file,"{record}");trace.entries+=1;
}
#[cfg(windows)] struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)] impl Job {
 fn new()->std::io::Result<Self>{use windows_sys::Win32::System::JobObjects::*;
  unsafe {let handle=CreateJobObjectW(std::ptr::null(),std::ptr::null());
   if handle.is_null(){return Err(std::io::Error::last_os_error());}
   let job=Self(handle);let mut info:JOBOBJECT_EXTENDED_LIMIT_INFORMATION=std::mem::zeroed();
   info.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
   if SetInformationJobObject(handle,JobObjectExtendedLimitInformation,&info as *const _ as *const _,std::mem::size_of_val(&info) as u32)==0{return Err(std::io::Error::last_os_error());}
   Ok(job)
  }
 }
 fn assign(&self,child:&Child)->std::io::Result<()>{use std::os::windows::io::AsRawHandle;
  if unsafe {windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(self.0,child.as_raw_handle())}==0 {Err(std::io::Error::last_os_error())}else{Ok(())}
 }
 fn close(&mut self){if !self.0.is_null(){unsafe {windows_sys::Win32::Foundation::CloseHandle(self.0)};self.0=std::ptr::null_mut();}}
}
#[cfg(windows)] impl Drop for Job {fn drop(&mut self){self.close();}}
pub struct Host { #[cfg(windows)] job:Job, pub incoming:Receiver<Value>, outgoing:Sender<Value>, pub ws_port:u16, pub http_port:Option<u16>, pub inspectors:Option<crate::plugin_http::Inspectors>, pub folder:std::path::PathBuf, pub uuid:String, child:Child, worker:Option<thread::JoinHandle<()>>, stop:Arc<AtomicBool>, _http:Option<crate::plugin_http::HtmlServer> }
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
  let info=launch_info(plugin);
  let needs_http=matches!(plugin.runtime,Runtime::Html(_)) || plugin.inspector.is_some() || plugin.actions.iter().any(|a|a.inspector.is_some());
  let http=if needs_http {Some(crate::plugin_http::HtmlServer::start(plugin.folder.clone(),plugin.uuid.clone(),info.to_string(),port)?)}else{None};
  if let (Runtime::Html(path),Some(server))=(&plugin.runtime,&http){let name=path.strip_prefix(&plugin.folder)?.to_string_lossy().replace(std::path::MAIN_SEPARATOR,"/");let profile=std::env::current_exe()?.parent().ok_or_else(||anyhow::anyhow!("Dossier exécutable absent"))?.join("plugin-browser").join(plugin.uuid.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_'{c}else{'_'}).collect::<String>());process.arg("--headless=new").arg("--disable-gpu").arg("--no-first-run").arg("--no-default-browser-check").arg(format!("--user-data-dir={}",profile.display())).arg(format!("http://127.0.0.1:{}/{}/{}",server.port,plugin.uuid,name));}
  if !matches!(plugin.runtime,Runtime::Html(_)){process.arg("-port").arg(port.to_string()).arg("-pluginUUID").arg(&plugin.uuid).arg("-registerEvent").arg("registerPlugin").arg("-info").arg(info.to_string());}
  #[cfg(windows)] let job=Job::new()?;
  let mut child=process.current_dir(&plugin.folder).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn()?;
  #[cfg(windows)] if let Err(e)=job.assign(&child){let _=child.kill();let _=child.wait();return Err(e.into());}
  let (outgoing,commands)=mpsc::channel::<Value>();let (messages,incoming)=mpsc::channel();
  let stop=Arc::new(AtomicBool::new(false));let stopped=stop.clone();
  let registry=http.as_ref().map(|server|server.inspectors.clone());
  let trace=Trace::open(&plugin.uuid);
  let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,stopped,registry,trace));
  Ok(Self{#[cfg(windows)] job,incoming,outgoing,ws_port:port,http_port:http.as_ref().map(|h|h.port),inspectors:http.as_ref().map(|h|h.inspectors.clone()),folder:plugin.folder.clone(),uuid:plugin.uuid.clone(),child,worker:Some(worker),stop,_http:http})
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
  let mut inspector=result?;
  #[cfg(windows)] if let Err(e)=self.job.assign(&inspector){let _=inspector.kill();let _=inspector.wait();return Err(e.into());}
  Ok(id)
 }
 pub fn exited(&mut self)->bool{self.child.try_wait().ok().flatten().is_some()}
 pub fn send(&self,message:Value)->Result<(),mpsc::SendError<Value>>{self.outgoing.send(message)}
}
fn socket_loop(listener:TcpListener,commands:Receiver<Value>,messages:Sender<Value>,stop:Arc<AtomicBool>,registry:Option<crate::plugin_http::Inspectors>){socket_loop_traced(listener,commands,messages,stop,registry,None)}
fn socket_loop_traced(listener:TcpListener,commands:Receiver<Value>,messages:Sender<Value>,stop:Arc<AtomicBool>,registry:Option<crate::plugin_http::Inspectors>,mut trace:Option<Trace>){
    let _=listener.set_nonblocking(true);
    let mut plugin:Option<tungstenite::WebSocket<std::net::TcpStream>>=None;
    let mut pending=Vec::new();
    let mut queued=std::collections::VecDeque::new();
    let mut inspectors:HashMap<String,tungstenite::WebSocket<std::net::TcpStream>>=HashMap::new();
    while !stop.load(Ordering::Relaxed){
        match listener.accept(){Ok((stream,_))=>{let _=stream.set_nonblocking(false); // Windows : le socket accepté hérite du mode non bloquant de l'écouteur.
            let _=stream.set_read_timeout(Some(Duration::from_secs(2)));let _=stream.set_write_timeout(Some(Duration::from_secs(1)));if let Ok(ws)=tungstenite::accept(stream){let _=ws.get_ref().set_read_timeout(Some(Duration::from_millis(50)));pending.push(ws);}},Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{},Err(_)=>break}
        for i in (0..pending.len()).rev(){
            if let Ok(tungstenite::Message::Text(text))=pending[i].read(){
                if let Ok(value)=serde_json::from_str::<Value>(&text){
                    match value["event"].as_str(){
                        Some("registerPlugin") if plugin.is_none()=>{trace_message(&mut trace,"plugin->host",&value,false);plugin=Some(pending.swap_remove(i));let _=messages.send(value);},
                        Some("registerPropertyInspector")=>{if let Some(id)=value["uuid"].as_str().filter(|id|registry.as_ref().is_some_and(|r|r.lock().unwrap().contains_key(*id))){let id=id.to_owned();inspectors.insert(id.clone(),pending.swap_remove(i));let _=messages.send(json!({"event":"propertyInspectorDidAppear","__pi":id}));}},
                        _=>{}
                    }
                }
            }
        }
        while let Ok(cmd)=commands.try_recv(){ queued.push_back(cmd); }
        while plugin.is_some() && !queued.is_empty() {
            let mut cmd=queued.pop_front().unwrap();
            let target=cmd.get("__pi").and_then(Value::as_str).map(str::to_owned);
            if let Some(obj)=cmd.as_object_mut(){obj.remove("__pi");}
            trace_message(&mut trace,"host->plugin",&cmd,false);
            if let Some(id)=target{if let Some(ws)=inspectors.get_mut(&id){let _=ws.send(tungstenite::Message::Text(cmd.to_string().into()));}}
            else if let Some(ws)=plugin.as_mut(){let _=ws.send(tungstenite::Message::Text(cmd.to_string().into()));}
        }
        if let Some(ws)=plugin.as_mut(){match ws.read(){Ok(tungstenite::Message::Text(text))=>{match serde_json::from_str::<Value>(&text){Ok(value)=>{trace_message(&mut trace,"plugin->host",&value,!supported_command(&value));let _=messages.send(value);},Err(e)=>{let _=messages.send(json!({"event":"__hostError","reason":format!("Message JSON malformé : {e}")}));}}},Ok(tungstenite::Message::Close(_))=>{plugin=None;},Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},Err(_)=>{plugin=None;},_=>{}}}
        let mut gone=Vec::new();
        for (id,ws) in &mut inspectors {match ws.read(){Ok(tungstenite::Message::Text(text))=>{if let Ok(mut value)=serde_json::from_str::<Value>(&text){if let Some(obj)=value.as_object_mut(){obj.insert("__pi".into(),json!(id));let _=messages.send(value);}}},Ok(tungstenite::Message::Close(_))=>gone.push(id.clone()),Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},Err(_)=>gone.push(id.clone()),_=>{}}}
        for id in gone {inspectors.remove(&id);if let Some(r)=&registry{r.lock().unwrap().remove(&id);}let _=messages.send(json!({"event":"propertyInspectorDidDisappear","__pi":id}));}
        thread::sleep(Duration::from_millis(10));
    }
}
impl Drop for Host {fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);
 #[cfg(windows)] self.job.close();
 let _=self.child.kill();let _=self.child.wait(); // Le thread se termine au timeout de lecture et fermeture du canal.
 let _=self.worker.take();
}}
#[cfg(windows)] use std::os::windows::process::CommandExt;
#[cfg(test)] mod tests {use super::*;
 #[test] fn launch_contract_and_unknown_command(){
  let d=tempfile::tempdir().unwrap();let p=crate::plugins::parse(d.path(),br#"{"UUID":"test","Version":"1.0"}"#).unwrap();
  let info=launch_info(&p);assert_eq!(info["application"]["platform"],"windows");
  assert!(info["application"]["platformVersion"].is_string());assert_eq!(info["plugin"]["uuid"],"test");
  assert_eq!(info["devices"][0]["size"]["columns"],5);assert!(!supported_command(&json!({"event":"alien"})));
  assert!(supported_command(&json!({"event":"setTitle"})));
 }
 #[cfg(windows)] #[test] fn job_closing_kills_descendant(){
  use windows_sys::Win32::System::Threading::{OpenProcess,PROCESS_SYNCHRONIZE,WaitForSingleObject};
  use windows_sys::Win32::Foundation::{CloseHandle,WAIT_OBJECT_0};
  let dir=tempfile::tempdir().unwrap();let file=dir.path().join("pid.txt");let job=Job::new().unwrap();
  let script=format!("$p=Start-Process powershell -ArgumentList '-NoProfile','-Command','Start-Sleep -Seconds 30' -PassThru -WindowStyle Hidden; $p.Id | Set-Content -Path '{}'",file.display());
  let mut parent=Command::new("powershell").args(["-NoProfile","-Command",&script]).creation_flags(0x08000000).spawn().unwrap();
  job.assign(&parent).unwrap();
  let limit=std::time::Instant::now()+Duration::from_secs(10);
  while !file.is_file() && std::time::Instant::now()<limit {thread::sleep(Duration::from_millis(30));}
  assert!(file.is_file(),"descendant was not started");
  let pid:u32=std::fs::read_to_string(file).unwrap().trim().parse().unwrap();
  let handle=unsafe{OpenProcess(PROCESS_SYNCHRONIZE,0,pid)};assert!(!handle.is_null());
  drop(job);assert_eq!(unsafe{WaitForSingleObject(handle,5000)},WAIT_OBJECT_0);
  unsafe{CloseHandle(handle)};let _=parent.wait();
 }
 #[test] fn slow_handshake_client_still_registers(){
  // Régression : la poignée de main WebSocket arrivant après accept() était abandonnée (WouldBlock).
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let (_tx,commands)=mpsc::channel::<Value>();let (messages,rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let worker=thread::spawn(move||socket_loop(listener,commands,messages,flag,None));
  let stream=std::net::TcpStream::connect(("127.0.0.1",port)).unwrap();
  thread::sleep(Duration::from_millis(300));
  let (mut ws,_)=tungstenite::client(format!("ws://127.0.0.1:{port}"),stream).unwrap();
  ws.send(tungstenite::Message::Text(json!({"event":"registerPlugin","uuid":"slow"}).to_string().into())).unwrap();
  assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap()["event"],"registerPlugin");
  stop.store(true,Ordering::Relaxed);worker.join().unwrap();
 }
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
