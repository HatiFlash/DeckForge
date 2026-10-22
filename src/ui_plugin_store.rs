//! Vue de la bibliothèque consacrée aux plugins (aucun plugin externe n'est lancé ici).
use super::{App,DragItem};
use crate::{model::{Key,Step,PluginBinding},plugins::{Plugin,Runtime},plugin_import::{self,Category}};
use eframe::egui::{self,RichText};
use crate::iconset::{self,Ico};
use crate::plugin_validation;
use crate::community_plugins::{self,Entry};
use std::sync::{mpsc::{self,Receiver},Mutex,OnceLock};

#[derive(Default)]
pub(super) struct CommunityState {
 pub(super) consent:bool,
 pub(super) entries:Vec<Entry>,
 pub(super) loading:bool,
 pub(super) fetching:Option<Receiver<Result<Vec<Entry>,String>>>,
 pub(super) installing:Option<Receiver<Result<bool,String>>>,
 pub(super) confirmation:Option<Entry>,
 pub(super) message:String,
}
pub(super) fn community_state()->&'static Mutex<CommunityState>{static STATE:OnceLock<Mutex<CommunityState>>=OnceLock::new();STATE.get_or_init(||Mutex::new(CommunityState::default()))}
impl App {
 pub(super) fn plugin_store(&mut self,ui:&mut egui::Ui,search:&str){
  // Keep the entire tab inside one bounded viewport, including import controls and results.
  let height=ui.available_height().max(80.0);
  let content_width=ui.available_width();
  egui::ScrollArea::vertical().id_salt("plugin_store").auto_shrink([false,false]).max_height(height).show(ui,|ui|{
   // A ScrollArea may expose an unbounded child width; constrain paragraphs and cards to its viewport.
   ui.set_max_width(content_width);
   // Premier élément de l'onglet, sur une seule ligne pour garder l'inventaire local visible.
   let store=ui.add_sized([ui.available_width(),34.0],iconset::button(ui.ctx(),Ico::Store,crate::i18n::tr("Magasin de plugins")))
    .on_hover_text(crate::i18n::tr("Explorez les plugins communautaires avant de les installer."));
   if store.clicked(){super::plugin_market_ui::open();}
   ui.add_space(4.0);
  iconset::heading(ui,Ico::Plugin,RichText::new(crate::i18n::tr("Plugins · importer")).strong());
  ui.add(egui::Label::new(RichText::new(crate::i18n::tr("Catalogue local, hors ligne. Importer ne lance rien.")).small()).wrap());
  let catalog=crate::plugin_catalog::Catalog::offline();
  ui.label(crate::i18n::trf("Découvrir · {count} entrée(s) locales publiables", &[("count", &(catalog.entries.len()))] ));
  ui.separator();iconset::heading(ui,Ico::Install,RichText::new(crate::i18n::tr("Importer")).strong());
  ui.horizontal_wrapped(|ui|{
   if ui.button(crate::i18n::tr("Importer depuis FIFINE / StreamDock / Elgato")).clicked(){self.plugin_import_request(None);}
   if ui.button(crate::i18n::tr("Importer un fichier .streamDeckPlugin")).clicked(){self.plugin_import_dialog(true);}
   if ui.button(crate::i18n::tr("Importer un dossier .sdPlugin")).clicked(){self.plugin_import_dialog(false);}
   if ui.button(crate::i18n::tr("Actualiser la liste")).on_hover_text(crate::i18n::tr("Relire les dossiers de plugins")).clicked(){self.plugin_inventory=None;}
  });
  ui.label(RichText::new(&self.notice).small());
  let inventory=self.plugin_inventory();
   ui.separator();iconset::heading(ui,Ico::Plug,RichText::new(crate::i18n::tr("Installés dans NovyDeck")).strong());
   {for plugin in inventory.installed.iter(){
    if !format!("{} {}",plugin.display_name(),plugin.uuid).to_lowercase().contains(search) && !plugin.actions.iter().any(|a|a.name.to_lowercase().contains(search)){continue;}
    let ty=match plugin.runtime{Runtime::Exe(_)|Runtime::Node(_)=>crate::i18n::tr("programme"),Runtime::Html(_)=>crate::i18n::tr("web"),Runtime::Protected=>crate::i18n::tr("protégé"),Runtime::Unsupported=>crate::i18n::tr("indisponible")};
    ui.group(|ui|{ui.label(RichText::new(format!("{} · v{}",plugin.display_name(),plugin.version)).strong());
     ui.label(crate::i18n::trf("Auteur : {author} · Origine : import local", &[("author", &(if plugin.author.trim().is_empty(){crate::i18n::tr("inconnu")}else{&plugin.author}))] ));
     ui.label(crate::i18n::trf("Runtime : {ty} · Windows · Compatibilité : non vérifiée · Installé / inactif si aucune touche liée", &[("ty", &(ty))] ));
     if let Some(reason)=&plugin.prerequisite {ui.colored_label(egui::Color32::YELLOW,crate::i18n::trf("Prérequis manquant : {reason}", &[("reason", &(reason))] ));}
     if let Some(version)=&plugin.node_version {ui.label(crate::i18n::trf("Node.js demandé : {version}", &[("version", &(version))] ));}
     ui.label(crate::i18n::tr("Droits de redistribution : non évalués ; processus tiers = droits utilisateur, aucune sandbox."));
     ui.label(crate::i18n::trf("{count} actions · {status}", &[("count", &(plugin.actions.len())), ("status", &(self.plugin_errors.get(&plugin.uuid).map(String::as_str).unwrap_or(crate::i18n::tr("arrêté"))))] ));
     let unsupported=crate::plugin_host::unsupported_events(&plugin.uuid);
     ui.label(crate::i18n::trf("Événements non pris en charge : {events}", &[("events", &(if unsupported.is_empty(){crate::i18n::tr("aucun observé (ou trace désactivée)").into()}else{unsupported.join(", " )}))] ));
     let cache_key=format!("{}-{}",plugin.uuid,plugin.version);
     let report=self.plugin_report_cache.entry(cache_key).or_insert_with(||plugin_validation::report_path(&plugin).and_then(|p|std::fs::read(p).ok()).and_then(|bytes|serde_json::from_slice::<plugin_validation::Report>(&bytes).ok()).filter(|r|r.plugin==plugin.uuid && r.version==plugin.version));
     if let Some(report)=report {
      for result in &report.actions {
       let verdict=match result.verdict {crate::plugin_validation::Verdict::Passed=>crate::i18n::tr("Réussi"),crate::plugin_validation::Verdict::Failed=>crate::i18n::tr("Échoué"),crate::plugin_validation::Verdict::Unverified=>crate::i18n::tr("Non vérifié")};
       ui.label(format!("{} : {verdict} · {} · {} · {} UTC",result.action,result.reason,result.mode,report_date(result.tested_at_unix)));
      }
     } else { ui.label(crate::i18n::tr("Validation : non vérifiée (aucun rapport pour cette version).")); }
     if ui.add(iconset::button(ui.ctx(),Ico::Play,crate::i18n::tr("Tester le plugin (simulation uniquement)"))).clicked(){self.plugin_test_request(plugin.clone(),false);}
     let oracle=self.plugin_oracle_cache.entry(format!("{}-{}",plugin.uuid,plugin.version)).or_insert_with(||plugin_validation::load_oracle(&plugin).ok().flatten());
     if oracle.as_ref().is_some_and(|o|o.actions.iter().any(|a|a.real_effects)) && ui.button(crate::i18n::tr("Essai réel…")).clicked(){self.plugin_real_confirmation=Some(plugin.clone());}
     ui.label(crate::i18n::tr("Actions disponibles (placer sur une touche peut lancer le plugin) :"));
     for action in &plugin.actions{
      let name=plugin_import::action_name(&plugin,action);
      let mut key=Key{label:name.clone(),..Key::default()};
      key.image=action.states.first().and_then(|p|p.as_ref()).or(action.icon.as_ref()).map(|p|p.to_string_lossy().into_owned()).unwrap_or_default();
      key.plugin=Some(PluginBinding{plugin:plugin.uuid.clone(),action:action.uuid.clone(),settings:serde_json::json!({}),state:0});
      let response=ui.button(format!("+ {name}"));
      self.drag_sources.push((response.rect.intersect(ui.clip_rect()),DragItem::Bind(key.clone())));
      if response.clicked(){self.assign(DragItem::Bind(key),self.selected);}
      response.on_hover_text(crate::i18n::tr("Placer sur la touche sélectionnée, ou glisser sur une touche"));
     }
    });
   }}
   ui.separator();
   iconset::heading(ui,Ico::Store,RichText::new(crate::i18n::tr("Plugins disponibles sur ce PC")).strong());
  if search.is_empty() || "volumecontroller audio volume muet peripherique".contains(&search.to_lowercase()) {
   ui.collapsing(crate::i18n::tr("Audio natif · sans plugin"),|ui| ui.group(|ui| {
    ui.set_max_width((content_width-24.0).max(120.0));
    iconset::heading(ui,Ico::Headset,RichText::new(crate::i18n::tr("Audio natif NovyDeck")).strong());
    ui.add(egui::Label::new(crate::i18n::tr("Volume + / - et muet contrôlent le volume Windows par touches multimédias. Le basculement utilise les périphériques Core Audio détectés ci-dessous.")).wrap());
    ui.horizontal_wrapped(|ui| {
     for (label,command) in [("+ Volume +","volume_up"),("+ Volume -","volume_down"),("+ Muet / rétablir","mute")] {
      let key=Key{label:label.trim_start_matches("+ ").into(),steps:vec![Step::Media{command:command.into()}],..Key::default()};
      let response=ui.button(match command {"volume_up"=>crate::i18n::tr("+ Volume +"),"volume_down"=>crate::i18n::tr("+ Volume -"),_=>crate::i18n::tr("+ Muet / rétablir")});
      self.drag_sources.push((response.rect.intersect(ui.clip_rect()),DragItem::Bind(key.clone())));
      if response.clicked(){self.assign(DragItem::Bind(key),self.selected);}
     }
    });
    if !self.audio.error.is_empty(){ui.label(crate::i18n::trf("Audio : {error}", &[("error", &(self.audio.error))] ));}
    for (input,label) in [(false,crate::i18n::tr("Sorties")),(true,crate::i18n::tr("Entrées / micros"))] {
     let endpoints:Vec<_>=self.audio.devices.iter().filter(|d|d.input==input).collect();
     ui.label(format!("{} : {}",label,if endpoints.is_empty(){crate::i18n::tr("aucun périphérique actif").into()}else{endpoints.iter().map(|d|d.name.as_str()).collect::<Vec<_>>().join(" ; ")}));
     let key=native_audio_switch(&self.audio,input);
     let button = if input { crate::i18n::tr("+ Basculer deux entrées / micros") } else { crate::i18n::tr("+ Basculer deux sorties") };
     let response=ui.add_enabled(key.is_some(),egui::Button::new(button));
     if let Some(key)=key {
      self.drag_sources.push((response.rect.intersect(ui.clip_rect()),DragItem::Bind(key.clone())));
      if response.clicked(){self.assign(DragItem::Bind(key),self.selected);}
     }
    }
    ui.add(egui::Label::new(RichText::new(crate::i18n::tr("La liste est actualisée automatiquement par Core Audio. Choisis ensuite les deux périphériques dans l’inspecteur ; aucune configuration de plugin existante n’est convertie.")).small()).wrap());
   }));
  }
   let matches=|p:&&Plugin|format!("{} {}",p.display_name(),p.uuid).to_lowercase().contains(search);
   let available:Vec<&Plugin>=inventory.available.iter().filter(matches).collect();
   let built_in:Vec<&Plugin>=inventory.built_in.iter().filter(matches).collect();
   let protected:Vec<&Plugin>=inventory.protected.iter().filter(matches).collect();
   let unavailable:Vec<&Plugin>=inventory.unavailable.iter().filter(matches).collect();
   if !available.is_empty(){ui.horizontal(|ui|{
    ui.heading(crate::i18n::trf("Disponibles ({count})", &[("count", &(available.len()))] ));
    if ui.button(crate::i18n::trf("Importer tout ({count})", &[("count", &(available.len()))] )).clicked(){self.plugin_import_request(None);}
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
      ui.add(egui::Label::new(format!("{} · v{}",if plugin.author.trim().is_empty(){crate::i18n::tr("Auteur inconnu")}else{&plugin.author},plugin.version)).wrap());
     });
    });
    ui.label(crate::i18n::trf("{count} action(s) · {type} · Windows", &[("count", &plugin.actions.len()), ("type", &plugin_type(&plugin.runtime))]));
    if ui.add(iconset::button(ui.ctx(),Ico::Install,crate::i18n::tr("Importer ce plugin"))).clicked(){self.plugin_import_request(Some(plugin.folder.clone()));}
    ui.collapsing(crate::i18n::trf("Détails et actions ({count})", &[("count", &(plugin.actions.len()))] ),|ui|{
     ui.add(egui::Label::new(crate::i18n::trf("Origine locale · Analyse : {analysis} · Compatibilité non vérifiée · Redistribution non évaluée", &[("analysis", &format!("{:?}", plugin_import::compatibility(plugin)))] )).wrap());
     for action in &plugin.actions {ui.add(egui::Label::new(crate::i18n::trf("Action : {action}", &[("action", &(plugin_import::action_name(plugin,action)))] )).wrap());}
    });
   }));}
   ui.collapsing(crate::i18n::trf("Intégrés à FIFINE, à recréer nativement ({count})", &[("count", &(built_in.len()))] ),|ui|{
    for plugin in &built_in{ui.label(plugin.display_name());}
   });
   ui.collapsing(crate::i18n::trf("Non exécutables ({count})", &[("count", &(unavailable.len()))] ),|ui|{
    for plugin in &unavailable{ui.label(plugin.display_name());}
   });
   ui.collapsing(crate::i18n::trf("Protégés ({count})", &[("count", &(protected.len()))] ),|ui|{
    for plugin in &protected{ui.label(plugin.display_name());}
   });
   // Le catalogue communautaire s'ouvre dans une fenêtre dédiée.
  });
  self.plugin_market(ui.ctx());
  self.community_confirmation(ui);
  self.plugin_confirm_real(ui);
 }
 fn community_section(&mut self,ui:&mut egui::Ui,search:&str){
  let mut state=community_state().lock().unwrap_or_else(|e|e.into_inner());
  if let Some(rx)=state.fetching.as_ref(){if let Ok(result)=rx.try_recv(){
   state.loading=false;state.fetching=None;
   match result{Ok(entries)=>{state.entries=entries;state.message=crate::i18n::tr("Bibliothèque actualisée.").into();},Err(error)=>state.message=crate::i18n::trf("Bibliothèque indisponible : {error}", &[("error", &(error))] )}
  }}
  if let Some(rx)=state.installing.as_ref(){if let Ok(result)=rx.try_recv(){
   state.installing=None;
   match result{Ok(installed)=>{state.message=if installed{crate::i18n::tr("Plugin communautaire importé.")}else{crate::i18n::tr("Plugin déjà présent.")}.into();self.refresh_plugin_catalog();},Err(error)=>state.message=crate::i18n::trf("Installation impossible : {error}", &[("error", &(error))] )}
  }}
  ui.separator();iconset::heading(ui,Ico::Store,RichText::new(crate::i18n::tr("Communauté")).strong());
  if !state.consent{
   ui.add(egui::Label::new(crate::i18n::tr("La bibliothèque communautaire nécessite une connexion à GitHub.")).wrap());
   if ui.button(crate::i18n::tr("Se connecter à la bibliothèque")).clicked(){state.consent=true;start_community_fetch(&mut state,ui.ctx());}
   return;
  }
  if ui.add_enabled(!state.loading,egui::Button::new(crate::i18n::tr("Actualiser la bibliothèque"))).clicked(){start_community_fetch(&mut state,ui.ctx());}
  if state.loading{ui.label(crate::i18n::tr("Chargement de la bibliothèque…"));}
  if !state.message.is_empty(){ui.add(egui::Label::new(&state.message).wrap());}
  if !state.loading && state.entries.is_empty(){
   ui.label(crate::i18n::tr("Aucun plugin communautaire pour l'instant."));
   if ui.button(crate::i18n::tr("Proposer un plugin")).clicked(){ui.ctx().open_url(egui::OpenUrl::new_tab(community_plugins::CONTRIBUTE_URL));}
  }
  for entry in state.entries.clone(){
   if !community_matches(&entry,search){continue;}
   ui.push_id(&entry.id,|ui|ui.group(|ui|{
    ui.set_max_width(ui.available_width());
    ui.add(egui::Label::new(RichText::new(format!("{} · {}",entry.name,entry.version)).strong()).wrap());
    ui.label(crate::i18n::tr("Communauté — non vérifié"));
    ui.add(egui::Label::new(format!("{} · {}",entry.author,entry.license)).wrap());
    ui.add(egui::Label::new(if crate::i18n::current() == "fr" { entry.description_fr.as_deref().unwrap_or(&entry.description) } else { &entry.description }).wrap());
    if !entry.tested_decks.is_empty(){ui.add(egui::Label::new(crate::i18n::trf("Decks testés : {decks}", &[("decks", &(entry.tested_decks.join(", ")))] )).wrap());}
    if !entry.compatible{ui.label(crate::i18n::trf("Incompatible : nécessite NovyDeck {version} ou plus récent", &[("version", &(entry.min_novydeck))] ));}
    ui.horizontal_wrapped(|ui|{
     if ui.button(crate::i18n::tr("Voir la page")).clicked(){ui.ctx().open_url(egui::OpenUrl::new_tab(&entry.homepage));}
     if ui.add_enabled(entry.compatible && state.installing.is_none(),egui::Button::new(crate::i18n::tr("Installer…"))).clicked(){state.confirmation=Some(entry.clone());}
    });
   }));
  }
 }
 fn community_confirmation(&mut self,ui:&egui::Ui){
  let mut state=community_state().lock().unwrap_or_else(|e|e.into_inner());
  let Some(entry)=state.confirmation.clone() else{return;};
  let mut open=true;
  egui::Window::new(crate::i18n::tr("Confirmer l'installation communautaire")).open(&mut open).collapsible(false).show(ui.ctx(),|ui|{
   ui.label(crate::i18n::trf("Plugin : {name} · Auteur : {author}", &[("name", &(entry.name)), ("author", &(entry.author))] ));
   ui.add(egui::Label::new(crate::i18n::trf("Téléchargement : {url}", &[("url", &(entry.download))] )).wrap());
   ui.add(egui::Label::new(crate::i18n::trf("SHA-256 attendu : {hash}", &[("hash", &(entry.sha256))] )).wrap());
   ui.label(crate::i18n::tr("Ce plugin exécute du code tiers avec vos droits utilisateur."));
   ui.horizontal(|ui|{
    if ui.add_enabled(state.installing.is_none(),egui::Button::new(crate::i18n::tr("Confirmer et installer"))).clicked(){
     let (tx,rx)=mpsc::channel();state.installing=Some(rx);state.confirmation=None;
     let ctx=ui.ctx().clone();
     std::thread::spawn(move||{
      let result=plugin_import::installed_root().and_then(|root|community_plugins::install(&entry,&root)).map_err(|e|e.to_string());
      let _=tx.send(result);ctx.request_repaint();
     });
    }
    if ui.button(crate::i18n::tr("Annuler")).clicked(){state.confirmation=None;}
   });
  });
  if !open{state.confirmation=None;}
 }
 // Conversion UTC sans E/S disque et sans dépendance supplémentaire.
 fn plugin_confirm_real(&mut self,ui:&egui::Ui){
  if let Some(plugin)=self.plugin_real_confirmation.clone(){
   let mut open=true;
   egui::Window::new(crate::i18n::tr("Confirmer l’essai réel du plugin")).open(&mut open).show(ui.ctx(),|ui|{
    ui.label(crate::i18n::tr("Un plugin tiers agit avec vos droits. Vérifiez les effets avant de confirmer :"));
    if let Some(oracle)=self.plugin_oracle_cache.get(&format!("{}-{}",plugin.uuid,plugin.version)).and_then(Option::as_ref){for action in oracle.actions.iter().filter(|a|a.real_effects){
     ui.label(crate::i18n::trf("Action : {action}", &[("action", &(action.action))] ));ui.label(crate::i18n::trf("Effet attendu : {effect}", &[("effect", &(action.expected_effect))] ));ui.label(crate::i18n::trf("Observation : {observation} · délai {timeout} ms", &[("observation", &(format!("{:?}",action.observation))), ("timeout", &(action.timeout_ms))] ));
    }}
    ui.horizontal(|ui|{if ui.button(crate::i18n::tr("Confirmer et lancer l’essai réel")).clicked(){self.plugin_test_request(plugin.clone(),true);self.plugin_real_confirmation=None;}
     if ui.button(crate::i18n::tr("Annuler")).clicked(){self.plugin_real_confirmation=None;}});
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
    Ok(())=>{app.plugin_report_cache.insert(format!("{}-{}",plugin.uuid,plugin.version),Some(report));app.notice=crate::i18n::tr("Rapport de validation enregistré hors du plugin.").into();},
    Err(e)=>app.fail(crate::i18n::trf("Rapport de plugin non enregistré : {error}", &[("error", &(e))] )),
   }))
  });
 }
 /// Inventaire de l'onglet Plugins : lu une fois, puis réutilisé à chaque image egui.
 /// Invalidé à l'ouverture de l'onglet, après un import et par « Actualiser la liste ».
 pub(super) fn plugin_inventory(&mut self)->std::sync::Arc<Inventory>{
  self.plugin_inventory.get_or_insert_with(||std::sync::Arc::new(Inventory::scan())).clone()
 }
 /// Après un import, le gestionnaire connaît le nouveau plugin sans redémarrage de l'application.
 pub(super) fn refresh_plugin_catalog(&mut self){self.plugin_inventory=None;if let (Some(manager),Ok(root))=(self.plugins.as_mut(),plugin_import::installed_root()){manager.set_catalog(plugin_import::inventory(&root));}}
 fn plugin_import_request(&mut self,source:Option<std::path::PathBuf>){
  self.start_dialog(move||{let result=plugin_import::installed_root().and_then(|root|match source{Some(folder)=>plugin_import::import_folder(&folder,&root).map(|copied|if copied{crate::i18n::tr("Plugin : copié").into()}else{crate::i18n::tr("Plugin : déjà présent").into()}),None=>{let report=plugin_import::import_sources(&root);Ok(crate::i18n::trf("Import : {copied} copiés, {existing} déjà présents, {protected} protégés, {built_in} intégrés FIFINE, {skipped} ignorés", &[("copied", &report.copied), ("existing", &report.existing), ("protected", &report.protected), ("built_in", &report.built_in), ("skipped", &report.skipped)]))}});
   Some(Box::new(move |app:&mut App|match result{Ok(text)=>{app.notice=text.clone();app.log(text);app.refresh_plugin_catalog();},Err(e)=>app.fail(crate::i18n::trf("Import plugin impossible : {error}", &[("error", &(e))] ))}))
  });
 }
 fn plugin_import_dialog(&mut self,archive:bool){self.start_dialog(move||{
  let path=if archive{egui_file_dialog_archive()}else{rfd::FileDialog::new().pick_folder()}?;
  let result=plugin_import::installed_root().and_then(|root|if archive{plugin_import::import_archive(&path,&root)}else{plugin_import::import_folder(&path,&root)});
  Some(Box::new(move |app:&mut App|match result{Ok(copied)=>{app.refresh_plugin_catalog();app.notice=if copied{crate::i18n::tr("Plugin importé")}else{crate::i18n::tr("Plugin déjà présent")}.into();},Err(e)=>app.fail(crate::i18n::trf("Import plugin impossible : {error}", &[("error", &(e))] ))}))
 });}
}
fn start_community_fetch(state:&mut CommunityState,ctx:&egui::Context){
 let (tx,rx)=mpsc::channel();state.fetching=Some(rx);state.loading=true;state.message.clear();
 let ctx=ctx.clone();std::thread::spawn(move||{let result=community_plugins::fetch().map_err(|e|e.to_string());let _=tx.send(result);ctx.request_repaint();});
}
fn community_matches(entry:&Entry,search:&str)->bool{
 let search=search.trim().to_lowercase();
 search.is_empty() || [entry.name.as_str(),entry.author.as_str(),entry.version.as_str(),entry.description.as_str(),entry.description_fr.as_deref().unwrap_or(""),entry.license.as_str()]
  .into_iter().chain(entry.tested_decks.iter().map(String::as_str)).any(|field|field.to_lowercase().contains(&search))
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
fn plugin_type(runtime:&Runtime)->&'static str{match runtime{Runtime::Exe(_)|Runtime::Node(_)=>crate::i18n::tr("Programme"),Runtime::Html(_)=>crate::i18n::tr("Web"),_=>crate::i18n::tr("Non exécutable")}}
fn plugin_icon(ui:&mut egui::Ui,plugin:&Plugin){
 if let Some(texture)=plugin.icon.as_ref().and_then(|p|super::icon_ui::texture(ui.ctx(),&p.to_string_lossy())){
  ui.image((texture.id(),egui::vec2(42.0,42.0)));
 }
}
fn egui_file_dialog_archive()->Option<std::path::PathBuf>{rfd::FileDialog::new().add_filter(crate::i18n::tr("Plugin Stream Deck"), &["streamDeckPlugin","zip"]).pick_file()}
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
 #[test] fn community_search_covers_author_version_license_and_decks(){
  let entry=Entry{id:"x".into(),name:"Nom".into(),author:"Alice".into(),version:"1.2.0".into(),description:"Audio".into(),description_fr:Some("Son".into()),homepage:"https://example.org".into(),download:"https://example.org/x.zip".into(),sha256:"a".repeat(64),license:"MIT".into(),tested_decks:vec!["FIFINE D6".into()],min_novydeck:"0.8.12".into(),uuid:None,category:None,summary:None,summary_fr:None,icon:None,screenshots:vec![],actions:vec![],tags:vec![],requires:None,compatible:true};
  for query in ["nom","alice","1.2","audio","son","mit","fifine"]{assert!(community_matches(&entry,query),"{query}");}
  assert!(!community_matches(&entry,"inconnu"));
 }
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
