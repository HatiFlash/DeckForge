//! Vue de la bibliothèque consacrée aux plugins (aucun plugin externe n'est lancé ici).
use super::{App,DragItem};
use crate::{model::{Key,PluginBinding},plugins::{Plugin,Runtime},plugin_import::{self,Category}};
use eframe::egui::{self,RichText};
impl App {
 pub(super) fn plugin_store(&mut self,ui:&mut egui::Ui,search:&str){
  ui.label(RichText::new("Les plugins restent arrêtés tant qu'ils ne sont pas utilisés.").small());
  ui.horizontal_wrapped(|ui|{
   if ui.button("Importer depuis FIFINE / StreamDock").clicked(){self.plugin_import_request(None);}
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
    ui.group(|ui|{ui.label(RichText::new(format!("{} · v{}",plugin.display_name(),plugin.version)).strong());ui.label(format!("{} actions · {} · arrêté",plugin.actions.len(),ty));
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
   ui.label(RichText::new("Disponibles depuis FIFINE / StreamDock").strong());
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
      ui.label(format!("{} action(s) · {}",plugin.actions.len(),plugin_type(&plugin.runtime)));
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
fn plugin_type(runtime:&Runtime)->&'static str{match runtime{Runtime::Exe(_)|Runtime::Node(_)=>"Programme",Runtime::Html(_)=>"Web",_=>"Non exécutable"}}
fn plugin_icon(ui:&mut egui::Ui,plugin:&Plugin){
 if let Some(texture)=plugin.icon.as_ref().and_then(|p|super::icon_ui::texture(ui.ctx(),&p.to_string_lossy())){
  ui.image((texture.id(),egui::vec2(42.0,42.0)));
 }
}
fn egui_file_dialog_archive()->Option<std::path::PathBuf>{rfd::FileDialog::new().add_filter("Plugin Stream Deck", &["streamDeckPlugin","zip"]).pick_file()}
#[cfg(test)] mod tests {use super::*;
 #[test] fn binding_for_imported_action_roundtrips(){let k=Key{plugin:Some(PluginBinding{plugin:"fixture.plugin".into(),action:"fixture.action".into(),settings:serde_json::json!({}),state:0}),..Key::default()};let value=serde_json::to_value(&k).unwrap();assert_eq!(serde_json::from_value::<Key>(value).unwrap().plugin.unwrap().action,"fixture.action");}
}
