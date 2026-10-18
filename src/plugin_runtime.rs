//! Cycle de vie des actions Stream Deck visibles (aucun I/O dans le verrou App).
use std::{collections::HashMap, sync::mpsc::{self, Receiver, Sender}, thread, time::{Duration, Instant}};
use serde_json::{json, Value};
use crate::{model::Config, plugin_host::Host, plugins::Plugin};

/// Identité SDK d'un appareil physique, ou de l'appareil virtuel d'essai.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device { pub id:String, pub name:String, pub columns:usize, pub rows:usize, pub kind:u8 }
impl Device {
    pub fn virtual_device()->Self { let model=&crate::devices::MODELS[0];Self{id:"deckforge-virtual".into(),name:format!("{} (virtuel)",model.name),columns:model.cols as usize,rows:model.rows as usize,kind:0} }
    pub fn from_grid(name:String,columns:usize,rows:usize)->Self {
        let kind=crate::devices::MODELS.iter().find(|m|m.name==name && m.cols as usize==columns && m.rows as usize==rows).map_or(0,|m| match m.keys {6=>1,32=>2,_=>0});
        Self{id:format!("deckforge-{}x{}",columns,rows),name,columns,rows,kind}
    }
    pub fn info(&self)->Value {json!({"id":self.id,"name":self.name,"size":{"columns":self.columns,"rows":self.rows},"type":self.kind})}
    fn event(&self,name:&str)->Value {json!({"event":name,"device":self.id,"deviceInfo":{"name":self.name,"size":{"columns":self.columns,"rows":self.rows},"type":self.kind}})}
}
#[derive(Clone, Debug, PartialEq)]
pub struct Visible {
    pub context: String,
    pub plugin: String,
    pub action: String,
    pub index: usize,
    pub settings: Value,
    pub state: u8,
}
pub fn visible(config: &Config) -> HashMap<String, Visible> {
    config.keys().iter().enumerate().filter_map(|(index,key)| {
        let binding=key.plugin.as_ref()?;
        let context=format!("deckforge-{}-{}-{}",config.active,config.page,index);
        Some((context.clone(),Visible{context,plugin:binding.plugin.clone(),action:binding.action.clone(),index,settings:binding.settings.clone(),state:binding.state}))
    }).collect()
}
pub fn diff(old:&HashMap<String,Visible>,new:&HashMap<String,Visible>) -> (Vec<Visible>,Vec<Visible>) {
    let gone=old.iter().filter(|(id,v)|new.get(*id)!=Some(*v)).map(|(_,v)|v.clone()).collect();
    let added=new.iter().filter(|(id,v)|old.get(*id)!=Some(*v)).map(|(_,v)|v.clone()).collect();
    (gone,added)
}
pub(crate) fn event(name:&str,v:&Visible,device:&Device)->Value {
    let mut payload=json!({"settings":v.settings,"coordinates":{"column":v.index%device.columns,"row":v.index/device.columns},"state":v.state,"isInMultiAction":false,"resources":{}});
    if matches!(name,"willAppear"|"willDisappear"|"didReceiveSettings"|"keyDown"|"keyUp") {payload["controller"]=json!("Keypad");}
    json!({"event":name,"action":v.action,"context":v.context,"device":device.id,"payload":payload})
}
pub(crate) fn title_event(v:&Visible,device:&Device,title:&str)->Value {
    json!({"event":"titleParametersDidChange","action":v.action,"context":v.context,"device":device.id,
        "payload":{"controller":"Keypad","settings":v.settings,"resources":{},"title":title,"titleParameters":{"fontFamily":"","fontSize":14,"fontStyle":"Regular","fontUnderline":false,"showTitle":true,"titleAlignment":"middle","titleColor":"#ffffff"},"state":v.state,"coordinates":{"column":v.index%device.columns,"row":v.index/device.columns}}})
}
enum Request { Sync(HashMap<String,Visible>), Device(Device), Key(String,bool), Title(String,String), Send(String,Value), Open(String), Stop }
pub enum Reply { Message(String,Value), Error(String,String) }
pub struct Manager { tx:Sender<Request>,pub rx:Receiver<Reply>,visible:HashMap<String,Visible>,titles:HashMap<String,String>,pub catalog:Vec<Plugin> }
impl Manager {
    pub fn new(plugins:Vec<Plugin>)->Self {
        let (tx,requests)=mpsc::channel();let (reply,rx)=mpsc::channel();
        let catalog=plugins.clone();thread::spawn(move||run(plugins,requests,reply));
        Self{tx,rx,visible:HashMap::new(),titles:HashMap::new(),catalog}
    }
    pub fn device(&self,device:Device){let _=self.tx.send(Request::Device(device));}
    pub fn sync(&mut self, config:&Config) {
        let next=visible(config);
        if next!=self.visible {self.visible=next.clone();let _=self.tx.send(Request::Sync(next.clone()));}
        let titles=next.iter().map(|(id,v)|(id.clone(),config.keys()[v.index].label.clone())).collect::<HashMap<_,_>>();
        for (id,title) in &titles {if self.titles.get(id)!=Some(title){let _=self.tx.send(Request::Title(id.clone(),title.clone()));}}
        self.titles=titles;
    }
    pub fn key(&self,index:usize,down:bool) {
        if let Some(v)=self.visible.values().find(|v|v.index==index){let _=self.tx.send(Request::Key(v.context.clone(),down));}
    }
    pub fn inspector_path(&self,index:usize)->Option<String>{let v=self.visible.values().find(|v|v.index==index)?;let p=self.catalog.iter().find(|p|p.uuid==v.plugin)?;p.actions.iter().find(|a|a.uuid==v.action).and_then(|a|a.inspector.clone()).or_else(||p.inspector.clone())}
    pub fn open_inspector(&self,index:usize){if let Some(v)=self.visible.values().find(|v|v.index==index){let _=self.tx.send(Request::Open(v.context.clone()));}}
    pub fn send(&self,plugin:&str,value:Value){let _=self.tx.send(Request::Send(plugin.to_owned(),value));}
    pub fn visible(&self)->&HashMap<String,Visible>{&self.visible}
}
impl Drop for Manager {fn drop(&mut self){let _=self.tx.send(Request::Stop);}}
fn run(plugins:Vec<Plugin>,requests:Receiver<Request>,reply:Sender<Reply>){
    let mut hosts:HashMap<String,Host>=HashMap::new();let mut current:HashMap<String,Visible>=HashMap::new();
    let mut device=Device::virtual_device();
    let mut inspectors:HashMap<String,Visible>=HashMap::new();
    let mut idle:HashMap<String,Instant>=HashMap::new();
    let mut restarts:HashMap<String,Vec<Instant>>=HashMap::new();
    loop {
        match requests.recv_timeout(Duration::from_millis(50)) {
            Ok(Request::Stop)|Err(mpsc::RecvTimeoutError::Disconnected)=>break,
            Ok(Request::Title(context,title))=>{if let Some(v)=current.get(&context){if let Some(host)=hosts.get(&v.plugin){let _=host.send(title_event(v,&device,&title));}}}
            Ok(Request::Device(next))=>{if next!=device {for host in hosts.values(){let _=host.send(device.event("deviceDidDisconnect"));let _=host.send(next.event("deviceDidConnect"));}device=next;}}
            Ok(Request::Sync(next))=>{
                let (gone,added)=diff(&current,&next);
                for v in gone {if let Some(host)=hosts.get(&v.plugin){let _=host.send(event("willDisappear",&v,&device));}}
                for v in added {if !hosts.contains_key(&v.plugin){
                    if let Some(p)=plugins.iter().find(|p|p.uuid==v.plugin){match Host::start(p,&device){Ok(h)=>{let _=h.send(device.event("deviceDidConnect"));hosts.insert(v.plugin.clone(),h);},Err(e)=>{let _=reply.send(Reply::Error(v.plugin.clone(),e.to_string()));}}}
                    else{let _=reply.send(Reply::Error(v.plugin.clone(),"Plugin installé introuvable".into()));}
                }
                if let Some(host)=hosts.get(&v.plugin){let _=host.send(event("willAppear",&v,&device));}}
                current=next;inspectors.retain(|_,v|current.get(&v.context)==Some(v));
                for plugin in hosts.keys(){if current.values().any(|v|&v.plugin==plugin){idle.remove(plugin);}else{idle.entry(plugin.clone()).or_insert_with(Instant::now);}}
            }
            Ok(Request::Open(context))=>{if let Some(v)=current.get(&context){if let (Some(p),Some(host))=(plugins.iter().find(|p|p.uuid==v.plugin),hosts.get(&v.plugin)){
                let path=p.actions.iter().find(|a|a.uuid==v.action).and_then(|a|a.inspector.as_ref()).or(p.inspector.as_ref());
                if let Some(path)=path {let action_info=json!({"action":v.action,"context":v.context,"device":device.id,"payload":{"settings":v.settings,"coordinates":{"column":v.index%device.columns,"row":v.index/device.columns}}});
                    match host.open_inspector(path,action_info){Ok(id)=>{inspectors.insert(id,v.clone());},Err(e)=>{let _=reply.send(Reply::Error(v.plugin.clone(),format!("Inspecteur : {e}")));}}
                }
            }}}
            Ok(Request::Key(context,down))=>{if let Some(v)=current.get(&context){if let Some(host)=hosts.get(&v.plugin){let _=host.send(event(if down{"keyDown"}else{"keyUp"},v,&device));}}}
            Ok(Request::Send(plugin,value))=>{if let Some(host)=hosts.get(&plugin){let _=host.send(value);}}
            Err(mpsc::RecvTimeoutError::Timeout)=>{}
        }
        for (plugin,host) in &hosts {while let Ok(mut value)=host.incoming.try_recv(){
            let name=value["event"].as_str().unwrap_or("").to_owned();
            if name=="__hostError" {let _=reply.send(Reply::Error(plugin.clone(),value["reason"].as_str().unwrap_or("Message invalide").into()));continue;}
            if !matches!(name.as_str(),"registerPlugin"|"logMessage"|"setTitle"|"setImage"|"setState"|"showOk"|"showAlert"|"setSettings"|"getSettings"|"setGlobalSettings"|"getGlobalSettings"|"sendToPlugin"|"sendToPropertyInspector"|"switchToProfile"|"propertyInspectorDidAppear"|"propertyInspectorDidDisappear"|"openUrl") {
                let _=reply.send(Reply::Error(plugin.clone(),format!("Evenement plugin non pris en charge : {name}")));continue;
            }
            if matches!(name.as_str(),"setTitle"|"setImage"|"setState"|"showOk"|"showAlert"|"setSettings"|"getSettings")
                && !current.get(value["context"].as_str().unwrap_or("")).is_some_and(|v:&Visible|v.plugin==*plugin) {
                let _=reply.send(Reply::Error(plugin.clone(),"Contexte étranger ou absent".into()));continue;
            }
            if let Some(id)=value["__pi"].as_str().map(str::to_owned){
                let Some(v)=inspectors.get(&id).filter(|v|v.plugin==*plugin).cloned() else {continue};
                if name=="propertyInspectorDidDisappear" {inspectors.remove(&id);}
                if name=="propertyInspectorDidAppear" {
                    // Comme Stream Dock/Stream Deck : le PI reçoit tout de suite les réglages de sa touche.
                    let _=host.send(json!({"event":"didReceiveSettings","action":v.action,"context":v.context,"device":device.id,"__pi":id,
                        "payload":event("didReceiveSettings",&v,&device)["payload"]}));
                }
                if !matches!(name.as_str(),"setSettings"|"getSettings"|"setGlobalSettings"|"getGlobalSettings"|"sendToPlugin"|"propertyInspectorDidAppear"|"propertyInspectorDidDisappear"){continue;}
                value["context"]=json!(v.context);value["action"]=json!(v.action);
                if name=="sendToPlugin" || name.starts_with("propertyInspectorDid"){
                    let mut command=value.clone();if let Some(obj)=command.as_object_mut(){obj.remove("__pi");}
                    let _=host.send(command);continue;
                }
            } else if name=="sendToPropertyInspector" {
                if let Some(ctx)=value["context"].as_str(){for (id,v) in &inspectors {if v.context==ctx && v.plugin==*plugin {let mut message=value.clone();message["__pi"]=json!(id);let _=host.send(message);}}}continue;
            }
            if name=="switchToProfile" {let _=reply.send(Reply::Error(plugin.clone(),"switchToProfile non pris en charge : profil non vérifié".into()));continue;}
            let _=reply.send(Reply::Message(plugin.clone(),value));
        }}
        let exited=hosts.iter_mut().filter_map(|(id,host)|{
            if current.values().any(|v|v.plugin==*id) && host.exited() {Some(id.clone())}else{None}
        }).collect::<Vec<_>>();
        for id in exited {
            let _=reply.send(Reply::Error(id.clone(),"Plugin termine inopinement; redemarrage tente".into()));
            hosts.remove(&id);inspectors.retain(|_,v|v.plugin!=id); // Drop kills any remaining descendants before restart.
            let attempts=restarts.entry(id.clone()).or_default();
            attempts.retain(|when|when.elapsed()<Duration::from_secs(300));
            if attempts.len()>=3 {
                let _=reply.send(Reply::Error(id.clone(),"Erreur : plugin arrêté après 3 redémarrages en 5 minutes".into()));
                continue;
            }
            attempts.push(Instant::now());
            if let Some(p)=plugins.iter().find(|p|p.uuid==id) {
                match Host::start(p,&device) {
                    Ok(host)=>{
                        let _=host.send(device.event("deviceDidConnect"));
                        for v in current.values().filter(|v|v.plugin==id) {let _=host.send(event("willAppear",v,&device));}
                        hosts.insert(id.clone(),host);
                    },
                    Err(e)=>{let _=reply.send(Reply::Error(id.clone(),format!("Erreur : redémarrage impossible : {e}")));}
                }
            }
        }
        hosts.retain(|id,_|!idle.get(id).is_some_and(|t|t.elapsed()>=Duration::from_secs(30)));
        idle.retain(|id,_|hosts.contains_key(id));
    }
    // Host::drop termine tout l'arbre de chaque processus.
}
#[cfg(test)]mod tests {use super::*;use crate::model::{Config,PluginBinding};
    #[test]fn page_diff(){let mut c=Config::default();c.keys_mut()[0].plugin=Some(PluginBinding{plugin:"fixture".into(),action:"fixture.a".into(),settings:json!({}),state:0});let old=visible(&c);c.profiles[c.active].pages.push(crate::model::Page::new("other".into(),"Other".into(),false));c.page="other".into();let next=visible(&c);let (gone,added)=diff(&old,&next);assert_eq!(gone.len(),1);assert!(added.is_empty());}
    #[test]fn sdk_device_and_payload_contract(){
        let d=Device::from_grid("Elgato Stream Deck XL".into(),8,4);
        let v=Visible{context:"x".into(),plugin:"p".into(),action:"a".into(),index:17,settings:json!({"n":1}),state:1};
        let connect=d.event("deviceDidConnect");
        assert_eq!(connect["deviceInfo"]["size"],json!({"columns":8,"rows":4}));
        assert_eq!(connect["device"],d.id);
        for name in ["willAppear","willDisappear","didReceiveSettings","keyDown","keyUp"] {
            let msg=event(name,&v,&d);assert_eq!(msg["device"],connect["device"]);
            assert_eq!(msg["payload"]["coordinates"],json!({"column":1,"row":2}));
            assert_eq!(msg["payload"]["settings"],json!({"n":1}));
            assert_eq!(msg["payload"]["resources"],json!({}));
            assert_eq!(msg["payload"]["isInMultiAction"],false);
            assert_eq!(msg["payload"]["state"],1);
        }
        for name in ["willAppear","willDisappear","didReceiveSettings","keyDown","keyUp"] { assert_eq!(event(name,&v,&d)["payload"]["controller"],"Keypad"); }
        assert_eq!(Device::virtual_device().info()["size"]["columns"],5);
    }
    #[test]fn configured_title_event(){
        let v=Visible{context:"ctx".into(),plugin:"p".into(),action:"a".into(),index:7,settings:json!({}),state:1};
        let msg=title_event(&v,&Device::virtual_device(),"Titre configuré");
        assert_eq!(msg["event"],"titleParametersDidChange");
        assert_eq!(msg["payload"]["title"],"Titre configuré");
        assert_eq!(msg["payload"]["coordinates"],json!({"column":2,"row":1}));
        assert_eq!(msg["payload"]["controller"],"Keypad");
        assert_eq!(msg["payload"]["settings"],json!({}));
        assert_eq!(msg["payload"]["resources"],json!({}));
        assert_eq!(msg["payload"]["state"],1);
        assert!(msg["payload"]["titleParameters"].is_object());
    }
    #[test]fn event_coordinates(){let v=Visible{context:"ctx".into(),plugin:"p".into(),action:"a".into(),index:7,settings:json!({"x":1}),state:2};let message=event("keyDown",&v,&Device::virtual_device());assert_eq!(message["payload"]["coordinates"],json!({"column":2,"row":1}));assert_eq!(message["context"],"ctx");}
}
