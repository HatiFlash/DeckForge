//! Vue de la bibliothèque consacrée aux plugins (aucun plugin externe n'est lancé ici).
use super::{App,DragItem};
use crate::{model::{Key,Step,PluginBinding},plugins::{Plugin,Runtime},plugin_import::{self,Category}};
use eframe::egui::{self,RichText};
use crate::iconset::{self,Ico};
use crate::plugin_validation;
impl App {
 pub(super) fn plugin_store(&mut self,ui:&mut egui::Ui,search:&str){
  // Keep the entire tab inside one bounded viewport, including import controls and results.
  let height=ui.available_height().max(80.0);
  let content_width=ui.available_width();
  egui::ScrollArea::vertical().id_salt("plugin_store").auto_shrink([false,false]).max_height(height).show(ui,|ui|{
   // A ScrollArea may expose an unbounded child width; constrain paragraphs and cards to its viewport.
   ui.set_max_width(content_width);
  iconset::heading(ui,Ico::Plugin,RichText::new("Plugins · importer").strong());
  ui.add(egui::Label::new(RichText::new("Catalogue local, hors ligne. Importer ne lance rien.").small()).wrap());
  let catalog=crate::plugin_catalog::Catalog::offline();
  ui.label(format!("Découvrir · {} entrée(s) locales publiables",catalog.entries.len()));
  ui.separator();iconset::heading(ui,Ico::Install,RichText::new("Importer").strong());
  ui.horizontal_wrapped(|ui|{
   if ui.button("Importer depuis FIFINE / StreamDock / Elgato").clicked(){self.plugin_import_request(None);}
   if ui.button("Importer un fichier .streamDeckPlugin").clicked(){self.plugin_import_dialog(true);}
   if ui.button("Importer un dossier .sdPlugin").clicked(){self.plugin_import_dialog(false);}
   if ui.button("Actualiser la liste").on_hover_text("Relire les dossiers de plugins").clicked(){self.plugin_inventory=None;}
  });
  ui.label(RichText::new(&self.notice).small());
  let inventory=self.plugin_inventory();
   ui.separator();iconset::heading(ui,Ico::Plug,RichText::new("Installés dans DeckForge").strong());
   {for plugin in inventory.installed.iter(){
    if !format!("{} {}",plugin.display_name(),plugin.uuid).to_lowercase().contains(search) && !plugin.actions.iter().any(|a|a.name.to_lowercase().contains(search)){continue;}
    let ty=match plugin.runtime{Runtime::Exe(_)|Runtime::Node(_)=>"programme",Runtime::Html(_)=>"web",Runtime::Protected=>"protégé",Runtime::Unsupported=>"indisponible"};
    ui.group(|ui|{ui.label(RichText::new(format!("{} · v{}",plugin.display_name(),plugin.version)).strong());
     ui.label(format!("Auteur : {} · Origine : import local",if plugin.author.trim().is_empty(){"inconnu"}else{&plugin.author}));
     ui.label(format!("Runtime : {ty} · Windows · Compatibilité : non vérifiée · Installé / inactif si aucune touche liée"));
     if let Some(reason)=&plugin.prerequisite {ui.colored_label(egui::Color32::YELLOW,format!("Prérequis manquant : {reason}"));}
     if let Some(version)=&plugin.node_version {ui.label(format!("Node.js demandé : {version}"));}
     ui.label("Droits de redistribution : non évalués ; processus tiers = droits utilisateur, aucune sandbox.");
     ui.label(format!("{} actions · {}",plugin.actions.len(),self.plugin_errors.get(&plugin.uuid).map(String::as_str).unwrap_or("arrêté")));
     let unsupported=crate::plugin_host::unsupported_events(&plugin.uuid);
     ui.label(format!("Événements non pris en charge : {}",if unsupported.is_empty(){"aucun observé (ou trace désactivée)".into()}else{unsupported.join(", " )}));
     let cache_key=format!("{}-{}",plugin.uuid,plugin.version);
     let report=self.plugin_report_cache.entry(cache_key).or_insert_with(||plugin_validation::report_path(&plugin).and_then(|p|std::fs::read(p).ok()).and_then(|bytes|serde_json::from_slice::<plugin_validation::Report>(&bytes).ok()).filter(|r|r.plugin==plugin.uuid && r.version==plugin.version));
     if let Some(report)=report {
      for result in &report.actions {
       let verdict=match result.verdict {crate::plugin_validation::Verdict::Passed=>"Réussi",crate::plugin_validation::Verdict::Failed=>"Échoué",crate::plugin_validation::Verdict::Unverified=>"Non vérifié"};
       ui.label(format!("{} : {verdict} · {} · {} · {} UTC",result.action,result.reason,result.mode,report_date(result.tested_at_unix)));
      }
     } else { ui.label("Validation : non vérifiée (aucun rapport pour cette version)."); }
     if ui.add(iconset::button(ui.ctx(),Ico::Play,"Tester le plugin (simulation uniquement)")).clicked(){self.plugin_test_request(plugin.clone(),false);}
     let oracle=self.plugin_oracle_cache.entry(format!("{}-{}",plugin.uuid,plugin.version)).or_insert_with(||plugin_validation::load_oracle(&plugin).ok().flatten());
     if oracle.as_ref().is_some_and(|o|o.actions.iter().any(|a|a.real_effects)) && ui.button("Essai réel…").clicked(){self.plugin_real_confirmation=Some(plugin.clone());}
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
   iconset::heading(ui,Ico::Store,RichText::new("Plugins disponibles sur ce PC").strong());
  if search.is_empty() || "volumecontroller audio volume muet peripherique".contains(&search.to_lowercase()) {
   ui.collapsing("Audio natif · sans plugin",|ui| ui.group(|ui| {
    ui.set_max_width((content_width-24.0).max(120.0));
    iconset::heading(ui,Ico::Headset,RichText::new("Audio natif DeckForge").strong());
    ui.add(egui::Label::new("Volume + / - et muet contrôlent le volume Windows par touches multimédias. Le basculement utilise les périphériques Core Audio détectés ci-dessous.").wrap());
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
    ui.add(egui::Label::new(RichText::new("La liste est actualisée automatiquement par Core Audio. Choisis ensuite les deux périphériques dans l’inspecteur ; aucune configuration de plugin existante n’est convertie.").small()).wrap());
   }));
  }
   let matches=|p:&&Plugin|format!("{} {}",p.display_name(),p.uuid).to_lowercase().contains(search);
   let available:Vec<&Plugin>=inventory.available.iter().filter(matches).collect();
   let built_in:Vec<&Plugin>=inventory.built_in.iter().filter(matches).collect();
   let protected:Vec<&Plugin>=inventory.protected.iter().filter(matches).collect();
   let unavailable:Vec<&Plugin>=inventory.unavailable.iter().filter(matches).collect();
   if !available.is_empty(){ui.horizontal(|ui|{
    ui.heading(format!("Disponibles ({})",available.len()));
    if ui.button(format!("Importer tout ({})",available.len())).clicked(){self.plugin_import_request(None);}
   });}
   for plugin in &available {ui.push_id(&plugin.uuid,|ui|ui.group(|ui|{
    ui.set_min_width((content_width-18.0).max(140.0));
    // A horizontal row must have a bounded text column; otherwise long names expand past the side panel.
    ui.horizontal(|ui|{
     plugin_icon(ui,plugin);
     let name_width=(content_width-if plugin.icon.is_some(){80.0}else{32.0}).max(110.0);
     ui.vertical(|ui|{
      ui.set_max_width(name_width);
      ui.add(egui::Label::new(RichText::new(plugin.display_name()).strong()).wrap());
      ui.add(egui::Label::new(format!("{} · v{}",if plugin.author.trim().is_empty(){"Auteur inconnu"}else{&plugin.author},plugin.version)).wrap());
     });
    });
    ui.label(format!("{} action(s) · {} · Windows",plugin.actions.len(),plugin_type(&plugin.runtime)));
    if ui.add(iconset::button(ui.ctx(),Ico::Install,"Importer ce plugin")).clicked(){self.plugin_import_request(Some(plugin.folder.clone()));}
    ui.collapsing(format!("Détails et actions ({})",plugin.actions.len()),|ui|{
     ui.add(egui::Label::new(format!("Origine locale · Analyse : {:?} · Compatibilité non vérifiée · Redistribution non évaluée",plugin_import::compatibility(plugin))).wrap());
     for action in &plugin.actions {ui.add(egui::Label::new(format!("Action : {}",plugin_import::action_name(plugin,action))).wrap());}
    });
   }));}
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
  self.plugin_confirm_real(ui);
 }
 // Conversion UTC sans E/S disque et sans dépendance supplémentaire.
 fn plugin_confirm_real(&mut self,ui:&egui::Ui){
  if let Some(plugin)=self.plugin_real_confirmation.clone(){
   let mut open=true;
   egui::Window::new("Confirmer l’essai réel du plugin").open(&mut open).show(ui.ctx(),|ui|{
    ui.label("Un plugin tiers agit avec vos droits. Vérifiez les effets avant de confirmer :");
    if let Some(oracle)=self.plugin_oracle_cache.get(&format!("{}-{}",plugin.uuid,plugin.version)).and_then(Option::as_ref){for action in oracle.actions.iter().filter(|a|a.real_effects){
     ui.label(format!("Action : {}",action.action));ui.label(format!("Effet attendu : {}",action.expected_effect));ui.label(format!("Observation : {:?} · délai {} ms",action.observation,action.timeout_ms));
    }}
    ui.horizontal(|ui|{if ui.button("Confirmer et lancer l’essai réel").clicked(){self.plugin_test_request(plugin.clone(),true);self.plugin_real_confirmation=None;}
     if ui.button("Annuler").clicked(){self.plugin_real_confirmation=None;}});
   });
   if !open{self.plugin_real_confirmation=None;}
  }
 }
 fn plugin_test_request(&mut self,plugin:Plugin,confirmed_real:bool){
  self.start_dialog(move||{
   let oracle=plugin_validation::load_oracle(&plugin);
   let report=plugin_validation::run(&plugin,oracle.as_ref().ok().and_then(Option::as_ref),confirmed_real);
   let result=oracle.map_err(anyhow::Error::msg).and_then(|_|plugin_validation::save_report(&plugin,&report).map_err(anyhow::Error::msg));
   Some(Box::new(move |app:&mut App|match result{
    Ok(())=>{app.plugin_report_cache.insert(format!("{}-{}",plugin.uuid,plugin.version),Some(report));app.notice="Rapport de validation enregistré hors du plugin.".into();},
    Err(e)=>app.fail(format!("Rapport de plugin non enregistré : {e}")),
   }))
  });
 }
 /// Inventaire de l'onglet Plugins : lu une fois, puis réutilisé à chaque image egui.
 /// Invalidé à l'ouverture de l'onglet, après un import et par « Actualiser la liste ».
 fn plugin_inventory(&mut self)->std::sync::Arc<Inventory>{
  self.plugin_inventory.get_or_insert_with(||std::sync::Arc::new(Inventory::scan())).clone()
 }
 /// Après un import, le gestionnaire connaît le nouveau plugin sans redémarrage de l'application.
 fn refresh_plugin_catalog(&mut self){self.plugin_inventory=None;if let (Some(manager),Ok(root))=(self.plugins.as_mut(),plugin_import::installed_root()){manager.set_catalog(plugin_import::inventory(&root));}}
 fn plugin_import_request(&mut self,source:Option<std::path::PathBuf>){
  self.start_dialog(move||{let result=plugin_import::installed_root().and_then(|root|match source{Some(folder)=>plugin_import::import_folder(&folder,&root).map(|copied|format!("Plugin : {}",if copied{"copié"}else{"déjà présent"})),None=>{let report=plugin_import::import_sources(&root);Ok(format!("Import : {} copiés, {} déjà présents, {} protégés, {} intégrés FIFINE, {} ignorés",report.copied,report.existing,report.protected,report.built_in,report.skipped))}});
   Some(Box::new(move |app:&mut App|match result{Ok(text)=>{app.notice=text.clone();app.log(text);app.refresh_plugin_catalog();},Err(e)=>app.fail(format!("Import plugin impossible : {e}"))}))
  });
 }
 fn plugin_import_dialog(&mut self,archive:bool){self.start_dialog(move||{
  let path=if archive{egui_file_dialog_archive()}else{rfd::FileDialog::new().pick_folder()}?;
  let result=plugin_import::installed_root().and_then(|root|if archive{plugin_import::import_archive(&path,&root)}else{plugin_import::import_folder(&path,&root)});
  Some(Box::new(move |app:&mut App|match result{Ok(copied)=>{app.refresh_plugin_catalog();app.notice=if copied{"Plugin importé"}else{"Plugin déjà présent"}.into();},Err(e)=>app.fail(format!("Import plugin impossible : {e}"))}))
 });}
}
fn native_audio_switch(audio:&crate::audio::Snapshot,input:bool)->Option<Key>{
 let mut endpoints=audio.devices.iter().filter(|d|d.input==input);
 let first=endpoints.next()?;let second=endpoints.next()?;
 Some(Key{label:if input{"Basculer les micros"}else{"Basculer les sorties"}.into(),steps:vec![Step::AudioSwitch{first:first.id.clone(),second:second.id.clone(),input,first_label:first.name.clone(),second_label:second.name.clone(),first_icon:String::new(),second_icon:String::new()}],..Key::default()})
}
/// Plugins installés et plugins détectés dans les dossiers FIFINE / StreamDock / Elgato.
#[derive(Default)]
pub(super) struct Inventory{pub installed:Vec<Plugin>,pub available:Vec<Plugin>,pub built_in:Vec<Plugin>,pub protected:Vec<Plugin>,pub unavailable:Vec<Plugin>}
impl Inventory{
 fn scan()->Self{
  let mut inventory=Self::default();
  if let Ok(root)=plugin_import::installed_root(){inventory.installed=plugin_import::inventory(&root);}
  for source in plugin_import::sources(){for plugin in plugin_import::inventory(&source){
   match plugin_import::category(&plugin){Category::Available=>inventory.available.push(plugin),Category::BuiltIn=>inventory.built_in.push(plugin),Category::Protected=>inventory.protected.push(plugin),Category::NonExecutable=>inventory.unavailable.push(plugin)}
  }}
  inventory
 }
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

fn report_date(timestamp:u64)->String {
 let days=(timestamp/86400) as i64;let z=days+719468;
 let era=if z>=0 {z} else {z-146096}/146097;let doe=z-era*146097;
 let yoe=(doe-doe/1460+doe/36524-doe/146096)/365;
 let mut y=yoe+era*400;let doy=doe-(365*yoe+yoe/4-yoe/100);
 let mp=(5*doy+2)/153;let d=doy-(153*mp+2)/5+1;
 let m=mp+if mp<10 {3} else {-9};if m<=2 {y+=1;}
 format!("{d:02}/{m:02}/{y:04} {:02}:{:02}:{:02}",(timestamp%86400)/3600,(timestamp%3600)/60,timestamp%60)
}
