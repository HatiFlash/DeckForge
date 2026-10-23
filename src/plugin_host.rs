//! Hôte local WebSocket pour les plugins Stream Deck classiques (exécutables et Node).
use std::{collections::HashMap, net::TcpListener, process::{Child,Command,Stdio}, sync::{mpsc::{self,Receiver,Sender,SyncSender,TrySendError},Arc,Mutex,atomic::{AtomicBool,Ordering}}, thread, time::{Duration,Instant}};
use std::io::Write;
use serde_json::{json,Value};
use crate::plugins::{Plugin,Runtime};

/// Résultat mis en cache par version minimale : chaque sonde lance `node --version`.
fn node_executable(required:u32)->Option<std::path::PathBuf>{
 static CACHE:std::sync::OnceLock<Mutex<HashMap<u32,Option<std::path::PathBuf>>>>=std::sync::OnceLock::new();
 let cache=CACHE.get_or_init(Default::default);
 if let Some(found)=cache.lock().ok().and_then(|c|c.get(&required).cloned()){return found;}
 let found=find_node(required);
 if let Ok(mut c)=cache.lock(){c.insert(required,found.clone());}
 found
}
fn find_node(required:u32)->Option<std::path::PathBuf>{
 let mut candidates=Vec::new();
 if let Some(paths)=std::env::var_os("PATH"){candidates.extend(std::env::split_paths(&paths).map(|p|p.join("node.exe")));}
 if let Some(p)=std::env::var_os("ProgramFiles"){candidates.push(std::path::PathBuf::from(p).join("nodejs/node.exe"));}
 if let Some(p)=std::env::var_os("ProgramFiles(x86)"){candidates.push(std::path::PathBuf::from(p).join("fifine Control Deck/node/node20.exe"));}
 if let Some(p)=std::env::var_os("APPDATA"){let root=std::path::PathBuf::from(p).join("Elgato/StreamDeck/NodeJS");candidates.push(root.join("node.exe"));if let Ok(entries)=std::fs::read_dir(root){for entry in entries.flatten().take(32){candidates.push(entry.path().join("node.exe"));}}}
 candidates.into_iter().filter(|p|p.is_file()).find(|p|node_major(p).is_some_and(|v|v>=required))
}
fn node_major(path:&std::path::Path)->Option<u32>{
 let mut command=Command::new(path);
 command.arg("--version").stdin(Stdio::null());
 #[cfg(windows)] command.creation_flags(0x08000000); // CREATE_NO_WINDOW : aucune console visible
 let output=command.output().ok()?;
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
 json!({"application":{"font":"","language":crate::i18n::current(),"platform":"windows","platformVersion":platform_version,"version":env!("CARGO_PKG_VERSION")},"plugin":{"uuid":plugin.uuid,"version":plugin.version},"devicePixelRatio":1,"colors":{},"devices":[device.info()]})
}
/// Le handle n'est qu'une indication de présentation ; l'enfant vérifie son processus propriétaire.
static MAIN_WINDOW:std::sync::atomic::AtomicIsize=std::sync::atomic::AtomicIsize::new(0);
pub fn set_main_window(hwnd:isize){MAIN_WINDOW.store(hwnd,Ordering::Relaxed);}
pub(crate) fn inspector_title(plugin:&str,action:&str)->String{
 let clean=|text:&str|text.chars().filter(|c|!c.is_control()).take(80).collect::<String>().trim().to_owned();
 let (plugin,action)=(clean(plugin),clean(action));
 if plugin.is_empty() || action.is_empty(){return crate::i18n::tr("Réglages du plugin").to_owned();}
 crate::i18n::trf("Réglages — {plugin} · {action}",&[("plugin",&plugin),("action",&action)])
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
struct Pending {ws:tungstenite::WebSocket<std::net::TcpStream>,browser:bool,first:bool,accepted:Instant,warned:bool}
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
fn random_session_id()->anyhow::Result<String>{let mut random=[0u8;16];getrandom::fill(&mut random)?;Ok(random.iter().map(|b|format!("{b:02x}")).collect())}
fn session_url(server:u16,uuid:&str,path:&str,id:&str)->String{
 let encoded=path.replace('\\',"/").split('/').map(|part|part.replace(' ',"%20")).collect::<Vec<_>>().join("/");
 format!("http://127.0.0.1:{server}/{uuid}/{encoded}?pi={id}")
}
fn inspector_exe()->anyhow::Result<std::path::PathBuf>{
 #[cfg(test)] if let Some(exe)=std::env::var_os("DECKFORGE_TEST_APP_EXE"){return Ok(exe.into());}
 Ok(std::env::current_exe()?)
}
pub struct Host { #[cfg(windows)] job:Job, pub incoming:Receiver<Value>, outgoing:SyncSender<Value>, feedback:Sender<Value>, pub ws_port:u16, pub http_port:Option<u16>, pub inspectors:Option<crate::plugin_http::Inspectors>, pub folder:std::path::PathBuf, pub uuid:String, child:Child, worker:Option<thread::JoinHandle<()>>, stop:Arc<AtomicBool>, inspector_children:Mutex<Vec<Child>>, _http:Option<crate::plugin_http::HtmlServer> }
impl Host {
 pub fn start(plugin:&Plugin,device:&crate::plugin_runtime::Device)->anyhow::Result<Self>{
  if let Some(reason)=&plugin.prerequisite {anyhow::bail!("Prérequis manquant : {reason}");}
  // HTML plugin backends run in the isolated WebView2 process (hidden), never in Edge --app.
  let html_runtime=match &plugin.runtime {Runtime::Html(p)=>Some(p.clone()),_=>None};
  let listener=TcpListener::bind("127.0.0.1:0")?;
  let port=listener.local_addr()?.port();
  let mut process=match &plugin.runtime {
   Runtime::Exe(p)=>Command::new(p),
   Runtime::Node(p)=>{let required=required_node(plugin);let node=node_executable(required).ok_or_else(||anyhow::anyhow!("Node.js >= {required} introuvable (système, FIFINE ou Elgato)"))?;let mut c=Command::new(node);c.arg(p);c},
   Runtime::Html(_)=>{let mut c=Command::new(inspector_exe()?);c.arg("--isolated-inspector");c},
   _=>anyhow::bail!("Plugin protégé ou type non pris en charge"),
  };
  let info=launch_info(plugin,device);
  let needs_http=html_runtime.is_some() || plugin.inspector.is_some() || plugin.actions.iter().any(|a|a.inspector.is_some());
  let http=if needs_http {Some(crate::plugin_http::HtmlServer::start(plugin.folder.clone(),plugin.uuid.clone(),info.to_string(),port)?)}else{None};
  // Runtime page URL with a single-use token; only that page may register as the plugin from a browser.
  let runtime_url=if let Some(page)=&html_runtime {
   let http=http.as_ref().unwrap();
   let root=plugin.folder.canonicalize()?;let page=page.canonicalize()?;
   let rel=page.strip_prefix(&root).map_err(|_|anyhow::anyhow!("Page HTML hors du plugin"))?.to_string_lossy().replace('\\',"/");
   let token=random_session_id()?;
   let url=session_url(http.port,&plugin.uuid,&rel,&token);
   anyhow::ensure!(crate::plugin_inspector::allowed_initial(&url),"Chemin HTML du plugin non pris en charge");
   http.inspectors.lock().unwrap().insert(token,crate::plugin_http::RUNTIME_SESSION.into());
   Some(url)
  } else {
   process.arg("-port").arg(port.to_string()).arg("-pluginUUID").arg(&plugin.uuid).arg("-registerEvent").arg("registerPlugin").arg("-info").arg(info.to_string());
   None
  };
  #[cfg(windows)] let job=Job::new()?;
  // Comme Stream Dock/Stream Deck sous Windows : un exécutable natif démarre dans son propre dossier
  // (ex. bin/ ou plugin/), ses chemins relatifs y sont résolus ; Node et HTML restent à la racine du plugin.
  let stdin=if runtime_url.is_some(){Stdio::piped()}else{Stdio::null()};
  let mut child=process.current_dir(working_dir(plugin)).stdin(stdin).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn()?;
  #[cfg(windows)] if let Err(e)=job.assign(&child){let _=child.kill();let _=child.wait();return Err(e.into());}
  if let Some(url)=&runtime_url {
   if let Err(e)=child.stdin.take().unwrap().write_all(format!("{url}\n{port}\nruntime\n").as_bytes()){let _=child.kill();let _=child.wait();return Err(e.into());}
  }

  let (outgoing,commands)=mpsc::sync_channel::<Value>(64);let (messages,incoming)=mpsc::channel();let feedback=messages.clone();
  let stop=Arc::new(AtomicBool::new(false));let stopped=stop.clone();
  let registry=http.as_ref().map(|server|server.inspectors.clone());
  let trace=Trace::open(&plugin.uuid);
  let origin=http.as_ref().map(|s|format!("http://127.0.0.1:{}",s.port));
  let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,stopped,registry,trace,origin));
  Ok(Self{#[cfg(windows)] job,incoming,outgoing,feedback,ws_port:port,http_port:http.as_ref().map(|h|h.port),inspectors:http.as_ref().map(|h|h.inspectors.clone()),folder:plugin.folder.clone(),uuid:plugin.uuid.clone(),child,worker:Some(worker),stop,inspector_children:Mutex::new(Vec::new()),_http:http})
 }
 pub fn open_inspector(&self,path:&str,action_info:Value)->anyhow::Result<String> {
  self.open_inspector_with_title(path,action_info,crate::i18n::tr("Réglages du plugin"))
 }
 pub fn open_inspector_with_title(&self,path:&str,action_info:Value,title:&str)->anyhow::Result<String> {
  let title=crate::plugin_inspector::clean_title(title);
  let server=self.http_port.ok_or_else(||anyhow::anyhow!("Serveur HTML indisponible"))?;
  let registry=self.inspectors.as_ref().ok_or_else(||anyhow::anyhow!("Inspecteur indisponible"))?;
  let rel=std::path::Path::new(path);
  anyhow::ensure!(!rel.is_absolute() && rel.components().all(|c|matches!(c,std::path::Component::Normal(_))),"Chemin d'inspecteur invalide");
  let file=self.folder.join(rel).canonicalize()?;
  anyhow::ensure!(file.is_file() && file.starts_with(self.folder.canonicalize()?) && matches!(file.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(),"html"|"htm"),"Chemin d'inspecteur invalide");
  let id=random_session_id()?;
  let url=session_url(server,&self.uuid,path,&id);
  anyhow::ensure!(crate::plugin_inspector::allowed_initial(&url),"Chemin PI non pris en charge (encodage ou identifiant ambigu)");
  let mut children=self.inspector_children.lock().map_err(|_|anyhow::anyhow!("Inspecteurs indisponibles"))?;
  children.retain_mut(|child|child.try_wait().ok().flatten().is_none());
  anyhow::ensure!(children.len()<4,"Limite de quatre inspecteurs actifs");
  registry.lock().unwrap().insert(id.clone(),format!("JSON.stringify({})",action_info));
  let result=(||->anyhow::Result<Child>{
   let mut inspector=Command::new(inspector_exe()?).arg("--isolated-inspector")
       .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
   #[cfg(windows)] if let Err(e)=self.job.assign(&inspector){let _=inspector.kill();let _=inspector.wait();return Err(e.into());}
   if let Err(e)=inspector.stdin.take().unwrap().write_all(format!("{url}\n{}\n{title}\n{}\n{}\n",self.ws_port,MAIN_WINDOW.load(Ordering::Relaxed),std::process::id()).as_bytes()){let _=inspector.kill();let _=inspector.wait();return Err(e.into());}
   Ok(inspector)
  })();
  match result {Ok(child)=>{children.push(child);Ok(id)},Err(e)=>{registry.lock().unwrap().remove(&id);Err(e)}}
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
fn socket_loop(listener:TcpListener,commands:Receiver<Value>,messages:Sender<Value>,stop:Arc<AtomicBool>,registry:Option<crate::plugin_http::Inspectors>){socket_loop_traced(listener,commands,messages,stop,registry,None,None)}
fn browser_origin_allowed(origin:Option<&str>,expected:Option<&str>)->bool{
    match origin {None=>true,Some(value)=>expected==Some(value)}
}
fn socket_loop_traced(listener:TcpListener,commands:Receiver<Value>,messages:Sender<Value>,stop:Arc<AtomicBool>,registry:Option<crate::plugin_http::Inspectors>,mut trace:Option<Trace>,expected_origin:Option<String>){
    // Browser Origin is mandatory and exact; native/Node clients have no Origin.
    let _=listener.set_nonblocking(true);
    let mut plugin:Option<tungstenite::WebSocket<std::net::TcpStream>>=None;
    let mut pending:Vec<Pending>=Vec::new();
    let (mut tcp_count,mut handshake_count,mut register_count)=(0u32,0u32,0u32);
    let mut queued:std::collections::VecDeque<(Instant,Value)>=std::collections::VecDeque::new();
    let mut inspectors:HashMap<String,tungstenite::WebSocket<std::net::TcpStream>>=HashMap::new();
    while !stop.load(Ordering::Relaxed){
        match listener.accept(){Ok((stream,_))=>{if pending.len()>=16 {drop(stream);continue;}tcp_count+=1;trace_stage(&mut trace,"tcp_accept");let _=stream.set_nonblocking(false); // Windows : le socket accepté hérite du mode non bloquant de l'écouteur.
            let _=stream.set_read_timeout(Some(Duration::from_secs(2)));let _=stream.set_write_timeout(Some(Duration::from_secs(1)));
            let mut browser=false;
            let mut ws_cfg=tungstenite::protocol::WebSocketConfig::default();
            ws_cfg.max_message_size=Some(2*1024*1024);ws_cfg.max_frame_size=Some(512*1024);
            match tungstenite::accept_hdr_with_config(stream,|req:&tungstenite::handshake::server::Request,response:tungstenite::handshake::server::Response|{
                let origin=req.headers().get("origin").and_then(|v|v.to_str().ok());
                browser=origin.is_some();
                if !browser_origin_allowed(origin,expected_origin.as_deref()) || (req.headers().contains_key("origin") && origin.is_none()) {
                    return Err(tungstenite::http::Response::builder().status(403).body(Some("Forbidden".into())).unwrap());
                }
                Ok(response)
            },Some(ws_cfg)){Ok(ws)=>{handshake_count+=1;trace_stage(&mut trace,"handshake_ok");let _=ws.get_ref().set_read_timeout(Some(Duration::from_millis(50)));pending.push(Pending{ws,browser,first:true,accepted:Instant::now(),warned:false});},Err(tungstenite::HandshakeError::Failure(e))=>trace_ws_error(&mut trace,"handshake_error",&e),Err(tungstenite::HandshakeError::Interrupted(_))=>trace_stage(&mut trace,"handshake_interrupted")}},Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{},Err(_)=>break}
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
                        Some("registerPlugin") if plugin.is_none() && pending[i].browser
                            && value["uuid"].as_str().is_some_and(|id|registry.as_ref().is_some_and(|r|r.lock().unwrap().get(id).is_some_and(|v|v==crate::plugin_http::RUNTIME_SESSION)))=>{
                            // HTML plugin runtime: single-use token, then this socket is the plugin.
                            if let Some(r)=&registry{r.lock().unwrap().remove(value["uuid"].as_str().unwrap());}
                            register_count+=1;trace_stage(&mut trace,"register_html_runtime");plugin=Some(pending.swap_remove(i).ws);removed=true;
                            let mut value=value;if let Some(o)=value.as_object_mut(){o.remove("uuid");}let _=messages.send(value);},
                        Some("registerPlugin") if plugin.is_none() && !pending[i].browser=>{register_count+=1;trace_stage(&mut trace,"register_plugin");trace_message(&mut trace,"plugin->host",&value,false);plugin=Some(pending.swap_remove(i).ws);removed=true;let _=messages.send(value);},
                        Some("registerPropertyInspector")=>{if let Some(id)=value["uuid"].as_str().filter(|id|!inspectors.contains_key(*id) && registry.as_ref().is_some_and(|r|r.lock().unwrap().contains_key(*id))){let id=id.to_owned();inspectors.insert(id.clone(),pending.swap_remove(i).ws);removed=true;let _=messages.send(json!({"event":"propertyInspectorDidAppear","__pi":id}));}},
                        _=>{}
                    }}
                },
                Ok(tungstenite::Message::Close(_))=>{trace_stage(&mut trace,"socket_closed_pre_register");pending.swap_remove(i);removed=true;},
                Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},
                Err(e)=>{trace_ws_error(&mut trace,"socket_error_pre_register",&e);pending.swap_remove(i);removed=true;},_=>{}
            }
            if !removed && pending[i].accepted.elapsed()>=Duration::from_secs(3) && !pending[i].warned {
                trace_stage(&mut trace,"no_register_timeout");pending[i].warned=true;
                pending.swap_remove(i);
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
        for (id,ws) in &mut inspectors {match ws.read(){Ok(tungstenite::Message::Text(text))=>{if text.len()>64*1024 {gone.push(id.clone());continue;}if let Ok(mut value)=serde_json::from_str::<Value>(&text){if let Some(obj)=value.as_object_mut(){obj.insert("__pi".into(),json!(id));let _=messages.send(value);}}},Ok(tungstenite::Message::Close(_))=>gone.push(id.clone()),Err(tungstenite::Error::Io(ref e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},Err(_)=>gone.push(id.clone()),_=>{}}}
        for id in gone {inspectors.remove(&id);if let Some(r)=&registry{r.lock().unwrap().remove(&id);}let _=messages.send(json!({"event":"propertyInspectorDidDisappear","__pi":id}));}
        thread::sleep(Duration::from_millis(10));
    }
    if let Some(t)=trace.as_mut(){if t.entries<128{let _=writeln!(t.file,"{}",json!({"stage":"startup_counts","tcp_accepted":tcp_count,"handshakes_ok":handshake_count,"registered":register_count}));}}
}
impl Drop for Host {fn drop(&mut self){self.stop.store(true,Ordering::Relaxed);
 #[cfg(windows)] self.job.close();
 if let Ok(mut children)=self.inspector_children.lock(){for child in children.iter_mut(){let _=child.kill();let _=child.wait();}children.clear();}
 let _=self.child.kill();let _=self.child.wait(); // Le thread se termine au timeout de lecture et fermeture du canal.
 let _=self.worker.take();
}}
#[cfg(windows)] use std::os::windows::process::CommandExt;
#[cfg(test)] mod tests {use super::*;
    #[test] fn title_is_bounded_clean_and_translated() {
        crate::i18n::set_language("en");
        assert_eq!(inspector_title("Test", "Counter"), "Settings — Test · Counter");
        assert_eq!(inspector_title("\r\n", "Counter"), "Plugin settings");
        let title=inspector_title(&"a".repeat(200),"Counter\nUnsafe");
        assert!(!title.contains('\n') && title.contains("CounterUnsafe") && title.chars().count()<180);
        crate::i18n::set_language("fr");
    }
    #[test] fn launch_info_uses_active_language() {
        let d=tempfile::tempdir().unwrap();let p=crate::plugins::parse(d.path(),br#"{"UUID":"test"}"#).unwrap();
        crate::i18n::set_language("en");
        assert_eq!(launch_info(&p,&crate::plugin_runtime::Device::virtual_device())["application"]["language"],"en");
        crate::i18n::set_language("fr");
        assert_eq!(launch_info(&p,&crate::plugin_runtime::Device::virtual_device())["application"]["language"],"fr");
    }
 #[test] fn browser_websocket_origin_is_rejected_without_pi_server(){
  use tungstenite::client::IntoClientRequest;
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let (_tx,commands)=mpsc::channel::<Value>();let (messages,_rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let worker=thread::spawn(move||socket_loop(listener,commands,messages,flag,None));
  for origin in ["http://evil.invalid","http://127.0.0.1:1","null"] {
   let mut request=format!("ws://127.0.0.1:{port}").into_client_request().unwrap();
   request.headers_mut().insert("Origin",origin.parse().unwrap());
   assert!(tungstenite::connect(request).is_err(),"browser accepted: {origin}");
  }
  let (mut native,_)=tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
  native.close(None).unwrap();stop.store(true,Ordering::Relaxed);worker.join().unwrap();
 }
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
 #[test] fn html_runtime_registers_only_with_single_use_token(){
  use tungstenite::client::IntoClientRequest;
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let registry:crate::plugin_http::Inspectors=Default::default();let token="f".repeat(32);
  registry.lock().unwrap().insert(token.clone(),crate::plugin_http::RUNTIME_SESSION.into());
  registry.lock().unwrap().insert("a".repeat(32),"JSON.stringify({})".into());
  let (tx,commands)=mpsc::channel();let (messages,rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let r=registry.clone();
  let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,flag,Some(r),None,Some("http://127.0.0.1:12345".into())));
  let browser=||{let mut req=format!("ws://127.0.0.1:{port}").into_client_request().unwrap();req.headers_mut().insert("Origin","http://127.0.0.1:12345".parse().unwrap());tungstenite::connect(req).unwrap().0};
  // A PI session id is not a runtime token.
  let mut pi=browser();pi.send(tungstenite::Message::Text(json!({"event":"registerPlugin","uuid":"a".repeat(32)}).to_string().into())).unwrap();
  assert!(rx.recv_timeout(Duration::from_millis(400)).is_err(),"PI token registered as plugin");
  let mut page=browser();page.send(tungstenite::Message::Text(json!({"event":"registerPlugin","uuid":token}).to_string().into())).unwrap();
  let reg=rx.recv_timeout(Duration::from_secs(3)).unwrap();assert_eq!(reg["event"],"registerPlugin");assert!(reg.get("uuid").is_none(),"token leaked");
  assert!(!registry.lock().unwrap().contains_key(&token),"token must be single use");
  tx.send(json!({"event":"keyDown","context":"c"})).unwrap();
  assert_eq!(serde_json::from_str::<Value>(&page.read().unwrap().to_string()).unwrap()["event"],"keyDown");
  page.send(tungstenite::Message::Text(json!({"event":"setTitle","context":"c","payload":{"title":"x"}}).to_string().into())).unwrap();
  assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap()["event"],"setTitle");
  let mut replay=browser();replay.send(tungstenite::Message::Text(json!({"event":"registerPlugin","uuid":token}).to_string().into())).unwrap();
  assert!(rx.recv_timeout(Duration::from_millis(400)).is_err(),"token replay accepted");
  drop((pi,page,replay));drop(tx);stop.store(true,Ordering::Relaxed);worker.join().unwrap();
 }
 #[cfg(windows)] #[test] #[ignore="requires DECKFORGE_TEST_PI_FOLDER (HTML plugin) and interactive Windows WebView2"]
 fn html_runtime_plugin_registers(){
  let root=std::path::PathBuf::from(std::env::var("DECKFORGE_TEST_PI_FOLDER").expect("plugin folder"));
  let plugin=crate::plugins::parse(&root,&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
  assert!(matches!(plugin.runtime,Runtime::Html(_)));
  let host=Host::start(&plugin,&crate::plugin_runtime::Device::virtual_device()).unwrap();
  let until=Instant::now()+Duration::from_secs(20);let mut registered=false;
  while Instant::now()<until {if let Ok(msg)=host.incoming.recv_timeout(Duration::from_millis(200)){eprintln!("HTML_RUNTIME_EVENT {}",msg["event"].as_str().unwrap_or("?"));if msg["event"]=="registerPlugin"{registered=true;break}}}
  eprintln!("HTML_RUNTIME {} {}",plugin.uuid,if registered{"REGISTERED"}else{"NO_REGISTER"});
  assert!(registered,"HTML plugin runtime did not register");
  let action=plugin.actions.first().expect("action");
  let v=crate::plugin_runtime::Visible{context:"html-ctx".into(),plugin:plugin.uuid.clone(),action:action.uuid.clone(),index:0,settings:json!({}),state:0};
  host.send(crate::plugin_runtime::event("willAppear",&v,&crate::plugin_runtime::Device::virtual_device())).unwrap();
  let until=Instant::now()+Duration::from_secs(3);
  while Instant::now()<until {if let Ok(msg)=host.incoming.recv_timeout(Duration::from_millis(200)){eprintln!("HTML_RUNTIME_EVENT {}",msg["event"].as_str().unwrap_or("?"));}}
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
  let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,flag,None,trace,None));
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
  let trace=Some(Trace{file:std::fs::File::create(&path).unwrap(),entries:0});let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,flag,None,trace,None));
  let mut bad=std::net::TcpStream::connect(("127.0.0.1",port)).unwrap();bad.write_all(b"SECRET\r\n\r\n").unwrap();drop(bad);
  let (mut ws,_)=tungstenite::connect(format!("ws://127.0.0.1:{port}")).unwrap();
  ws.send(tungstenite::Message::Text("SECRET invalid json".into())).unwrap();
  ws.close(None).unwrap();thread::sleep(Duration::from_millis(150));stop.store(true,Ordering::Relaxed);worker.join().unwrap();
  let data=std::fs::read_to_string(path).unwrap();assert!(data.contains("handshake_error"),"{data}");assert!(data.contains("first_malformed_json"),"{data}");assert!(data.contains("socket_closed_pre_register"),"{data}");assert!(!data.contains("SECRET"));
 }
 #[test] fn trace_warns_without_register(){
  let d=tempfile::tempdir().unwrap();let path=d.path().join("trace.jsonl");let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let (_tx,commands)=mpsc::channel::<Value>();let (messages,_rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let trace=Some(Trace{file:std::fs::File::create(&path).unwrap(),entries:0});let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,flag,None,trace,None));
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
  let limit=std::time::Instant::now()+Duration::from_secs(30);
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
 #[test] fn browser_origin_is_exact_and_cannot_register_plugin(){
  use tungstenite::client::IntoClientRequest;
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
  let (tx,commands)=mpsc::channel();let (messages,rx)=mpsc::channel();let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
  let worker=thread::spawn(move||socket_loop_traced(listener,commands,messages,flag,None,None,Some("http://127.0.0.1:12345".into())));
  let mut evil=format!("ws://127.0.0.1:{port}").into_client_request().unwrap();
  evil.headers_mut().insert("Origin","https://example.invalid".parse().unwrap());
  assert!(tungstenite::connect(evil).is_err());
  let mut good=format!("ws://127.0.0.1:{port}").into_client_request().unwrap();
  good.headers_mut().insert("Origin","http://127.0.0.1:12345".parse().unwrap());
  let (mut browser,_)=tungstenite::connect(good).unwrap();
  browser.send(tungstenite::Message::Text(json!({"event":"registerPlugin","uuid":"malicious-browser"}).to_string().into())).unwrap();
  assert!(rx.recv_timeout(Duration::from_millis(400)).is_err(),"Browser spoofed registerPlugin");
  drop(browser);drop(tx);stop.store(true,Ordering::Relaxed);worker.join().unwrap();
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
  pi.send(tungstenite::Message::Text(json!({"event":"setGlobalSettings","context":id,"payload":{"theme":"dark"}}).to_string().into())).unwrap();
  let gmsg=rx.recv_timeout(Duration::from_secs(3)).unwrap();
  assert_eq!(gmsg["event"],"setGlobalSettings");assert_eq!(gmsg["__pi"],id);
  stop.store(true,Ordering::Relaxed);worker.join().unwrap();
 }
 #[cfg(windows)] #[test] #[ignore="requires explicit offline SDK sample and interactive Windows session"]
 fn property_inspector_offline_sample_connects_in_webview2(){
  let root=std::path::PathBuf::from(std::env::var("DECKFORGE_TEST_PI_FOLDER").expect("explicit offline sample folder"));
  let plugin=crate::plugins::parse(&root,&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
  let action=plugin.actions.iter().find(|a|a.inspector.is_some()).expect("action with local PI");
  let host=Host::start(&plugin,&crate::plugin_runtime::Device::virtual_device()).unwrap();
  let mut registered=false;
  for _ in 0..20 {if let Ok(msg)=host.incoming.recv_timeout(Duration::from_millis(500)){if msg["event"]=="registerPlugin"{registered=true;break}}}
  assert!(registered,"sample plugin did not register");
  let v=crate::plugin_runtime::Visible{context:"pi-ctx".into(),plugin:plugin.uuid.clone(),action:action.uuid.clone(),index:0,settings:json!({}),state:0};
  let device=crate::plugin_runtime::Device::virtual_device();
  host.send(crate::plugin_runtime::event("willAppear",&v,&device)).unwrap();
  let id=host.open_inspector(action.inspector.as_deref().unwrap(),json!({"action":action.uuid,"context":"pi-ctx","device":"deckforge-virtual","payload":{"settings":{"product":""},"coordinates":{"column":0,"row":0}}})).unwrap();
  let mut appeared=false;
  let mut early=Vec::new();
  for _ in 0..40 {if let Ok(msg)=host.incoming.recv_timeout(Duration::from_millis(500)){if msg["event"]=="propertyInspectorDidAppear" && msg["__pi"]==id {appeared=true;break} eprintln!("PI_PRE_EVENT {}",msg["event"].as_str().unwrap_or("?"));early.push(msg);}}
  assert!(appeared,"WebView2 PI did not register with its unique session");
  host.send(json!({"event":"propertyInspectorDidAppear","action":action.uuid,"context":"pi-ctx","device":"deckforge-virtual"})).unwrap();
  host.send(json!({"event":"didReceiveSettings","action":action.uuid,"context":"pi-ctx","device":"deckforge-virtual","payload":{"settings":{}},"__pi":id})).unwrap();
  let mut query=false;let mut answer=false;
  for _ in 0..40 {if let Some(mut msg)=if !early.is_empty(){Some(early.remove(0))}else{host.incoming.recv_timeout(Duration::from_millis(500)).ok()} {
   eprintln!("PI_EVENT {} from_pi={} context={} action={} payload_event={}",msg["event"].as_str().unwrap_or("?"),msg.get("__pi").is_some(),msg.get("context").is_some(),msg.get("action").is_some(),msg["payload"]["event"].as_str().unwrap_or("?"));
   match msg["event"].as_str().unwrap_or("") {
    "sendToPlugin" if msg["__pi"]==id => {query=true;msg.as_object_mut().unwrap().remove("__pi");msg["context"]=json!("pi-ctx");msg["action"]=json!(action.uuid);host.send(msg).unwrap();},
    "sendToPropertyInspector" => {answer=true;msg["__pi"]=json!(id);host.send(msg).unwrap();break;},
    _=>{}
   }
  }}
  assert!(query && answer,"PI data-source SDK did not exchange with its native Node plugin");
  if std::env::var_os("DECKFORGE_TEST_PI_CLICK").is_some(){
   let mut changed=false;
   for _ in 0..24 {if let Ok(msg)=host.incoming.recv_timeout(Duration::from_millis(500)) {
     if msg["event"]=="setSettings" && msg["__pi"]==id && msg["payload"]["product"].as_str().is_some_and(|s|s.starts_with("https://www.elgato.com/uk/en/p/stream-deck")){changed=true;break;}
   }}
   assert!(changed,"PI dropdown did not send setSettings to DeckForge");
  } else {std::thread::sleep(Duration::from_secs(12));}
 }
 /// Compatibility matrix for third-party PIs: open each PI and require the real SDK handshake
 /// (connectElgatoStreamDeckSocket -> registerPropertyInspector). Nothing is forwarded to the
 /// plugin backend and network access stays confined, so no plugin action is triggered.
 #[cfg(windows)] #[test] #[ignore="requires DECKFORGE_TEST_PI_FOLDER and interactive Windows WebView2"]
 fn external_property_inspector_registers(){
  let root=std::path::PathBuf::from(std::env::var("DECKFORGE_TEST_PI_FOLDER").expect("plugin folder"));
  let plugin=crate::plugins::parse(&root,&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
  let wanted=std::env::var("DECKFORGE_TEST_PI_ACTION").ok();
  let action=plugin.actions.iter().find(|a|(a.inspector.is_some()||plugin.inspector.is_some()) && wanted.as_ref().is_none_or(|w|w==&a.uuid)).expect("action with PI");
  let inspector=action.inspector.clone().or_else(||plugin.inspector.clone()).unwrap();
  let host=Host::start(&plugin,&crate::plugin_runtime::Device::virtual_device()).unwrap();
  let id=host.open_inspector(&inspector,json!({"action":action.uuid,"context":"pi-ctx","device":"deckforge-virtual","payload":{"settings":{},"coordinates":{"column":0,"row":0}}})).unwrap();
  let until=Instant::now()+Duration::from_secs(15);let mut appeared=false;
  while Instant::now()<until {if let Ok(msg)=host.incoming.recv_timeout(Duration::from_millis(200)){if msg["event"]=="propertyInspectorDidAppear" && msg["__pi"]==id{appeared=true;break}}}
  eprintln!("PI_MATRIX {} {} {}",plugin.uuid,action.uuid,if appeared{"REGISTERED"}else{"NO_HANDSHAKE"});
  assert!(appeared,"PI did not complete the SDK handshake");
  // Optional hold for documentation screenshots of the live window.
  let hold=std::env::var("DECKFORGE_TEST_PI_HOLD_MS").ok().and_then(|v|v.parse().ok()).unwrap_or(1500u64).min(60_000);
  std::thread::sleep(Duration::from_millis(hold));
 }
 #[cfg(windows)] #[test] #[ignore="requires explicit offline SDK sample and interactive Windows WebView2"]
 fn property_inspector_denies_popup_navigation_and_external_resources(){
  let source=std::path::PathBuf::from(std::env::var("DECKFORGE_TEST_PI_FOLDER").expect("local SDK sample path"));
  let dir=tempfile::tempdir().unwrap();let root=dir.path();
  for rel in ["manifest.json","bin/plugin.js","ui/open-product-page.html","ui/sdpi-components.js"] {
   let target=root.join(rel);std::fs::create_dir_all(target.parent().unwrap()).unwrap();std::fs::copy(source.join(rel),&target).unwrap();
  }
  let trap=TcpListener::bind("127.0.0.1:0").unwrap();trap.set_nonblocking(true).unwrap();let url=format!("http://127.0.0.1:{}/leak",trap.local_addr().unwrap().port());
  let page=root.join("ui/open-product-page.html");let source=std::fs::read_to_string(&page).unwrap();
  let mode=std::env::var("DECKFORGE_TEST_PI_ATTACK").unwrap_or_else(|_|"all".into());
  let js=match mode.as_str(){"popup"=>format!("window.open('{url}')"),"nav"=>format!("setTimeout(()=>location.assign('{url}'), 300)"),"fetch"=>format!("fetch('{url}').catch(()=>{{}})"),"image"=>format!("new Image().src='{url}'"),"css"=>format!("document.body.style.backgroundImage=\"url('{url}')\""),_=>format!("window.open('{url}');location.assign('{url}');fetch('{url}').catch(()=>{{}});new Image().src='{url}'")};
  let attack=format!("<script>{js}</script>");
  std::fs::write(&page,source.replace("</body>",&format!("{attack}</body>"))).unwrap();
  let plugin=crate::plugins::parse(root,&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
  let action=plugin.actions.iter().find(|a|a.inspector.is_some()).unwrap();
  let host=Host::start(&plugin,&crate::plugin_runtime::Device::virtual_device()).unwrap();
  let id=host.open_inspector(action.inspector.as_deref().unwrap(),json!({"action":action.uuid,"context":"pi-ctx","device":"deckforge-virtual","payload":{"settings":{},"coordinates":{"column":0,"row":0}}})).unwrap();
  let until=Instant::now()+Duration::from_secs(8);let mut appeared=false;
  while Instant::now()<until {if let Ok((mut incoming,_))=trap.accept(){let _=incoming.set_read_timeout(Some(Duration::from_millis(200)));let mut bytes=[0u8;1024];let n=std::io::Read::read(&mut incoming,&mut bytes).unwrap_or(0);panic!("Untrusted PI escaped via {mode}: {}",String::from_utf8_lossy(&bytes[..n]).lines().next().unwrap_or("no HTTP line"))}
   if let Ok(msg)=host.incoming.recv_timeout(Duration::from_millis(100)){if msg["event"]=="propertyInspectorDidAppear" && msg["__pi"]==id{appeared=true;}}
  }
  assert!(appeared,"PI fixture never loaded; negative test not exercised");
  assert!(trap.accept().is_err(),"Untrusted PI leaked after check");
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
  /// Strict matrix: drain willAppear until idle before pressing the virtual key.
  #[test] #[ignore="set DECKFORGE_TEST_PLUGIN_DIR and DECKFORGE_TEST_PLUGIN_ACTION; starts third-party code"]
  fn external_plugin_folder_reacts_after_key_press(){
   let var=|name:&str|std::env::var(name).unwrap_or_else(|_|panic!("set {name}"));
   let action=var("DECKFORGE_TEST_PLUGIN_ACTION");let imported=tempfile::tempdir().unwrap();
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
   let start=Instant::now();let mut last=start;let mut before=Vec::new();
   while start.elapsed()<Duration::from_secs(5){
    let remaining=(start+Duration::from_secs(5)).min(last+Duration::from_millis(1500)).saturating_duration_since(Instant::now());
    if remaining.is_zero(){break;}
    match host.incoming.recv_timeout(remaining){
     Ok(message)=>{
      let name=message["event"].as_str().unwrap_or("").to_owned();
      assert_ne!(name,"__hostError","{message}");
      if message["context"]=="matrix-context" || message.get("context").is_none(){before.push(name);}
      last=Instant::now();
     },
     Err(std::sync::mpsc::RecvTimeoutError::Timeout)=>break,
     Err(e)=>panic!("plugin channel closed before press: {e}"),
    }
   }
   for name in ["keyDown","keyUp"]{host.send(crate::plugin_runtime::event(name,&visible,&device)).unwrap();}
   let deadline=Instant::now()+Duration::from_secs(8);let mut after=Vec::new();
   while let Ok(message)=host.incoming.recv_timeout(deadline.saturating_duration_since(Instant::now())){
    let name=message["event"].as_str().unwrap_or("").to_owned();
    assert_ne!(name,"__hostError","{message}");
    if message["context"]=="matrix-context" || message.get("context").is_none(){after.push(name);}
   }
   println!("MATRIX {} before: {} after: {}",plugin.uuid,before.join(","),after.join(","));
   assert!(after.iter().any(|e|matches!(e.as_str(),"setTitle"|"setImage"|"setSettings"|"showOk")),"no post-press reaction: {after:?}");
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
