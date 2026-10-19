//! Installation confinée des plugins tiers : aucune exécution depuis les sources externes.
use std::{fs,io::{self,Read},path::{Path,PathBuf}};
use sha2::{Digest,Sha256};
use crate::plugins::{self,Plugin,Runtime};
const LIMIT:u64=100*1024*1024;
const FILE_LIMIT:usize=4096;
fn safe_relative(path:&Path)->bool {
 !path.as_os_str().is_empty() && path.components().all(|c|matches!(c,std::path::Component::Normal(_)))
 && !path.to_string_lossy().contains([':', '\0'])
}
fn validate(folder:&Path)->anyhow::Result<Plugin> {
 anyhow::ensure!(!fs::symlink_metadata(folder)?.file_type().is_symlink(),"Lien symbolique refusé");
 let manifest=folder.join("manifest.json");
 if let Ok(meta)=fs::symlink_metadata(&manifest){anyhow::ensure!(!meta.file_type().is_symlink(),"Lien symbolique refusé");}
 let data=fs::read(folder.join("manifest.json")).map_err(|e|anyhow::anyhow!("Manifeste absent ou illisible : {e}"))?;
 let plugin=plugins::parse(folder,&data).map_err(|e|anyhow::anyhow!("Manifeste invalide : {e}"))?;
 anyhow::ensure!(plugin.runtime!=Runtime::Protected,"Plugin protégé (ELGATO), import impossible");
 anyhow::ensure!(matches!(plugin.runtime,Runtime::Exe(_)|Runtime::Node(_)|Runtime::Html(_)),"Point d'entrée non pris en charge ou absent");
 anyhow::ensure!(!plugin.uuid.trim().is_empty(),"UUID vide");
 let value:serde_json::Value=serde_json::from_slice(data.strip_prefix(&[239,187,191]).unwrap_or(&data))?;
 for key in ["Icon","CodePath","CodePathWin","PropertyInspectorPath"] {
  if let Some(s)=value[key].as_str().filter(|s|!s.is_empty()) {anyhow::ensure!(!s.contains('\\') && safe_relative(Path::new(s)),"Chemin de manifeste invalide : {key}");}
 }
 if let Some(actions)=value["Actions"].as_array(){
  for action in actions {
   for key in ["Icon","PropertyInspectorPath"] {if let Some(s)=action[key].as_str().filter(|s|!s.is_empty()){anyhow::ensure!(!s.contains('\\') && safe_relative(Path::new(s)),"Chemin d'action invalide : {key}");}}
   if let Some(states)=action["States"].as_array(){for state in states {if let Some(s)=state["Image"].as_str().filter(|s|!s.is_empty()){anyhow::ensure!(!s.contains('\\') && safe_relative(Path::new(s)),"Chemin d'image invalide");}}}
  }
 }
 Ok(plugin)
}
fn check_uuid(dest:&Path,plugin:&Plugin,name:&str)->anyhow::Result<()> {
 for installed in inventory(dest){if installed.uuid==plugin.uuid && installed.folder.file_name().and_then(|s|s.to_str())!=Some(name){anyhow::bail!("Conflit UUID {} : déjà installé dans {}",plugin.uuid,installed.folder.display());}}
 Ok(())
}
#[derive(Default,Debug)] pub struct Report {pub copied:usize,pub existing:usize,pub protected:usize,pub built_in:usize,pub skipped:usize}
pub fn installed_root()->anyhow::Result<PathBuf>{Ok(std::env::current_exe()?.parent().ok_or_else(||anyhow::anyhow!("Chemin de l'application absent"))?.join("plugins"))}
pub fn sources()->Vec<PathBuf>{let mut roots=Vec::new();if let Some(p)=std::env::var_os("APPDATA"){roots.push(PathBuf::from(&p).join("HotSpot/StreamDock/plugins"));roots.push(PathBuf::from(p).join("Elgato/StreamDeck/Plugins"));};if let Some(p)=std::env::var_os("ProgramFiles(x86)"){let p=PathBuf::from(p).join("fifine Control Deck");roots.push(p.join("defaultData/defaultPlugins"));roots.push(p.join("plugins"));}roots}
pub fn inventory(root:&Path)->Vec<Plugin>{fs::read_dir(root).into_iter().flatten().flatten().take(512).filter_map(|e| {let folder=e.path();if folder.file_name()?.to_string_lossy().starts_with("__") || !folder.is_dir(){return None;}let bytes=fs::read(folder.join("manifest.json")).ok()?;plugins::parse(&folder,&bytes).ok()}).collect()}
fn allowed_name(p:&Plugin)->anyhow::Result<&str>{let name=p.folder.file_name().and_then(|s|s.to_str()).ok_or_else(||anyhow::anyhow!("Nom de dossier invalide"))?;anyhow::ensure!(!name.starts_with("__") && !name.is_empty() && Path::new(name).components().count()==1,"Nom non confiné");Ok(name)}
fn copy_tree(source:&Path,target:&Path,total:&mut u64,count:&mut usize,depth:usize)->anyhow::Result<()> {
 anyhow::ensure!(depth<12,"Arborescence trop profonde");fs::create_dir_all(target)?;
 for item in fs::read_dir(source)?.take(4097){let item=item?;let ty=item.file_type()?;anyhow::ensure!(!ty.is_symlink(),"Lien symbolique refusé");let dst=target.join(item.file_name());*count+=1;anyhow::ensure!(*count<=FILE_LIMIT,"Trop de fichiers");if ty.is_dir(){copy_tree(&item.path(),&dst,total,count,depth+1)?;}else if ty.is_file(){let len=item.metadata()?.len();*total=total.saturating_add(len);anyhow::ensure!(*total<=LIMIT,"Plugin trop volumineux");fs::copy(item.path(),dst)?;}else{anyhow::bail!("Type de fichier refusé");}}Ok(())
}
pub fn import_folder(folder:&Path,dest:&Path)->anyhow::Result<bool>{import_folder_with_hash(folder,dest,None)}
fn import_folder_with_hash(folder:&Path,dest:&Path,sha256:Option<&str>)->anyhow::Result<bool>{
 let plugin=validate(folder)?;let name=allowed_name(&plugin)?;
 fs::create_dir_all(dest)?;check_uuid(dest,&plugin,name)?;
 let final_path=dest.join(name);if final_path.exists(){return Ok(false);}
 let staging=tempfile::Builder::new().prefix(".import-").tempdir_in(dest)?;
 let copy=staging.path().join(name);let mut total=0;let mut count=0;
 copy_tree(folder,&copy,&mut total,&mut count,0)?;
 let verified=validate(&copy)?;anyhow::ensure!(verified.uuid==plugin.uuid,"UUID modifié pendant la copie");
 if let Some(hash)=sha256 {fs::write(copy.join(".deckforge-import.json"),serde_json::to_vec(&serde_json::json!({"schema_version":1,"origin":"local","archive_sha256":hash,"compatibility":"unverified","active":false}))?)?;}
 check_uuid(dest,&plugin,name)?;
 fs::rename(copy,final_path)?;Ok(true)
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)] pub enum Category { Available, BuiltIn, NonExecutable, Protected }
pub fn compatibility(plugin:&Plugin)->crate::plugin_catalog::Compatibility {
 use crate::plugin_catalog::Compatibility;
 match plugin.runtime {
  Runtime::Protected=>Compatibility::Protected,
  Runtime::Unsupported=>Compatibility::Unsupported,
  // Static analysis only: the host can start all three in isolated processes; real behaviour is never implied.
  Runtime::Node(_)|Runtime::Exe(_)|Runtime::Html(_)=>Compatibility::Compatible,
 }
}
pub fn category(plugin:&Plugin)->Category {
 match plugin.runtime {
  Runtime::Exe(_)|Runtime::Node(_)|Runtime::Html(_)=>Category::Available,
  Runtime::Protected=>Category::Protected,
  Runtime::Unsupported=>{
   let parent=plugin.folder.parent();
   let fifine=parent.and_then(Path::file_name).and_then(|n|n.to_str()).is_some_and(|n|n.eq_ignore_ascii_case("plugins"))
    && parent.and_then(Path::parent).and_then(Path::file_name).and_then(|n|n.to_str()).is_some_and(|n|n.eq_ignore_ascii_case("fifine Control Deck"));
   if fifine { Category::BuiltIn } else { Category::NonExecutable }
  }
 }
}
pub fn import_sources(dest:&Path)->Report{let mut report=Report::default();for root in sources(){for p in inventory(&root){match p.runtime {Runtime::Protected=>report.protected+=1,Runtime::Unsupported=>{if category(&p)==Category::BuiltIn {report.built_in+=1}else{report.skipped+=1}},_=>match import_folder(&p.folder,dest){Ok(true)=>report.copied+=1,Ok(false)=>report.existing+=1,Err(_)=>report.skipped+=1}}}}report}
pub fn import_archive(archive:&Path,dest:&Path)->anyhow::Result<bool>{let mut file=fs::File::open(archive)?;anyhow::ensure!(file.metadata()?.len()<=LIMIT,"Archive trop volumineuse");let mut hasher=Sha256::new();io::copy(&mut file,&mut hasher)?;let hash=format!("{:x}",hasher.finalize());let mut zip=zip::ZipArchive::new(fs::File::open(archive)?)?;anyhow::ensure!(zip.len()<=FILE_LIMIT,"Archive trop fournie");let staging=tempfile::tempdir()?;let mut total=0u64;
 for i in 0..zip.len(){let entry=zip.by_index(i)?;// Les anciens outils Elgato (DistributionTool Windows) écrivent des séparateurs « \\ » : on les
  // normalise, puis on refuse toujours racine, lecteur (« : »), NUL et composants « .. ».
  let raw=entry.name()?.replace('\\',"/");anyhow::ensure!(!raw.starts_with('/') && !raw.contains([':', '\0']),"Chemin absolu ou Windows refusé");let name=raw.split('/').filter(|s|!s.is_empty()).collect::<PathBuf>();anyhow::ensure!(name.components().next().is_some(),"Chemin vide dans l'archive");anyhow::ensure!(safe_relative(&name),"Chemin non confiné dans l'archive : {:?} ({:?})",name,name.components().collect::<Vec<_>>());anyhow::ensure!(!entry.is_symlink(),"Lien refusé");total=total.saturating_add(entry.size());anyhow::ensure!(total<=LIMIT,"Archive décompressée trop volumineuse");let path=staging.path().join(name);if entry.is_dir(){fs::create_dir_all(path)?;}else{fs::create_dir_all(path.parent().unwrap())?;let mut output=fs::File::create(path)?;let size=io::copy(&mut entry.take(LIMIT+1),&mut output)?;anyhow::ensure!(size<=LIMIT,"Fichier trop volumineux");}}
 let folders=fs::read_dir(staging.path())?.collect::<Result<Vec<_>,_>>()?;
 if folders.len()==1 && folders[0].file_type()?.is_dir(){return import_folder_with_hash(&folders[0].path(),dest,Some(&hash))}
 if staging.path().join("manifest.json").is_file(){let named=staging.path().join("archive.sdPlugin");fs::create_dir(&named)?;for item in fs::read_dir(staging.path())?{let item=item?;if item.path()!=named{fs::rename(item.path(),named.join(item.file_name()))?;}}return import_folder_with_hash(&named,dest,Some(&hash))}
 anyhow::bail!("Archive : un dossier de plugin attendu")
}
pub fn action_name(plugin:&Plugin,action:&crate::plugins::Action)->String{for lang in ["fr","en"]{for base in [plugin.folder.join("localization"),plugin.folder.join("Localization")]{let p=base.join(format!("{lang}.json"));if let Ok(data)=fs::read(p){if let Ok(v)=serde_json::from_slice::<serde_json::Value>(&data){if let Some(name)=v["Actions"][&action.uuid]["Name"].as_str().or_else(||v["Actions"][&action.uuid].as_str()){return name.to_owned()}}}}}action.name.clone()}
#[cfg(test)] mod tests{use super::*;
 fn fixture(d:&Path,code:&str){fs::write(d.join("manifest.json"),format!(r#"{{"UUID":"test.plugin","CodePath":"{code}","Actions":[{{"UUID":"test.action","Name":"Original"}}]}}"#)).unwrap();}
 #[test] fn import_is_confined_and_idempotent(){let d=tempfile::tempdir().unwrap();let source=d.path().join("sample.sdPlugin");fs::create_dir(&source).unwrap();fixture(&source,"app.exe");fs::write(source.join("app.exe"),b"fake").unwrap();let dst=d.path().join("installed");assert!(import_folder(&source,&dst).unwrap());assert!(!import_folder(&source,&dst).unwrap());assert!(dst.join("sample.sdPlugin/app.exe").is_file());}
 #[test] fn rejects_zip_slip_and_bombs(){use std::io::Write;let d=tempfile::tempdir().unwrap();let path=d.path().join("bad.streamDeckPlugin");{let mut w=zip::ZipWriter::new(fs::File::create(&path).unwrap());w.start_file("../escape",zip::write::SimpleFileOptions::default()).unwrap();w.write_all(b"bad").unwrap();w.finish().unwrap();}assert!(import_archive(&path,&d.path().join("out")).is_err());assert!(!d.path().join("escape").exists());}
 #[test] fn valid_archive_records_sha_without_profiles(){use std::io::Write;
  let d=tempfile::tempdir().unwrap();let archive=d.path().join("ok.streamDeckPlugin");
  {let mut z=zip::ZipWriter::new(fs::File::create(&archive).unwrap());for (name,bytes) in [("example.sdPlugin/manifest.json",br#"{"UUID":"fixture.unique","Name":"Fixture","CodePath":"main.js","Actions":[{"UUID":"fixture.key","Name":"Counter"}]}"#.as_slice()),("example.sdPlugin/main.js",b"// inert".as_slice())]{z.start_file(name,zip::write::SimpleFileOptions::default()).unwrap();z.write_all(bytes).unwrap();}z.finish().unwrap();}
  let dest=d.path().join("plugins");assert!(import_archive(&archive,&dest).unwrap());
  let metadata=fs::read_to_string(dest.join("example.sdPlugin/.deckforge-import.json")).unwrap();
  assert_eq!(serde_json::from_str::<serde_json::Value>(&metadata).unwrap()["archive_sha256"].as_str().unwrap().len(),64);
  assert_eq!(inventory(&dest)[0].actions[0].uuid,"fixture.key");assert!(!d.path().join("profiles").exists());
  assert!(!import_archive(&archive,&dest).unwrap());
 }
 #[test] fn hostile_archives_rejected(){use std::io::Write;
  for name in ["../escape", "/absolute", "C:/absolute", "..\\escape", "plugin.sdPlugin\\..\\..\\escape", "C:\\absolute", "\\absolute"]{
   let d=tempfile::tempdir().unwrap();let file=d.path().join("bad.streamDeckPlugin");
   let mut z=zip::ZipWriter::new(fs::File::create(&file).unwrap());z.start_file(name,zip::write::SimpleFileOptions::default()).unwrap();z.write_all(b"bad").unwrap();z.finish().unwrap();
   assert!(import_archive(&file,&d.path().join("installed")).is_err(),"{name}");
  }
  let d=tempfile::tempdir().unwrap();let file=d.path().join("too-many.streamDeckPlugin");
  let mut z=zip::ZipWriter::new(fs::File::create(&file).unwrap());for i in 0..=FILE_LIMIT{z.start_file(format!("{i}"),zip::write::SimpleFileOptions::default()).unwrap();z.write_all(b"x").unwrap();}z.finish().unwrap();assert!(import_archive(&file,&d.path().join("installed")).unwrap_err().to_string().contains("fournie"));
  let huge=d.path().join("large.streamDeckPlugin");let f=fs::File::create(&huge).unwrap();f.set_len(LIMIT+1).unwrap();assert!(import_archive(&huge,&d.path().join("installed")).unwrap_err().to_string().contains("volumineuse"));
 }
 #[test] fn windows_separators_from_legacy_distribution_tool_import(){use std::io::Write;
  let d=tempfile::tempdir().unwrap();let file=d.path().join("legacy.streamDeckPlugin");
  let mut z=zip::ZipWriter::new(fs::File::create(&file).unwrap());
  z.start_file("com.legacy.tool.sdPlugin\\manifest.json",zip::write::SimpleFileOptions::default()).unwrap();z.write_all(br#"{"UUID":"com.legacy.tool","CodePath":"tool.exe","Actions":[{"UUID":"com.legacy.tool.a"}]}"#).unwrap();
  z.start_file("com.legacy.tool.sdPlugin\\tool.exe",zip::write::SimpleFileOptions::default()).unwrap();z.write_all(b"MZ").unwrap();
  z.start_file("com.legacy.tool.sdPlugin\\images\\icon.png",zip::write::SimpleFileOptions::default()).unwrap();z.write_all(b"png").unwrap();
  z.finish().unwrap();let dest=d.path().join("installed");
  assert!(import_archive(&file,&dest).unwrap());
  let installed=dest.join("com.legacy.tool.sdPlugin");
  assert!(installed.join("tool.exe").is_file() && installed.join("images").join("icon.png").is_file());
 }
 #[test] fn invalid_manifest_protected_uuid_and_images(){
  let d=tempfile::tempdir().unwrap();let source=d.path().join("a.sdPlugin");fs::create_dir(&source).unwrap();fs::write(source.join("main.js"),b"x").unwrap();let dest=d.path().join("installed");
  fs::write(source.join("manifest.json"),b"{not json").unwrap();assert!(import_folder(&source,&dest).unwrap_err().to_string().contains("Manifeste invalide"));
  fs::write(source.join("manifest.json"),b"ELGATO\x01protected").unwrap();assert!(import_folder(&source,&dest).unwrap_err().to_string().contains("protégé"));
  fs::write(source.join("manifest.json"),br#"{"UUID":"same","CodePath":"main.js","Actions":[{"UUID":"a","States":[{"Image":"../secret.png"}]}]}"#).unwrap();assert!(import_folder(&source,&dest).unwrap_err().to_string().contains("image"));
  fs::write(source.join("manifest.json"),br#"{"UUID":"same","CodePath":"main.js"}"#).unwrap();assert!(import_folder(&source,&dest).unwrap());
  let other=d.path().join("b.sdPlugin");fs::create_dir(&other).unwrap();fs::write(other.join("main.js"),b"x").unwrap();fs::copy(source.join("manifest.json"),other.join("manifest.json")).unwrap();
  assert!(import_folder(&other,&dest).unwrap_err().to_string().contains("Conflit UUID"));
 }
 #[test] fn localization_prefers_french(){let d=tempfile::tempdir().unwrap();fixture(d.path(),"app.exe");fs::write(d.path().join("app.exe"),b"fake").unwrap();fs::create_dir(d.path().join("localization")).unwrap();fs::write(d.path().join("localization/fr.json"),br#"{"Actions":{"test.action":{"Name":"Bonjour"}}}"#).unwrap();let p=plugins::parse(d.path(),&fs::read(d.path().join("manifest.json")).unwrap()).unwrap();assert_eq!(action_name(&p,&p.actions[0]),"Bonjour");}
}

#[cfg(test)] mod category_tests {use super::*;
 #[test] fn sources_are_classified_by_runtime_and_origin(){
 let d=tempfile::tempdir().unwrap();let fifine=d.path().join("fifine Control Deck/plugins");let external=d.path().join("StreamDock/plugins");fs::create_dir_all(&fifine).unwrap();fs::create_dir_all(&external).unwrap();
 for (root,code,expected) in [(&fifine,"",Category::BuiltIn),(&external,"",Category::NonExecutable),(&external,"SignalRGB-Deck",Category::Available),(&external,"page.html",Category::Available),(&external,"main.js",Category::Available)] {
 let folder=root.join(format!("plugin-{code}"));fs::create_dir_all(&folder).unwrap();if !code.is_empty(){fs::write(folder.join(if code=="SignalRGB-Deck"{"SignalRGB-Deck.exe"}else{code}),b"x").unwrap();}
 let p=plugins::parse(&folder,format!(r#"{{"UUID":"x","CodePath":"{code}"}}"#).as_bytes()).unwrap();assert_eq!(category(&p),expected,"{code}");
 if code=="page.html" {assert!(matches!(compatibility(&p),crate::plugin_catalog::Compatibility::Compatible));}
 }
 }
}
