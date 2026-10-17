//! Inventaire des plugins Stream Deck, sans exécuter de code tiers.
use std::path::{Path,PathBuf};
use serde_json::Value;
#[derive(Clone,Debug,PartialEq,Eq)] pub enum Runtime { Exe(PathBuf), Node(PathBuf), Html(PathBuf), Protected, Unsupported }
#[derive(Clone,Debug)] pub struct Action { pub uuid:String, pub name:String, pub icon:Option<PathBuf>, pub states:Vec<Option<PathBuf>>, pub multi_action:bool }
#[derive(Clone,Debug)] pub struct Plugin { pub uuid:String, pub name:String, pub version:String, pub folder:PathBuf, pub runtime:Runtime, pub actions:Vec<Action> }
fn safe(folder:&Path,name:&str)->Option<PathBuf>{ let p=Path::new(name); if name.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c,std::path::Component::Normal(_))) { None } else { Some(folder.join(p)) } }
fn icon(folder:&Path,name:&str)->Option<PathBuf>{ let p=safe(folder,name)?; ["", "@2x.png", ".png", "@2x.svg", ".svg"].iter().map(|suffix|PathBuf::from(format!("{}{}",p.display(),suffix))).find(|p|p.is_file()) }
pub fn parse(folder:&Path,bytes:&[u8])->anyhow::Result<Plugin>{
 anyhow::ensure!(bytes.len()<=2_000_000,"Manifeste trop volumineux");
 if bytes.starts_with(b"ELGATO\x01") {return Ok(Plugin{uuid:folder.file_name().unwrap_or_default().to_string_lossy().into_owned(),name:"Protégé par Elgato, non pris en charge".into(),version:String::new(),folder:folder.into(),runtime:Runtime::Protected,actions:vec![]});}
 let v:Value=serde_json::from_slice(bytes.strip_prefix(&[239,187,191]).unwrap_or(bytes))?;
 let field=|k:&str|v[k].as_str().unwrap_or("").to_owned();
 let runtime=if let Some(path)=safe(folder,&field("CodePath")) {match path.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(){"html"|"htm" if path.is_file()=>Runtime::Html(path),"js" if path.is_file()=>Runtime::Node(path),"exe" if path.is_file()=>Runtime::Exe(path),"" if path.with_extension("exe").is_file()=>Runtime::Exe(path.with_extension("exe")),_=>Runtime::Unsupported}}else{Runtime::Unsupported};
 let actions=v["Actions"].as_array().into_iter().flatten().take(256).filter_map(|a|{Some(Action{uuid:a["UUID"].as_str()?.into(),name:a["Name"].as_str().unwrap_or("").into(),icon:a["Icon"].as_str().and_then(|s|icon(folder,s)),states:a["States"].as_array().into_iter().flatten().map(|s|s["Image"].as_str().and_then(|n|icon(folder,n))).collect(),multi_action:a["SupportedInMultiActions"].as_bool().unwrap_or(false)})}).collect();
 Ok(Plugin{uuid:field("UUID"),name:field("Name"),version:field("Version"),folder:folder.into(),runtime,actions})
}
pub fn discover()->Vec<Plugin>{let mut roots=vec![];if let Some(p)=std::env::var_os("APPDATA"){roots.push(PathBuf::from(p).join("Elgato/StreamDeck/Plugins"));}if let Ok(exe)=std::env::current_exe(){if let Some(p)=exe.parent(){roots.push(p.join("plugins"));}}roots.into_iter().flat_map(|r|std::fs::read_dir(r).into_iter().flatten().flatten().take(256)).filter_map(|e|{let p=e.path();parse(&p,&std::fs::read(p.join("manifest.json")).ok()?).ok()}).collect()}
#[cfg(test)] mod tests {use super::*;
 #[test] fn bom_and_extensionless_exe(){let d=tempfile::tempdir().unwrap();std::fs::write(d.path().join("a.exe"),b"").unwrap();std::fs::write(d.path().join("icon@2x.png"),b"x").unwrap();let p=parse(d.path(),&[&[239,187,191][..],&br#"{"UUID":"x","CodePath":"a","Actions":[{"UUID":"x.a","Icon":"icon"}]}"#[..]].concat()).unwrap();assert!(matches!(p.runtime,Runtime::Exe(_)));assert!(p.actions[0].icon.is_some());}
 #[test] fn encrypted_and_traversal(){let d=tempfile::tempdir().unwrap();assert_eq!(parse(d.path(),b"ELGATO\x01payload").unwrap().runtime,Runtime::Protected);assert_eq!(parse(d.path(),br#"{"CodePath":"../escape.exe"}"#).unwrap().runtime,Runtime::Unsupported);}
}
