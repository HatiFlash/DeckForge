//! Cycle de vie des actions Stream Deck visibles (aucun I/O dans le verrou App).
use std::{collections::HashMap, sync::mpsc::{self, Receiver, Sender}, thread, time::{Duration, Instant}};
use serde_json::{json, Value};
use crate::{model::Config, plugin_host::Host, plugins::Plugin};

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
fn event(name:&str,v:&Visible)->Value {
    json!({"event":name,"action":v.action,"context":v.context,"device":"deckforge-d6","payload":{"settings":v.settings,"coordinates":{"column":v.index%5,"row":v.index/5},"state":v.state,"isInMultiAction":false,"controller":"Keypad"}})
}
enum Request { Sync(HashMap<String,Visible>), Key(String,bool), Send(String,Value), Open(String), Stop }
pub enum Reply { Message(String,Value), Error(String,String) }
pub struct Manager { tx:Sender<Request>,pub rx:Receiver<Reply>,visible:HashMap<String,Visible>,pub catalog:Vec<Plugin> }
impl Manager {
    pub fn new(plugins:Vec<Plugin>)->Self {
        let (tx,requests)=mpsc::channel();let (reply,rx)=mpsc::channel();
        let catalog=plugins.clone();thread::spawn(move||run(plugins,requests,reply));
        Self{tx,rx,visible:HashMap::new(),catalog}
    }
    pub fn sync(&mut self, config:&Config) {
        let next=visible(config);
        if next!=self.visible {self.visible=next.clone();let _=self.tx.send(Request::Sync(next));}
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
    let mut hosts:HashMap<String,Host>=HashMap::new();let mut current=HashMap::new();
    let mut inspectors:HashMap<String,Visible>=HashMap::new();
    let mut idle:HashMap<String,Instant>=HashMap::new();
    let mut restarts:HashMap<String,Vec<Instant>>=HashMap::new();
    loop {
        match requests.recv_timeout(Duration::from_millis(50)) {
            Ok(Request::Stop)|Err(mpsc::RecvTimeoutError::Disconnected)=>break,
            Ok(Request::Sync(next))=>{
                let (gone,added)=diff(&current,&next);
                for v in gone {if let Some(host)=hosts.get(&v.plugin){let _=host.send(event("willDisappear",&v));}}
                for v in added {if !hosts.contains_key(&v.plugin){
                    if let Some(p)=plugins.iter().find(|p|p.uuid==v.plugin){match Host::start(p){Ok(h)=>{hosts.insert(v.plugin.clone(),h);},Err(e)=>{let _=reply.send(Reply::Error(v.plugin.clone(),e.to_string()));}}}
                    else{let _=reply.send(Reply::Error(v.plugin.clone(),"Plugin installé introuvable".into()));}
                }
                if let Some(host)=hosts.get(&v.plugin){let _=host.send(event("willAppear",&v));}}
                current=next;inspectors.retain(|_,v|current.get(&v.context)==Some(v));
                for plugin in hosts.keys(){if current.values().any(|v|&v.plugin==plugin){idle.remove(plugin);}else{idle.entry(plugin.clone()).or_insert_with(Instant::now);}}
            }
            Ok(Request::Open(context))=>{if let Some(v)=current.get(&context){if let (Some(p),Some(host))=(plugins.iter().find(|p|p.uuid==v.plugin),hosts.get(&v.plugin)){
                let path=p.actions.iter().find(|a|a.uuid==v.action).and_then(|a|a.inspector.as_ref()).or(p.inspector.as_ref());
                if let Some(path)=path {let action_info=json!({"action":v.action,"context":v.context,"device":"deckforge-d6","payload":{"settings":v.settings,"coordinates":{"column":v.index%5,"row":v.index/5}}});
                    match host.open_inspector(path,action_info){Ok(id)=>{inspectors.insert(id,v.clone());},Err(e)=>{let _=reply.send(Reply::Error(v.plugin.clone(),format!("Inspecteur : {e}")));}}
                }
            }}}
            Ok(Request::Key(context,down))=>{if let Some(v)=current.get(&context){if let Some(host)=hosts.get(&v.plugin){let _=host.send(event(if down{"keyDown"}else{"keyUp"},v));}}}
            Ok(Request::Send(plugin,value))=>{if let Some(host)=hosts.get(&plugin){let _=host.send(value);}}
            Err(mpsc::RecvTimeoutError::Timeout)=>{}
        }
        for (plugin,host) in &hosts {while let Ok(mut value)=host.incoming.try_recv(){
            let name=value["event"].as_str().unwrap_or("").to_owned();
            if name=="__hostError" {let _=reply.send(Reply::Error(plugin.clone(),value["reason"].as_str().unwrap_or("Message invalide").into()));continue;}
            if !matches!(name.as_str(),"registerPlugin"|"logMessage"|"setTitle"|"setImage"|"setState"|"showOk"|"showAlert"|"setSettings"|"getSettings"|"setGlobalSettings"|"getGlobalSettings"|"sendToPlugin"|"sendToPropertyInspector"|"propertyInspectorDidAppear"|"propertyInspectorDidDisappear"|"openUrl") {
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
                    let _=host.send(json!({"event":"didReceiveSettings","action":v.action,"context":v.context,"device":"deckforge-d6","__pi":id,
                        "payload":{"settings":v.settings,"coordinates":{"column":v.index%5,"row":v.index/5},"isInMultiAction":false}}));
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
                match Host::start(p) {
                    Ok(host)=>{
                        for v in current.values().filter(|v|v.plugin==id) {let _=host.send(event("willAppear",v));}
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
    #[test]fn event_coordinates(){let v=Visible{context:"ctx".into(),plugin:"p".into(),action:"a".into(),index:7,settings:json!({"x":1}),state:2};let message=event("keyDown",&v);assert_eq!(message["payload"]["coordinates"],json!({"column":2,"row":1}));assert_eq!(message["context"],"ctx");}
}
