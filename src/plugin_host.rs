//! Hôte local WebSocket pour les plugins Stream Deck classiques (exécutables et Node).
use std::{collections::HashMap, net::TcpListener, process::{Child,Command,Stdio}, sync::{mpsc::{self,Receiver,Sender,SyncSender,TrySendError},Arc,atomic::{AtomicBool,Ordering}}, thread, time::{Duration,Instant}};
use std::io::Write;
use serde_json::{json,Value};
use crate::plugins::{Plugin,Runtime};

fn node_executable(required:u32)->Option<std::path::PathBuf>{
 let mut candidates=Vec::new();
 if let Some(paths)=std::env::var_os("PATH"){candidates.extend(std::env::split_paths(&paths).map(|p|p.join("node.exe")));}
 if let Some(p)=std::env::var_os("ProgramFiles"){candidates.push(std::path::PathBuf::from(p).join("nodejs/node.exe"));}
 if let Some(p)=std::env::var_os("ProgramFiles(x86)"){candidates.push(std::path::PathBuf::from(p).join("fifine Control Deck/node/node20.exe"));}
 if let Some(p)=std::env::var_os("APPDATA"){let root=std::path::PathBuf::from(p).join("Elgato/StreamDeck/NodeJS");candidates.push(root.join("node.exe"));if let Ok(entries)=std::fs::read_dir(root){for entry in entries.flatten().take(32){candidates.push(entry.path().join("node.exe"));}}}
 candidates.into_iter().filter(|p|p.is_file()).find(|p|node_major(p).is_some_and(|v|v>=required))
}
fn node_major(path:&std::path::Path)->Option<u32>{
 let output=Command::new(path).arg("--version").stdin(Stdio::null()).output().ok()?;
 if !output.status.success(){return None;}
 let text=std::str::from_utf8(&output.stdout).ok()?;
 text.trim().trim_start_matches('v').split('.').next()?.parse().ok()
}
fn required_node(plugin:&Plugin)->u32{
 let declared=plugin.node_version.as_deref().and_then(|v|v.trim_start_matches('v').split('.').next()?.parse::<u32>().ok()).unwrap_or(0);
 let sdk_v2=matches!(&plugin.runtime,Runtime::Node(p) if p.to_string_lossy().replace('\\',"/").ends_with("bin/plugin.js"));
 declared.max(if sdk_v2 {20}else{0})
}
/// Dossier de travail du processus : celui de l'exécutable natif (Gizmos lance `../cmd/scripts/*.ps1`
/// depuis bin/, les plugins Mirabox chargent leurs fichiers depuis plugin/), sinon la racine du plugin.
fn working_dir(plugin:&Plugin)->std::path::PathBuf{
 match &plugin.runtime {Runtime::Exe(p)=>p.parent().filter(|d|d.starts_with(&plugin.folder)).map(|d|d.to_path_buf()).unwrap_or_else(||plugin.folder.clone()),_=>plugin.folder.clone()}
}
fn launch_info(plugin:&Plugin,device:&crate::plugin_runtime::Device)->Value {
 let platform_version=std::env::var("DECKFORGE_PLATFORM_VERSION").unwrap_or_else(|_|"10.0".into());
 json!({"application":{"font":"","language":"fr","platform":"windows","platformVersion":platform_version,"version":env!("CARGO_PKG_VERSION")},"plugin":{"uuid":plugin.uuid,"version":plugin.version},"devicePixelRatio":1,"colors":{},"devices":[device.info()]})
}
// Trace opt-in : uniquement le sens, le nom de commande et les identifiants non secrets. Jamais payload/settings.
struct Trace { file:std::fs::File, entries:usize }
impl Trace {fn open(uuid:&str)->Option<Self>{
 if std::env::var("DECKFORGE_PLUGIN_TRACE_UUID").ok().as_deref()!=Some(uuid){return None;}
 let path=std::env::temp_dir().join("deckforge-plugin-traces");std::fs::create_dir_all(&path).ok()?;
 let name=uuid.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_'{c}else{'_'}).take(128).collect::<String>();
 let target=path.join(format!("{name}-{}.jsonl",std::process::id()));
 let truncate=target.metadata().is_ok_and(|m|m.len()>40_000);
 let file=std::fs::OpenOptions::new().create(true).append(!truncate).write(true).truncate(truncate).open(target).ok()?;
 Some(Self{file,entries:0})
}}
fn supported_command(message:&Value)->bool {
 matches!(message.get("event").and_then(Value::as_str),Some("registerPlugin"|"registerPropertyInspector"|"logMessage"|"setTitle"|"setImage"|"setState"|"showOk"|"showAlert"|"setSettings"|"getSettings"|"setGlobalSettings"|"getGlobalSettings"|"openUrl"|"sendToPlugin"|"sendToPropertyInspector"|"switchToProfile"))
}
pub fn unsupported_events(uuid:&str)->Vec<String>{
 if std::env::var("DECKFORGE_PLUGIN_TRACE_UUID").ok().as_deref()!=Some(uuid){return Vec::new();}
 let name=uuid.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_'{c}else{'_'}).take(128).collect::<String>();
 let path=std::env::temp_dir().join("deckforge-plugin-traces").join(format!("{name}-{}.jsonl",std::process::id()));
 let Ok(data)=std::fs::read(path) else{return Vec::new()};
 let mut events=std::collections::BTreeSet::new();
 for line in data[..data.len().min(128_000)].split(|b|*b==b'\n'){if let Ok(value)=serde_json::from_slice::<Value>(line){if value["unsupported"]==true {if let Some(name)=value["event"].as_str(){events.insert(name.to_owned());}}}}
 events.into_iter().take(32).collect()
}
fn trace_message(trace:&mut Option<Trace>,direction:&str,message:&Value,unsupported:bool){
 let Some(trace)=trace.as_mut() else{return};if trace.entries>=128{return;}
 let label=message.get("event").and_then(Value::as_str).unwrap_or("<absent>");
 let label=event_label(label);
 let record=json!({"direction":direction,"event":label,"unsupported":unsupported});
 let _=writeln!(trace.file,"{record}");trace.entries+=1;
}
// Event names are identifiers, never arbitrary plugin text; all other fields are fixed vocabulary.
fn event_label(name:&str)->String{
 // Never persist arbitrary plugin-controlled text: an "event" field may contain a secret.
 // Recognized protocol identifiers are fixed vocabulary; unknown values stay anonymous.
 if matches!(name,"registerPlugin"|"registerPropertyInspector"|"logMessage"|"setTitle"|"setImage"|"setState"|"showOk"|"showAlert"|"setSettings"|"getSettings"|"setGlobalSettings"|"getGlobalSettings"|"openUrl"|"sendToPlugin"|"sendToPropertyInspector"|"switchToProfile"|"keyDown"|"keyUp"|"willAppear"|"willDisappear"|"didReceiveSettings"|"didReceiveGlobalSettings"|"titleParametersDidChange"|"deviceDidConnect"|"deviceDidDisconnect"|"propertyInspectorDidAppear"|"propertyInspectorDidDisappear") {name.to_owned()} else {"<unknown>".into()}
}
fn trace_stage(trace:&mut Option<Trace>,stage:&str){
 let Some(trace)=trace.as_mut() else{return};if trace.entries>=128{return;}
 let record=json!({"stage":stage});let _=writeln!(trace.file,"{record}");trace.entries+=1;
}
fn ws_error_kind(error:&tungstenite::Error)->&'static str{
 match error {tungstenite::Error::Io(e) if matches!(e.kind(),std::io::ErrorKind::TimedOut|std::io::ErrorKind::WouldBlock)=>"timeout",
 tungstenite::Error::Io(_)=>"io",tungstenite::Error::Protocol(_)=>"protocol",
 tungstenite::Error::Capacity(_)=>"capacity",_=>"other"}
}
fn trace_ws_error(trace:&mut Option<Trace>,stage:&str,error:&tungstenite::Error){
 let Some(trace)=trace.as_mut() else{return};if trace.entries>=128{return;}
 let record=json!({"stage":stage,"category":ws_error_kind(error)});let _=writeln!(trace.file,"{record}");trace.entries+=1;
}
struct Pending {ws:tungstenite::WebSocket<std::net::TcpStream>,first:bool,accepted:Instant,warned:bool}
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
pub struct Host { #[cfg(windows)] job:Job, pub incoming:Receiver<Value>, outgoing:SyncSender<Value>, feedback:Sender<Value>, pub ws_port:u16, pub http_port:Option<u16>, pub inspectors:Option<crate::plugin_http::Inspectors>, pub folder:std::path::PathBuf, pub uuid:String, child:Child, worker:Option<thread::JoinHandle<()>>, stop:Arc<AtomicBool>, _http:Option<crate::plugin_http::HtmlServer> }
impl Host {
 pub fn start(plugin:&Plugin,device:&crate::plugin_runtime::Device)->anyhow::Result<Self>{
  if let Some(reason)=&plugin.prerequisite {anyhow::bail!("Prérequis manquant : {reason}");}
  let listener=TcpListener::bind("127.0.0.1:0")?;
  let port=listener.local_addr()?.port();
  let mut process=match &plugin.runtime {
   Runtime::Exe(p)=>Command::new(p),
   Runtime::Node(p)=>{let required=required_node(plugin);let node=node_executable(required).ok_or_else(||anyhow::anyhow!("Node.js >= {required} introuvable (système, FIFINE ou Elgato)"))?;let mut c=Command::new(node);c.arg(p);c},
   Runtime::Html(_)=>{let edge=std::path::Path::new(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");anyhow::ensure!(edge.is_file(),"Microsoft Edge introuvable");Command::new(edge)},
   _=>anyhow::bail!("Plugin protégé ou type non pris en charge"),
  };
  let info=launch_info(plugin,device);
  let needs_http=matches!(plugin.runtime,Runtime::Html(_)) || plugin.inspector.is_some() || plugin.actions.iter().any(|a|a.inspector.is_some());
  let http=if needs_http {Some(crate::plugin_http::HtmlServer::start(plugin.folder.clone(),plugin.uuid.clone(),info.to_string(),port)?)}else{None};
  if let (Runtime::Html(path),Some(server))=(&plugin.runtime,&http){let name=path.strip_prefix(&plugin.folder)?.to_string_lossy().replace(std::path::MAIN_SEPARATOR,"/");let profile=std::env::current_exe()?.parent().ok_or_else(||anyhow::anyhow!("Dossier exécutable absent"))?.join("plugin-browser").join(plugin.uuid.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_'{c}else{'_'}).collect::<String>());process.arg("--headless=new").arg("--disable-gpu").arg("--no-first-run").arg("--no-default-browser-check").arg(format!("--user-data-dir={}",profile.display())).arg(format!("http://127.0.0.1:{}/{}/{}",server.port,plugin.uuid,name));}
  if !matches!(plugin.runtime,Runtime::Html(_)){process.arg("-port").arg(port.to_string()).arg("-pluginUUID").arg(&plugin.uuid).arg("-registerEvent").arg("registerPlugin").arg("-info").arg(info.to_string());}
  #[cfg(windows)] let job=Job::new()?;
  // Comme Stream Dock/Stream Deck sous Windows : un exécutable natif démarre dans son propre dossier
  // (ex. bin/ ou plugin/), ses chemins relatifs y sont résolus ; Node et HTML restent à la racine du plugin.
  let mut child=process.current_dir(working_dir(plugin)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn()?;
  #[cfg(windows)] if let Err(e)=job.assign(&child){let _=child.kill();let _=child.wait();return Err(e.into());}
  let (outgoing,commands)=mpsc::sync_channel::<Value>(64);let (messages,incoming)=mpsc::channel();let feedback=messages.clone();
  let stop=Arc::new(AtomicBool::new(false));let stopped=stop.clone();
  let registry=http.as_ref().map(|server|server.inspectors.clone());
  let trace=Trace::open(&plugin.uuid);
  let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,stopped,registry,trace));
  Ok(Self{#[cfg(windows)] job,incoming,outgoing,feedback,ws_port:port,http_port:http.as_ref().map(|h|h.port),inspectors:http.as_ref().map(|h|h.inspectors.clone()),folder:plugin.folder.clone(),uuid:plugin.uuid.clone(),child,worker:Some(worker),stop,_http:http})
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
 pub fn send(&self,message:Value)->Result<(),TrySendError<Value>>{let result=self.outgoing.try_send(message);if result.is_err(){host_error(&self.feedback,"File de commandes plugin pleine ou arrêtée");}result}
}
fn host_error(messages:&Sender<Value>,reason:&'static str){let _=messages.send(json!({"event":"__hostError","reason":reason}));}
// send() confirms only a local WebSocket write, never execution or acknowledgement by the plugin.
fn send_traced(ws:&mut tungstenite::WebSocket<std::net::TcpStream>,cmd:&Value,messages:&Sender<Value>,trace:&mut Option<Trace>)->bool{
 trace_message(trace,"host->plugin attempted",cmd,false);
 match ws.send(tungstenite::Message::Text(cmd.to_string().into())){
  Ok(())=>{trace_message(trace,"host->plugin sent",cmd,false);true},
  Err(e)=>{trace_ws_error(trace,"host->plugin send_failed",&e);host_error(messages,"Échec d'envoi WebSocket au plugin ou inspecteur");false}
 }
}
fn socket_loop(listener:TcpListener,commands:Receiver<Value>,messages:Sender<Value>,stop:Arc<AtomicBool>,registry:Option<crate::plugin_http::Inspectors>){socket_loop_traced(listener,commands,messages,stop,registry,None)}
fn socket_loop_traced(listener:TcpListener,commands:Receiver<Value>,messages:Sender<Value>,stop:Arc<AtomicBool>,registry:Option<crate::plugin_http::Inspectors>,mut trace:Option<Trace>){
    let _=listener.set_nonblocking(true);
    let mut plugin:Option<tungstenite::WebSocket<std::net::TcpStream>>=None;
    let mut pending:Vec<Pending>=Vec::new();
    let (mut tcp_count,mut handshake_count,mut register_count)=(0u32,0u32,0u32);
    let mut queued:std::collections::VecDeque<(Instant,Value)>=std::collections::VecDeque::new();
    let mut inspectors:HashMap<String,tungstenite::WebSocket<std::net::TcpStream>>=HashMap::new();
    while !stop.load(Ordering::Relaxed){
        match listener.accept(){Ok((stream,_))=>{tcp_count+=1;trace_stage(&mut trace,"tcp_accept");let _=stream.set_nonblocking(false); // Windows : le socket accepté hérite du mode non bloquant de l'écouteur.
            let _=stream.set_read_timeout(Some(Duration::from_secs(2)));let _=stream.set_write_timeout(Some(Duration::from_secs(1)));
            match tungstenite::accept(stream){Ok(ws)=>{handshake_count+=1;trace_stage(&mut trace,"handshake_ok");let _=ws.get_ref().set_read_timeout(Some(Duration::from_millis(50)));pending.push(Pending{ws,first:true,accepted:Instant::now(),warned:false});},Err(tungstenite::HandshakeError::Failure(e))=>trace_ws_error(&mut trace,"handshake_error",&e),Err(tungstenite::HandshakeError::Interrupted(_))=>trace_stage(&mut trace,"handshake_interrupted")}},Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{},Err(_)=>break}
        for i in (0..pending.len()).rev(){
            let mut removed=false;
            match pending[i].ws.read(){
                Ok(tungstenite::Message::Text(text))=>{
                    let parsed=serde_json::from_str::<Value>(&text);
                    if pending[i].first {pending[i].first=false;
                        if let Ok(value)=&parsed {trace_stage(&mut trace,"first_message");trace_message(&mut trace,"pre-register",value,!supported_command(value));}
                        else {trace_stage(&mut trace,"first_malformed_json");}
                    }
                    if let Ok(value)=parsed{match value["event"].as_str(){
                        Some("registerPlugin") if plugin.is_none()=>{register_count+=1;trace_stage(&mut trace,"register_plugin");trace_message(&mut trace,"plugin->host",&value,false);plugin=Some(pending.swap_remove(i).ws);removed=true;let _=messages.send(value);},
                        Some("registerPropertyInspector")=>{if let Some(id)=value["uuid"].as_str().filter(|id|registry.as_ref().is_some_and(|r|r.lock().unwrap().contains_key(*id))){let id=id.to_owned();inspectors.insert(id.clone(),pending.swap_remove(i).ws);removed=true;let _=messages.send(json!({"event":"propertyInspectorDidAppear","__pi":id}));}},
                        _=>{}
                    }}
                },
                Ok(tungstenite::Message::Close(_))=>{trace_stage(&mut trace,"socket_closed_pre_register");pending.swap_remove(i);removed=true;},
                Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},
                Err(e)=>{trace_ws_error(&mut trace,"socket_error_pre_register",&e);pending.swap_remove(i);removed=true;},_=>{}
            }
            if !removed && pending[i].accepted.elapsed()>=Duration::from_secs(3) && !pending[i].warned {
                trace_stage(&mut trace,"no_register_timeout");pending[i].warned=true;
            }
        }
        while let Some((at,_))=queued.front(){if at.elapsed()<Duration::from_secs(3){break;}queued.pop_front();trace_stage(&mut trace,"command_expired_no_register");host_error(&messages,"Commande plugin expirée sans enregistrement WebSocket");}
        while let Ok(cmd)=commands.try_recv(){ if queued.len()<64 {queued.push_back((Instant::now(),cmd));}else{trace_stage(&mut trace,"command_queue_full");host_error(&messages,"File de commandes plugin pleine sans enregistrement WebSocket");} }
        while plugin.is_some() && !queued.is_empty() {
            let (_,mut cmd)=queued.pop_front().unwrap();
            let target=cmd.get("__pi").and_then(Value::as_str).map(str::to_owned);
            if let Some(obj)=cmd.as_object_mut(){obj.remove("__pi");}
            // Attempt and outcome are recorded by send_traced, not before routing.
            if let Some(id)=target{if let Some(ws)=inspectors.get_mut(&id){if !send_traced(ws,&cmd,&messages,&mut trace){inspectors.remove(&id);if let Some(r)=&registry{r.lock().unwrap().remove(&id);}let _=messages.send(json!({"event":"propertyInspectorDidDisappear","__pi":id}));}}else{host_error(&messages,"Inspecteur WebSocket indisponible");}}
            else if let Some(ws)=plugin.as_mut(){if !send_traced(ws,&cmd,&messages,&mut trace){plugin=None;}}
        }
        if let Some(ws)=plugin.as_mut(){match ws.read(){Ok(tungstenite::Message::Text(text))=>{match serde_json::from_str::<Value>(&text){Ok(value)=>{trace_message(&mut trace,"plugin->host",&value,!supported_command(&value));let _=messages.send(value);},Err(e)=>{let _=messages.send(json!({"event":"__hostError","reason":format!("Message JSON malformé : {e}")}));}}},Ok(tungstenite::Message::Close(_))=>{trace_stage(&mut trace,"socket_closed_plugin");plugin=None;},Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},Err(e)=>{trace_ws_error(&mut trace,"socket_error_plugin",&e);plugin=None;},_=>{}}}
        let mut gone=Vec::new();
        for (id,ws) in &mut inspectors {match ws.read(){Ok(tungstenite::Message::Text(text))=>{if let Ok(mut value)=serde_json::from_str::<Value>(&text){if let Some(obj)=value.as_object_mut(){obj.insert("__pi".into(),json!(id));let _=messages.send(value);}}},Ok(tungstenite::Message::Close(_))=>gone.push(id.clone()),Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},Err(_)=>gone.push(id.clone()),_=>{}}}
        for id in gone {inspectors.remove(&id);if let Some(r)=&registry{r.lock().unwrap().remove(&id);}let _=messages.send(json!({"event":"propertyInspectorDidDisappear","__pi":id}));}
        thread::sleep(Duration::from_millis(10));
    }
    if let Some(t)=trace.as_mut(){if t.entries<128{let _=writeln!(t.file,"{}",json!({"stage":"startup_counts","tcp_accepted":tcp_count,"handshakes_ok":handshake_count,"registered":register_count}));}}
}
impl Drop for Host {fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);
 #[cfg(windows)] self.job.close();
 let _=self.child.kill();let _=self.child.wait(); // Le thread se termine au timeout de lecture et fermeture du canal.
 let _=self.worker.take();
}}
#[cfg(windows)] use std::os::windows::process::CommandExt;
#[cfg(test)] mod tests {use super::*;
 #[test] fn websocket_send_trace_success_and_failure(){
  let dir=tempfile::tempdir().unwrap();let path=dir.path().join("trace.jsonl");
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let addr=listener.local_addr().unwrap();
  let client=thread::spawn(move||tungstenite::connect(format!("ws://{addr}")).unwrap().0);
  let (stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let mut peer=client.join().unwrap();
  let (messages,rx)=mpsc::channel();let mut trace=Some(Trace{file:std::fs::File::create(&path).unwrap(),entries:0});
  let cmd=json!({"event":"keyDown","payload":{"settings":"SECRET"}});
  assert!(send_traced(&mut ws,&cmd,&messages,&mut trace));assert_eq!(serde_json::from_str::<Value>(&peer.read().unwrap().to_string()).unwrap(),cmd);
  ws.close(None).unwrap();assert!(!send_traced(&mut ws,&cmd,&messages,&mut trace));
  assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap()["event"],"__hostError");
  let data=std::fs::read_to_string(path).unwrap();assert!(data.contains("host->plugin attempted"));assert!(data.contains("host->plugin sent"));assert!(data.contains("host->plugin send_failed"));assert!(!data.contains("SECRET"));
 }
 #[test] fn commands_expire_without_registration(){
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let (tx,commands)=mpsc::sync_channel(64);let (messages,rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let worker=thread::spawn(move||socket_loop(listener,commands,messages,flag,None));tx.send(json!({"event":"keyDown"})).unwrap();
  let error=rx.recv_timeout(Duration::from_secs(5)).unwrap();assert_eq!(error["event"],"__hostError");assert!(error["reason"].as_str().unwrap().contains("expirée"));
  stop.store(true,Ordering::Relaxed);worker.join().unwrap();
 }
 #[test] fn node_requirement_and_sdk_v2(){let d=tempfile::tempdir().unwrap();std::fs::create_dir(d.path().join("bin")).unwrap();std::fs::write(d.path().join("bin/plugin.js"),b"").unwrap();let p=crate::plugins::parse(d.path(),br#"{"CodePath":"bin/plugin.js","Nodejs":{"Version":"18.17"}}"#).unwrap();assert_eq!(required_node(&p),20);let p=crate::plugins::parse(d.path(),br#"{"CodePath":"bin/plugin.js","Nodejs":{"Version":"22"}}"#).unwrap();assert_eq!(required_node(&p),22);}
 #[test] fn trace_redacts_payload_and_marks_unknown(){
  let d=tempfile::tempdir().unwrap();let path=d.path().join("trace.jsonl");
  let file=std::fs::File::create(&path).unwrap();let mut trace=Some(Trace{file,entries:0});
  let unknown=json!({"event":"alien","payload":{"token":"SECRET"}});
  trace_message(&mut trace,"plugin->host",&unknown,!supported_command(&unknown));
  let data=std::fs::read_to_string(path).unwrap();assert!(data.contains(r#""unsupported":true"#));assert!(!data.contains("SECRET"));
 }
 #[test] fn trace_pre_registration_fixture_redacts_and_keeps_unknown(){
  let d=tempfile::tempdir().unwrap();let path=d.path().join("trace.jsonl");
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let (_tx,commands)=mpsc::channel::<Value>();let (messages,rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let trace=Some(Trace{file:std::fs::File::create(&path).unwrap(),entries:0});
  let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,flag,None,trace));
  let (mut ws,_)=tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
  ws.send(tungstenite::Message::Text(json!({"event":"alien","payload":{"settings":"SECRET"}}).to_string().into())).unwrap();
  ws.send(tungstenite::Message::Text(json!({"event":"registerPlugin","uuid":"fixture","info":"SECRET"}).to_string().into())).unwrap();
  assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap()["event"],"registerPlugin");
  stop.store(true,Ordering::Relaxed);worker.join().unwrap();
  let data=std::fs::read_to_string(path).unwrap();
  for marker in ["tcp_accept","handshake_ok","first_message","register_plugin","startup_counts","<unknown>"] {assert!(data.contains(marker),"missing {marker}: {data}");}
  assert!(data.contains("\"unsupported\":true"));assert!(!data.contains("SECRET"));assert!(!data.contains("fixture"));
  assert_eq!(event_label("bad SECRET /path\n"),"<unknown>");
 }
 #[test] fn trace_handshake_failure_and_malformed_first_are_redacted(){
  let d=tempfile::tempdir().unwrap();let path=d.path().join("trace.jsonl");let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let (_tx,commands)=mpsc::channel::<Value>();let (messages,_rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let trace=Some(Trace{file:std::fs::File::create(&path).unwrap(),entries:0});let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,flag,None,trace));
  let mut bad=std::net::TcpStream::connect(("127.0.0.1",port)).unwrap();bad.write_all(b"SECRET\r\n\r\n").unwrap();drop(bad);
  let (mut ws,_)=tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
  ws.send(tungstenite::Message::Text("SECRET invalid json".into())).unwrap();
  ws.close(None).unwrap();thread::sleep(Duration::from_millis(150));stop.store(true,Ordering::Relaxed);worker.join().unwrap();
  let data=std::fs::read_to_string(path).unwrap();assert!(data.contains("handshake_error"),"{data}");assert!(data.contains("first_malformed_json"),"{data}");assert!(data.contains("socket_closed_pre_register"),"{data}");assert!(!data.contains("SECRET"));
 }
 #[test] fn trace_warns_without_register(){
  let d=tempfile::tempdir().unwrap();let path=d.path().join("trace.jsonl");let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let (_tx,commands)=mpsc::channel::<Value>();let (messages,_rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let trace=Some(Trace{file:std::fs::File::create(&path).unwrap(),entries:0});let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,flag,None,trace));
  let (_ws,_)=tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();thread::sleep(Duration::from_millis(3200));
  stop.store(true,Ordering::Relaxed);worker.join().unwrap();assert!(std::fs::read_to_string(path).unwrap().contains("no_register_timeout"));
 }
 #[test] fn native_plugins_start_in_their_executable_folder(){
  let d=tempfile::tempdir().unwrap();std::fs::create_dir(d.path().join("bin")).unwrap();std::fs::write(d.path().join("bin/tool.exe"),b"").unwrap();std::fs::write(d.path().join("plugin.js"),b"").unwrap();
  let exe=crate::plugins::parse(d.path(),br#"{"UUID":"t","CodePathWin":"bin/tool.exe"}"#).unwrap();
  assert_eq!(working_dir(&exe),d.path().join("bin"));
  let node=crate::plugins::parse(d.path(),br#"{"UUID":"t","CodePath":"plugin.js"}"#).unwrap();
  assert_eq!(working_dir(&node),d.path().to_path_buf());
 }
 #[test] fn launch_contract_and_unknown_command(){
  let d=tempfile::tempdir().unwrap();let p=crate::plugins::parse(d.path(),br#"{"UUID":"test","Version":"1.0"}"#).unwrap();
  let info=launch_info(&p,&crate::plugin_runtime::Device::virtual_device());assert_eq!(info["application"]["platform"],"windows");
  assert!(info["application"]["platformVersion"].is_string());assert_eq!(info["plugin"]["uuid"],"test");
  assert_eq!(info["devices"][0]["size"]["columns"],5);
  let xl=crate::plugin_runtime::Device::from_grid("Elgato Stream Deck XL".into(),8,4);
  assert_eq!(launch_info(&p,&xl)["devices"][0]["size"],json!({"columns":8,"rows":4}));assert!(!supported_command(&json!({"event":"alien"})));
  assert!(supported_command(&json!({"event":"setTitle"})));
 }
 #[cfg(windows)] #[test] fn job_closing_kills_descendant(){
  use windows_sys::Win32::System::Threading::{OpenProcess,PROCESS_SYNCHRONIZE,WaitForSingleObject};
  use windows_sys::Win32::Foundation::{CloseHandle,WAIT_OBJECT_0};
  let dir=tempfile::tempdir().unwrap();let file=dir.path().join("pid.txt");let job=Job::new().unwrap();
  let script=format!("$p=Start-Process powershell -ArgumentList '-NoProfile','-Command','Start-Sleep -Seconds 30' -PassThru -WindowStyle Hidden; $p.Id | Set-Content -Path '{}'",file.display());
  let mut parent=Command::new("powershell").args(["-NoProfile","-Command",&script]).creation_flags(0x08000000).spawn().unwrap();
  job.assign(&parent).unwrap();
  // PowerShell peut encore tenir pid.txt ouvert : relire jusqu'à un PID complet.
  let limit=std::time::Instant::now()+Duration::from_secs(10);
  let pid:u32=loop {
   if let Some(pid)=std::fs::read_to_string(&file).ok().and_then(|text|text.trim().parse().ok()){break pid;}
   assert!(std::time::Instant::now()<limit,"descendant was not started");
   thread::sleep(Duration::from_millis(30));
  };
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
 #[test] fn registered_client_receives_key_events_for_distinct_contexts(){
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let (tx,commands)=mpsc::sync_channel(64);let (messages,rx)=mpsc::channel();
  let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let worker=thread::spawn(move||socket_loop(listener,commands,messages,flag,None));
  let (mut peer,_)=tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
  if let tungstenite::stream::MaybeTlsStream::Plain(stream)=peer.get_mut(){stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();}
  peer.send(tungstenite::Message::Text(json!({"event":"registerPlugin","uuid":"fixture"}).to_string().into())).unwrap();
  assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap()["event"],"registerPlugin");
  let device=crate::plugin_runtime::Device::virtual_device();
  for (context,index,name) in [("ctx-a",0,"keyDown"),("ctx-b",1,"keyDown"),("ctx-a",0,"keyUp")] {
   let visible=crate::plugin_runtime::Visible{context:context.into(),plugin:"fixture".into(),action:"fixture.increment".into(),index,settings:json!({}),state:0};
   let expected=crate::plugin_runtime::event(name,&visible,&device);
   tx.send(expected.clone()).unwrap();
   let received=serde_json::from_str::<Value>(&peer.read().unwrap().to_string()).unwrap();
   assert_eq!(received,expected);
   assert_eq!(received["action"],"fixture.increment");
   assert_eq!(received["context"],context);
   assert_eq!(received["device"],device.id);
   assert_eq!(received["payload"]["controller"],"Keypad");
   assert_eq!(received["payload"]["resources"],json!({}));
  }
  peer.send(tungstenite::Message::Text(json!({"event":"setTitle","context":"ctx-b","payload":{"title":"1"}}).to_string().into())).unwrap();
  let response=rx.recv_timeout(Duration::from_secs(3)).unwrap();
  assert_eq!(response["event"],"setTitle");assert_eq!(response["context"],"ctx-b");assert_eq!(response["payload"]["title"],"1");
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
 #[cfg(windows)] #[test] #[ignore="requires explicitly supplied, audited OpenAction counter v1.0.1 release archive"]
 fn imported_openaction_counter_executes_key_up(){
  use sha2::{Digest,Sha256};
  let path=std::path::PathBuf::from(std::env::var_os("DECKFORGE_TEST_OACOUNTER_ZIP").expect("set DECKFORGE_TEST_OACOUNTER_ZIP to the v1.0.1 release ZIP"));
  let bytes=std::fs::read(&path).unwrap();
  assert_eq!(format!("{:x}",Sha256::digest(&bytes)),"f04e0724188468e450633a49d15ee98ab0400faa2197332cfcf80278d0c54e47");
  let dir=tempfile::tempdir().unwrap();
  let installed=dir.path().join("plugins");
  assert!(crate::plugin_import::import_archive(&path,&installed).unwrap());
  let plugin=crate::plugin_import::inventory(&installed).into_iter().find(|p|p.uuid=="me.amankhanna.oacounter").expect("imported counter missing");
  let device=crate::plugin_runtime::Device::virtual_device();
  let host=Host::start(&plugin,&device).expect("failed to start imported counter");
  assert_eq!(host.incoming.recv_timeout(Duration::from_secs(15)).unwrap()["event"],"registerPlugin");
  let action=crate::plugin_runtime::Visible{context:"counter-test-context".into(),plugin:plugin.uuid.clone(),action:"me.amankhanna.oacounter.persisted".into(),index:0,settings:json!({}),state:0};
  host.send(json!({"event":"deviceDidConnect","device":device.id,"deviceInfo":device.info()})).unwrap();
  for name in ["willAppear","keyDown","keyUp"]{host.send(crate::plugin_runtime::event(name,&action,&device)).unwrap();}
  let mut settings=false;let mut title=false;
  let deadline=std::time::Instant::now()+Duration::from_secs(15);
  while std::time::Instant::now()<deadline && !(settings && title){
   let remaining=deadline.saturating_duration_since(std::time::Instant::now());
   let message=host.incoming.recv_timeout(remaining).expect("counter did not return title and settings");
   assert_eq!(message["context"],"counter-test-context");
   if message["event"]=="setSettings" {assert_eq!(message["payload"]["value"],1);settings=true;}
   if message["event"]=="setTitle" {assert_eq!(message["payload"]["title"],"1");title=true;}
  }
  assert!(settings && title,"imported plugin must execute keyUp and return its observable counter state");
 }
 /// Matrice de compatibilité : un plugin tiers extrait doit s'enregistrer puis réagir à un appui
 /// (titre, image, état, alerte/OK ou réglages) pour le contexte visé. Jamais lancé par défaut.
 #[test] #[ignore="set DECKFORGE_TEST_PLUGIN_DIR and DECKFORGE_TEST_PLUGIN_ACTION; starts third-party code"]
 fn external_plugin_folder_reacts_to_key_press(){
  let var=|name:&str|std::env::var(name).unwrap_or_else(|_|panic!("set {name}"));
  let action=var("DECKFORGE_TEST_PLUGIN_ACTION");let imported=tempfile::tempdir().unwrap();
  // DECKFORGE_TEST_PLUGIN_ARCHIVE : même chemin que le bouton d'import (.streamDeckPlugin/.zip), dans un dossier temporaire.
  let plugin=if let Ok(archive)=std::env::var("DECKFORGE_TEST_PLUGIN_ARCHIVE"){
   assert!(crate::plugin_import::import_archive(std::path::Path::new(&archive),imported.path()).unwrap());
   crate::plugin_import::inventory(imported.path()).into_iter().find(|p|p.actions.iter().any(|a|a.uuid==action)).expect("imported plugin with this action")
  }else{
   let folder=std::path::PathBuf::from(var("DECKFORGE_TEST_PLUGIN_DIR"));
   crate::plugins::parse(&folder,&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap()
  };
  let device=crate::plugin_runtime::Device::virtual_device();
  let host=Host::start(&plugin,&device).expect("plugin start");
  assert_eq!(host.incoming.recv_timeout(Duration::from_secs(20)).expect("no registerPlugin")["event"],"registerPlugin");
  let visible=crate::plugin_runtime::Visible{context:"matrix-context".into(),plugin:plugin.uuid.clone(),action,index:0,settings:json!({}),state:0};
  host.send(json!({"event":"deviceDidConnect","device":device.id,"deviceInfo":device.info()})).unwrap();
  host.send(crate::plugin_runtime::event("willAppear",&visible,&device)).unwrap();
  std::thread::sleep(Duration::from_millis(500));
  for name in ["keyDown","keyUp"]{host.send(crate::plugin_runtime::event(name,&visible,&device)).unwrap();}
  let deadline=Instant::now()+Duration::from_secs(8);let mut seen=Vec::new();
  while let Ok(message)=host.incoming.recv_timeout(deadline.saturating_duration_since(Instant::now())){
   let name=message["event"].as_str().unwrap_or("").to_owned();
   assert_ne!(name,"__hostError","{message}");
   if message["context"]=="matrix-context" || message.get("context").is_none(){seen.push(name);}
  }
  println!("MATRIX {} events: {}",plugin.uuid,seen.join(","));
  assert!(seen.iter().any(|e|matches!(e.as_str(),"setTitle"|"setImage"|"setState"|"showOk"|"showAlert"|"setSettings"|"setFeedback")),"no visible reaction: {seen:?}");
 }
 #[test] fn node_fixture_exchanges_key_messages(){
  let fixture=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sdplugin");
  let p=crate::plugins::parse(&fixture,&std::fs::read(fixture.join("manifest.json")).unwrap()).unwrap();
  let node=node_executable(0).expect("Node.js required for the plugin fixture test");
  let support=Command::new(&node).args(["-p","typeof WebSocket"]).output().expect("Node.js fixture preflight failed");
  assert!(support.status.success() && String::from_utf8_lossy(&support.stdout).trim()=="function", "Node.js fixture needs global WebSocket: {} {}", String::from_utf8_lossy(&support.stdout), String::from_utf8_lossy(&support.stderr));
  let host=Host::start(&p,&crate::plugin_runtime::Device::virtual_device()).expect("Failed to start plugin fixture");
  let register=host.incoming.recv_timeout(Duration::from_secs(15)).unwrap();
  assert_eq!(register["event"],"registerPlugin");
  let payload=json!({"settings":{},"coordinates":{"column":0,"row":0},"state":0,"isInMultiAction":false,"controller":"Keypad"});
  let event=|name|json!({"event":name,"action":"test.deckforge.fixture.action","context":"ctx-1","device":"deckforge-virtual","payload":payload});
  host.send(json!({"event":"deviceDidConnect","device":"deckforge-virtual","deviceInfo":crate::plugin_runtime::Device::virtual_device().info()})).unwrap();
  host.send(event("willAppear")).unwrap();host.send(event("keyDown")).unwrap();
  let mut names=std::collections::HashSet::new();
  for _ in 0..4 {let message=host.incoming.recv_timeout(Duration::from_secs(15)).unwrap();assert_eq!(message["context"],"ctx-1");if message["event"]=="setTitle"{assert_eq!(message["payload"]["title"],"OK 1");}if message["event"]=="setSettings"{assert_eq!(message["payload"]["presses"],1);}names.insert(message["event"].as_str().unwrap().to_string());}
  assert_eq!(names,std::collections::HashSet::from(["setTitle".into(),"setImage".into(),"setSettings".into(),"showOk".into()]));
 } #[test] fn rejected_protected_plugin_does_not_spawn(){let d=tempfile::tempdir().unwrap();let p=crate::plugins::parse(d.path(),b"ELGATO\x01payload").unwrap();assert!(Host::start(&p,&crate::plugin_runtime::Device::virtual_device()).is_err());}
}
