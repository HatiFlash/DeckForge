//! Vue de la bibliothèque consacrée aux plugins (aucun plugin externe n'est lancé ici).
use super::{App,DragItem};
use crate::{model::{Key,Step,PluginBinding},plugins::{Plugin,Runtime},plugin_import::{self,Category}};
use eframe::egui::{self,RichText};
impl App {
 pub(super) fn plugin_store(&mut self,ui:&mut egui::Ui,search:&str){
  ui.label(RichText::new("Découvrir / Installés / Importer").strong());
  ui.label(RichText::new("Catalogue local, sans réseau. Compatibilité non vérifiée ; installer ne lance rien. Lier une action à une touche peut ensuite lancer son plugin.").small());
  let catalog=crate::plugin_catalog::Catalog::offline();
  ui.label(format!("Découvrir · {} entrée(s) locales publiables",catalog.entries.len()));
  ui.separator();ui.label(RichText::new("Importer").strong());
  ui.horizontal_wrapped(|ui|{
   if ui.button("Importer depuis FIFINE / StreamDock / Elgato").clicked(){self.plugin_import_request(None);}
   if ui.button("Importer un fichier .streamDeckPlugin").clicked(){self.plugin_import_dialog(true);}
   if ui.button("Importer un dossier .sdPlugin").clicked(){self.plugin_import_dialog(false);}
  });
  ui.label(RichText::new(&self.notice).small());
  let root=plugin_import::installed_root().ok();
  egui::ScrollArea::vertical().id_salt("plugin_store").show(ui,|ui|{
   ui.separator();ui.label(RichText::new("Installés dans DeckForge").strong());
   if let Some(root)=&root{for plugin in plugin_import::inventory(root){
    if !format!("{} {}",plugin.display_name(),plugin.uuid).to_lowercase().contains(search) && !plugin.actions.iter().any(|a|a.name.to_lowercase().contains(search)){continue;}
    let ty=match plugin.runtime{Runtime::Exe(_)|Runtime::Node(_)=>"programme",Runtime::Html(_)=>"web",Runtime::Protected=>"protégé",Runtime::Unsupported=>"indisponible"};
    ui.group(|ui|{ui.label(RichText::new(format!("{} · v{}",plugin.display_name(),plugin.version)).strong());
     ui.label(format!("Auteur : {} · Origine : import local",if plugin.author.trim().is_empty(){"inconnu"}else{&plugin.author}));
     ui.label(format!("Runtime : {ty} · Windows · Compatibilité : non vérifiée · Installé / inactif si aucune touche liée"));
     ui.label("Droits de redistribution : non évalués ; processus tiers = droits utilisateur, aucune sandbox.");
     ui.label(format!("{} actions · {}",plugin.actions.len(),self.plugin_errors.get(&plugin.uuid).map(String::as_str).unwrap_or("arrêté")));
     let report_path=plugin.folder.join("validation-report.json");
     let report=std::fs::read(&report_path).ok().and_then(|data|serde_json::from_slice::<crate::plugin_validation::Report>(&data).ok())
         .filter(|r|r.plugin==plugin.uuid && r.version==plugin.version);
     if let Some(report)=report {
      for result in report.actions {
       let verdict=match result.verdict {crate::plugin_validation::Verdict::Passed=>"Réussi",crate::plugin_validation::Verdict::Failed=>"Échoué",crate::plugin_validation::Verdict::Unverified=>"Non vérifié"};
       ui.label(format!("{} : {verdict} · {} · {} · date Unix {}",result.action,result.reason,result.mode,result.tested_at_unix));
      }
     } else { ui.label("Validation : non vérifiée (aucun rapport pour cette version)."); }
     if ui.button("Tester le plugin (simulation uniquement)").clicked(){self.plugin_test_request(plugin.clone());}
     ui.label("Actions disponibles (placer sur une touche peut lancer le plugin) :");
     for action in &plugin.actions{
      let name=plugin_import::action_name(&plugin,action);
      let mut key=Key{label:name.clone(),..Key::default()};
      key.image=action.states.first().and_then(|p|p.as_ref()).or(action.icon.as_ref()).map(|p|p.to_string_lossy().into_owned()).unwrap_or_default();
      key.plugin=Some(PluginBinding{plugin:plugin.uuid.clone(),action:action.uuid.clone(),settings:serde_json::json!({}),state:0});
      let response=ui.button(format!("+ {name}"));
      self.drag_sources.push((response.rect.intersect(ui.clip_rect()),DragItem::Bind(key.clone())));
      if response.clicked(){self.assign(DragItem::Bind(key),self.selected);}
      response.on_hover_text("Placer sur la touche sélectionnée, ou glisser sur une touche");
     }
    });
   }}
   ui.separator();
   ui.label(RichText::new("Découvrir · sources locales FIFINE / StreamDock / Elgato").strong());
  if search.is_empty() || "volumecontroller audio volume muet peripherique".contains(&search.to_lowercase()) {
   ui.group(|ui| {
    ui.label(RichText::new("Audio natif DeckForge — sans plugin ni fenêtre externe").strong());
    ui.label("Volume + / - et muet contrôlent le volume Windows par touches multimédias. Le basculement utilise les périphériques Core Audio détectés ci-dessous.");
    ui.horizontal_wrapped(|ui| {
     for (label,command) in [("+ Volume +","volume_up"),("+ Volume -","volume_down"),("+ Muet / rétablir","mute")] {
      let key=Key{label:label.trim_start_matches("+ ").into(),steps:vec![Step::Media{command:command.into()}],..Key::default()};
      let response=ui.button(label);
      self.drag_sources.push((response.rect.intersect(ui.clip_rect()),DragItem::Bind(key.clone())));
      if response.clicked(){self.assign(DragItem::Bind(key),self.selected);}
     }
    });
    if !self.audio.error.is_empty(){ui.label(format!("Audio : {}",self.audio.error));}
    for (input,label) in [(false,"Sorties"),(true,"Entrées / micros")] {
     let endpoints:Vec<_>=self.audio.devices.iter().filter(|d|d.input==input).collect();
     ui.label(format!("{} : {}",label,if endpoints.is_empty(){"aucun périphérique actif".into()}else{endpoints.iter().map(|d|d.name.as_str()).collect::<Vec<_>>().join(" ; ")}));
     let key=native_audio_switch(&self.audio,input);
     let response=ui.add_enabled(key.is_some(),egui::Button::new(format!("+ Basculer deux {}",label.to_lowercase())));
     if let Some(key)=key {
      self.drag_sources.push((response.rect.intersect(ui.clip_rect()),DragItem::Bind(key.clone())));
      if response.clicked(){self.assign(DragItem::Bind(key),self.selected);}
     }
    }
    ui.label(RichText::new("La liste est actualisée automatiquement par Core Audio. Choisis ensuite les deux périphériques dans l’inspecteur ; aucune configuration de plugin existante n’est convertie.").small());
   });
  }
   let mut available=Vec::new();let mut built_in=Vec::new();let mut protected=Vec::new();let mut unavailable=Vec::new();
   for source in plugin_import::sources(){for plugin in plugin_import::inventory(&source){
    if !format!("{} {}",plugin.display_name(),plugin.uuid).to_lowercase().contains(search){continue;}
    match plugin_import::category(&plugin){Category::Available=>available.push(plugin),Category::BuiltIn=>built_in.push(plugin),Category::Protected=>protected.push(plugin),Category::NonExecutable=>unavailable.push(plugin)}
   }}
   if !available.is_empty(){ui.horizontal(|ui|{
    ui.heading(format!("Disponibles ({})",available.len()));
    if ui.button(format!("Importer tout ({})",available.len())).clicked(){self.plugin_import_request(None);}
   });}
   for plugin in &available {ui.group(|ui|{
    ui.horizontal(|ui|{
     plugin_icon(ui,plugin);
     ui.vertical(|ui|{
      ui.label(RichText::new(plugin.display_name()).strong());
      ui.label(format!("{} · v{}",if plugin.author.trim().is_empty(){"Auteur inconnu"}else{&plugin.author},plugin.version));
      ui.label(format!("{} action(s) · Runtime : {} · Windows",plugin.actions.len(),plugin_type(&plugin.runtime)));
      ui.label(format!("Origine : locale · Analyse statique : {:?} · Compatibilité : non vérifiée · Redistribution : non évaluée",plugin_import::compatibility(plugin)));
      for action in &plugin.actions {ui.label(format!("Action : {}",plugin_import::action_name(plugin,action)));}
     });
     if ui.button("Importer").clicked(){self.plugin_import_request(Some(plugin.folder.clone()));}
    });
   });}
   ui.collapsing(format!("Intégrés à FIFINE, à recréer nativement ({})",built_in.len()),|ui|{
    for plugin in &built_in{ui.label(plugin.display_name());}
   });
   ui.collapsing(format!("Non exécutables ({})",unavailable.len()),|ui|{
    for plugin in &unavailable{ui.label(plugin.display_name());}
   });
   ui.collapsing(format!("Protégés ({})",protected.len()),|ui|{
    for plugin in &protected{ui.label(plugin.display_name());}
   });
  });
 }
 fn plugin_test_request(&mut self,plugin:Plugin){
  self.start_dialog(move||{
   let report=crate::plugin_validation::simulate(&plugin);
   let result=serde_json::to_vec_pretty(&report).map_err(anyhow::Error::from)
      .and_then(|bytes|std::fs::write(plugin.folder.join("validation-report.json"),bytes).map_err(anyhow::Error::from));
   Some(Box::new(move |app:&mut App|match result{
    Ok(())=>{app.notice="Rapport de simulation enregistré (aucun essai matériel).".into();},
    Err(e)=>app.fail(format!("Rapport de plugin non enregistré : {e}")),
   }))
  });
 }
 fn plugin_import_request(&mut self,source:Option<std::path::PathBuf>){
  self.start_dialog(move||{let result=plugin_import::installed_root().and_then(|root|match source{Some(folder)=>plugin_import::import_folder(&folder,&root).map(|copied|format!("Plugin : {}",if copied{"copié"}else{"déjà présent"})),None=>{let report=plugin_import::import_sources(&root);Ok(format!("Import : {} copiés, {} déjà présents, {} protégés, {} intégrés FIFINE, {} ignorés",report.copied,report.existing,report.protected,report.built_in,report.skipped))}});
   Some(Box::new(move |app:&mut App|match result{Ok(text)=>{app.notice=text.clone();app.log(text);},Err(e)=>app.fail(format!("Import plugin impossible : {e}"))}))
  });
 }
 fn plugin_import_dialog(&mut self,archive:bool){self.start_dialog(move||{
  let path=if archive{egui_file_dialog_archive()}else{rfd::FileDialog::new().pick_folder()}?;
  let result=plugin_import::installed_root().and_then(|root|if archive{plugin_import::import_archive(&path,&root)}else{plugin_import::import_folder(&path,&root)});
  Some(Box::new(move |app:&mut App|match result{Ok(copied)=>{app.notice=if copied{"Plugin importé"}else{"Plugin déjà présent"}.into();},Err(e)=>app.fail(format!("Import plugin impossible : {e}"))}))
 });}
}
fn native_audio_switch(audio:&crate::audio::Snapshot,input:bool)->Option<Key>{
 let mut endpoints=audio.devices.iter().filter(|d|d.input==input);
 let first=endpoints.next()?;let second=endpoints.next()?;
 Some(Key{label:if input{"Basculer les micros"}else{"Basculer les sorties"}.into(),steps:vec![Step::AudioSwitch{first:first.id.clone(),second:second.id.clone(),input,first_label:first.name.clone(),second_label:second.name.clone(),first_icon:String::new(),second_icon:String::new()}],..Key::default()})
}
fn plugin_type(runtime:&Runtime)->&'static str{match runtime{Runtime::Exe(_)|Runtime::Node(_)=>"Programme",Runtime::Html(_)=>"Web",_=>"Non exécutable"}}
fn plugin_icon(ui:&mut egui::Ui,plugin:&Plugin){
 if let Some(texture)=plugin.icon.as_ref().and_then(|p|super::icon_ui::texture(ui.ctx(),&p.to_string_lossy())){
  ui.image((texture.id(),egui::vec2(42.0,42.0)));
 }
}
fn egui_file_dialog_archive()->Option<std::path::PathBuf>{rfd::FileDialog::new().add_filter("Plugin Stream Deck", &["streamDeckPlugin","zip"]).pick_file()}
#[cfg(test)] mod tests {use super::*;
 #[test] fn native_audio_requires_two_real_endpoints_per_direction(){
  let mut audio=crate::audio::Snapshot::default();
  assert!(native_audio_switch(&audio,false).is_none());
  audio.devices.push(crate::audio::Endpoint{id:"real-output-a".into(),name:"Sortie A".into(),input:false});
  audio.devices.push(crate::audio::Endpoint{id:"real-input".into(),name:"Micro".into(),input:true});
  assert!(native_audio_switch(&audio,false).is_none());
  assert!(native_audio_switch(&audio,true).is_none());
  audio.devices.push(crate::audio::Endpoint{id:"real-output-b".into(),name:"Sortie B".into(),input:false});
  let key=native_audio_switch(&audio,false).unwrap();
  assert!(key.plugin.is_none());
  assert!(matches!(&key.steps[0],Step::AudioSwitch{first,second,input:false,..} if first=="real-output-a" && second=="real-output-b"));
  assert!(native_audio_switch(&audio,true).is_none());
 }
 #[test] fn binding_for_imported_action_roundtrips(){let k=Key{plugin:Some(PluginBinding{plugin:"fixture.plugin".into(),action:"fixture.action".into(),settings:serde_json::json!({}),state:0}),..Key::default()};let value=serde_json::to_value(&k).unwrap();assert_eq!(serde_json::from_value::<Key>(value).unwrap().plugin.unwrap().action,"fixture.action");}
}
