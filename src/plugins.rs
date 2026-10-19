//! Inventaire des plugins Stream Deck, sans exécuter de code tiers.
use std::path::{Path,PathBuf};
use serde_json::Value;
#[derive(Clone,Debug,PartialEq,Eq)] pub enum Runtime { Exe(PathBuf), Node(PathBuf), Html(PathBuf), Protected, Unsupported }
#[derive(Clone,Debug)] pub struct Action { pub uuid:String, pub name:String, pub icon:Option<PathBuf>, pub states:Vec<Option<PathBuf>>, pub titles:Vec<Option<String>>, pub multi_action:bool, pub inspector:Option<String> }
#[derive(Clone,Debug)] pub struct Plugin { pub uuid:String, pub name:String, pub version:String, pub author:String, pub icon:Option<PathBuf>, pub folder:PathBuf, pub runtime:Runtime, pub actions:Vec<Action>, pub inspector:Option<String>, pub node_version:Option<String>, pub prerequisite:Option<String>, pub applications:Vec<String> }
fn safe(folder:&Path,name:&str)->Option<PathBuf>{ let p=Path::new(name); if name.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c,std::path::Component::Normal(_))) { None } else { Some(folder.join(p)) } }
fn icon(folder:&Path,name:&str)->Option<PathBuf>{ let p=safe(folder,name)?; ["", "@2x.png", ".png", "@2x.svg", ".svg"].iter().map(|suffix|PathBuf::from(format!("{}{}",p.display(),suffix))).find(|p|p.is_file() && p.canonicalize().ok().is_some_and(|real|real.starts_with(folder.canonicalize().unwrap_or_else(|_|folder.to_path_buf())))) }
/// Contraintes du manifeste évaluées avant tout lancement de code tiers.
fn windows_prerequisite(manifest:&Value)->Option<String>{
 let systems=manifest["OS"].as_array()?;
 let Some(windows)=systems.iter().find(|s|s["Platform"].as_str().is_some_and(|p|p.eq_ignore_ascii_case("windows"))) else{return Some("Plateforme Windows non déclarée dans OS".into())};
 let minimum=windows["MinimumVersion"].as_str().unwrap_or("");
 if minimum.is_empty(){return None;}
 let current=std::env::var("DECKFORGE_PLATFORM_VERSION").ok().or_else(||{
  let output=std::process::Command::new("reg").args(["query",r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion","/v","CurrentBuildNumber"]).output().ok()?;
  let text=String::from_utf8(output.stdout).ok()?;
  let build=text.split_whitespace().last()?.parse::<u32>().ok()?;
  Some(if build>=22000 {"11.0"}else{"10.0"}.into())
 });
 let Some(current)=current else{return Some(format!("Version Windows non vérifiable (minimum : {minimum})"))};
 fn parts(v:&str)->Option<Vec<u32>>{v.split('.').map(str::parse::<u32>).collect::<Result<Vec<_>,_>>().ok()}
 match (parts(&current),parts(minimum)) {
  (Some(mut have),Some(mut need))=>{have.resize(have.len().max(need.len()),0);need.resize(have.len(),0);if have<need {Some(format!("Windows {minimum} minimum requis (version détectée : {current})"))}else{None}},
  _=>Some(format!("Version Windows minimale non vérifiable : {minimum}")),
 }
}
pub fn parse(folder:&Path,bytes:&[u8])->anyhow::Result<Plugin>{
 anyhow::ensure!(bytes.len()<=2_000_000,"Manifeste trop volumineux");
 if bytes.starts_with(b"ELGATO\x01") {return Ok(Plugin{uuid:folder.file_name().unwrap_or_default().to_string_lossy().into_owned(),name:"Protégé par Elgato, non pris en charge".into(),version:String::new(),author:String::new(),icon:None,folder:folder.into(),runtime:Runtime::Protected,actions:vec![],inspector:None,node_version:None,prerequisite:Some("Manifeste protégé".into()),applications:vec![]});}
 let v:Value=serde_json::from_slice(bytes.strip_prefix(&[239,187,191]).unwrap_or(bytes))?;
 let field=|k:&str|v[k].as_str().unwrap_or("").to_owned();
 let code={let w=field("CodePathWin");if w.trim().is_empty(){field("CodePath")}else{w}};
    let runtime=if let Some(path)=safe(folder,&code) {match path.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(){"html"|"htm" if path.is_file()=>Runtime::Html(path),"js" if path.is_file()=>Runtime::Node(path),"exe" if path.is_file()=>Runtime::Exe(path),"" if path.with_extension("exe").is_file()=>Runtime::Exe(path.with_extension("exe")),_=>Runtime::Unsupported}}else{Runtime::Unsupported};
 let prerequisite=windows_prerequisite(&v);
 let node_version=v["Nodejs"]["Version"].as_str().map(str::to_owned);
 // ApplicationsToMonitor : noms d'exécutables Windows dont le plugin attend applicationDidLaunch/Terminate.
 let applications=v["ApplicationsToMonitor"]["windows"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|s|!s.trim().is_empty()&&s.len()<=260).take(64).map(str::to_owned).collect();
 let actions=v["Actions"].as_array().into_iter().flatten().take(256).filter_map(|a|{Some(Action{uuid:a["UUID"].as_str()?.into(),name:a["Name"].as_str().unwrap_or("").into(),icon:a["Icon"].as_str().and_then(|s|icon(folder,s)),states:a["States"].as_array().into_iter().flatten().map(|s|s["Image"].as_str().and_then(|n|icon(folder,n))).collect(),titles:a["States"].as_array().into_iter().flatten().map(|s|s["Title"].as_str().map(str::to_owned)).collect(),multi_action:a["SupportedInMultiActions"].as_bool().unwrap_or(false),inspector:a["PropertyInspectorPath"].as_str().filter(|s|safe(folder,s).is_some_and(|p|p.is_file())).map(str::to_owned)})}).collect();
 let uuid={let u=field("UUID");if u.trim().is_empty(){folder.file_name().map(|n|n.to_string_lossy().trim_end_matches(".sdPlugin").to_string()).unwrap_or_default()}else{u}};
    Ok(Plugin{uuid,name:field("Name"),version:field("Version"),author:field("Author"),icon:icon(folder,&field("Icon")),folder:folder.into(),runtime,actions,inspector:v["PropertyInspectorPath"].as_str().filter(|s|safe(folder,s).is_some_and(|p|p.is_file())).map(str::to_owned),node_version,prerequisite,applications})
}
pub fn discover()->Vec<Plugin>{let mut roots=vec![];if let Some(p)=std::env::var_os("APPDATA"){roots.push(PathBuf::from(p).join("Elgato/StreamDeck/Plugins"));}if let Ok(exe)=std::env::current_exe(){if let Some(p)=exe.parent(){roots.push(p.join("plugins"));}}roots.into_iter().flat_map(|r|std::fs::read_dir(r).into_iter().flatten().flatten().take(256)).filter_map(|e|{let p=e.path();parse(&p,&std::fs::read(p.join("manifest.json")).ok()?).ok()}).collect()}
#[cfg(test)] mod tests {use super::*;
 #[test] fn applications_to_monitor_windows_only(){let d=tempfile::tempdir().unwrap();std::fs::write(d.path().join("p.js"),b"").unwrap();
  let p=parse(d.path(),br#"{"UUID":"a.b","CodePath":"p.js","ApplicationsToMonitor":{"mac":["com.obs"],"windows":["obs64.exe",""]}}"#).unwrap();
  assert_eq!(p.applications,vec!["obs64.exe".to_string()]);
  assert!(parse(d.path(),br#"{"UUID":"a.b","CodePath":"p.js"}"#).unwrap().applications.is_empty());}
 #[test] fn runtime_manifest_constraints(){let d=tempfile::tempdir().unwrap();std::fs::write(d.path().join("plugin.js"),b"").unwrap();std::fs::write(d.path().join("native.exe"),b"").unwrap();
 let p=parse(d.path(),br#"{"CodePath":"plugin.js","CodePathWin":"native.exe","Nodejs":{"Version":"20"},"OS":[{"Platform":"mac"}]}"#).unwrap();
 assert!(matches!(p.runtime,Runtime::Exe(_)));assert_eq!(p.node_version.as_deref(),Some("20"));assert!(p.prerequisite.as_deref().unwrap().contains("Windows"));
 let p=parse(d.path(),br#"{"CodePath":"plugin.js","OS":[{"Platform":"windows","MinimumVersion":"99.0"}]}"#).unwrap();assert!(matches!(p.runtime,Runtime::Node(_)));assert!(p.prerequisite.is_some());
 }
 #[test] fn property_inspector_path_checked(){let d=tempfile::tempdir().unwrap();std::fs::write(d.path().join("pi.html"),"ok").unwrap();let p=parse(d.path(),br#"{"PropertyInspectorPath":"pi.html","Actions":[{"UUID":"a","PropertyInspectorPath":"../bad.html"},{"UUID":"b","PropertyInspectorPath":"pi.html"}]}"#).unwrap();assert_eq!(p.inspector.as_deref(),Some("pi.html"));assert_eq!(p.actions[0].inspector,None);assert_eq!(p.actions[1].inspector.as_deref(),Some("pi.html"));}
 #[test] fn missing_uuid_uses_folder_name(){let d=tempfile::tempdir().unwrap();let f=d.path().join("com.krabs.windowsgizmos.sdPlugin");std::fs::create_dir(&f).unwrap();std::fs::write(f.join("a.exe"),b"").unwrap();let p=parse(&f,br#"{"Name":"G","CodePathWin":"a.exe","Actions":[]}"#).unwrap();assert_eq!(p.uuid,"com.krabs.windowsgizmos");assert!(matches!(p.runtime,Runtime::Exe(_)));}
#[test] fn bom_and_extensionless_exe(){let d=tempfile::tempdir().unwrap();std::fs::write(d.path().join("a.exe"),b"").unwrap();std::fs::write(d.path().join("icon@2x.png"),b"x").unwrap();let p=parse(d.path(),&[&[239,187,191][..],&br#"{"UUID":"x","CodePath":"a","Actions":[{"UUID":"x.a","Icon":"icon"}]}"#[..]].concat()).unwrap();assert!(matches!(p.runtime,Runtime::Exe(_)));assert!(p.actions[0].icon.is_some());}
 #[test] fn state_image_and_title(){let d=tempfile::tempdir().unwrap();std::fs::write(d.path().join("icon@2x.svg"),b"<svg/>").unwrap();let p=parse(d.path(),br#"{"Actions":[{"UUID":"a","Icon":"icon","States":[{"Image":"icon","Title":"Titre"}]}]}"#).unwrap();assert!(p.actions[0].states[0].as_ref().unwrap().ends_with("icon@2x.svg"));assert_eq!(p.actions[0].titles[0].as_deref(),Some("Titre"));}
 #[test] fn manifest_icon_renders_on_device_and_preview(){
 let d=tempfile::tempdir().unwrap();
 let mut png=image::RgbaImage::new(16,16);for pixel in png.pixels_mut(){*pixel=image::Rgba([250,40,30,255]);}
 png.save(d.path().join("red.png")).unwrap();
 let p=parse(d.path(),br#"{"Actions":[{"UUID":"a","Icon":"red"}]}"#).unwrap();
 let mut key=crate::model::Key::default();key.image=p.actions[0].icon.as_ref().unwrap().to_string_lossy().into_owned();
 let rendered=crate::key_render::for_plugin(&key,72,&crate::key_render::font().unwrap(),None,None).unwrap();
 let device=image::load_from_memory(&rendered).unwrap().into_rgb8();
 let preview=crate::key_render::preview(&key,72).unwrap();
 assert!(device.pixels().any(|pixel|pixel[0]>150 && pixel[1]<100));
 assert!(preview.pixels().any(|pixel|pixel[0]>150 && pixel[1]<100));
 }
 #[test] fn encrypted_and_traversal(){let d=tempfile::tempdir().unwrap();assert_eq!(parse(d.path(),b"ELGATO\x01payload").unwrap().runtime,Runtime::Protected);assert_eq!(parse(d.path(),br#"{"CodePath":"../escape.exe"}"#).unwrap().runtime,Runtime::Unsupported);}
}

impl Plugin { pub fn display_name(&self)->&str { if self.name.trim().is_empty(){&self.uuid}else{self.name.trim()} } }
#[cfg(test)] mod presentation_tests { use super::*;
 #[test] fn blank_name_falls_back_to_uuid_and_manifest_icon(){let d=tempfile::tempdir().unwrap();std::fs::write(d.path().join("symbol@2x.png"),b"x").unwrap();let p=parse(d.path(),br#"{"UUID":"com.elgato.tutorial","Name":" ","Icon":"symbol"}"#).unwrap();assert_eq!(p.display_name(),"com.elgato.tutorial");assert!(p.icon.unwrap().ends_with("symbol@2x.png"));}
}
