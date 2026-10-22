//! Fenêtre du magasin communautaire ; aucune requête tant que l'utilisateur n'a pas consenti.
use super::App;
use super::plugin_store_ui::community_state;
use crate::community_plugins::{self,Entry};
use crate::i18n::{tr,trf};
use eframe::egui::{self,RichText};
use std::collections::HashMap;
use std::sync::{mpsc::{self,Receiver},Mutex,OnceLock};

#[derive(Default)]
struct Market { open:bool, search:String, category:String, selected:Option<String>, enlarged:Option<String> }
fn market()->&'static Mutex<Market> { static STATE:OnceLock<Mutex<Market>>=OnceLock::new(); STATE.get_or_init(||Mutex::new(Market::default())) }
pub(super) fn open(){market().lock().unwrap_or_else(|e|e.into_inner()).open=true;}

enum ImageSlot { Pending(Receiver<Option<egui::ColorImage>>), Ready(egui::TextureHandle), Failed }
fn images()->&'static Mutex<HashMap<String,ImageSlot>> {static CACHE:OnceLock<Mutex<HashMap<String,ImageSlot>>>=OnceLock::new();CACHE.get_or_init(||Mutex::new(HashMap::new()))}
#[cfg(test)]
pub(super) fn test_image(ctx:&egui::Context,url:&str){
 let image=egui::ColorImage::new([2,2],egui::Color32::WHITE);
 images().lock().unwrap().insert(url.into(),ImageSlot::Ready(ctx.load_texture(url,image,egui::TextureOptions::LINEAR)));
}
fn thumbnail(ui:&mut egui::Ui,url:&str,icon:bool,size:egui::Vec2) -> bool {
 let mut cache=images().lock().unwrap_or_else(|e|e.into_inner());
 if !cache.contains_key(url){
  let (tx,rx)=mpsc::channel();let address=url.to_owned();let ctx=ui.ctx().clone();
  std::thread::spawn(move||{let result=community_plugins::image(&address,icon).ok();let _=tx.send(result);ctx.request_repaint();});
  cache.insert(url.to_owned(),ImageSlot::Pending(rx));
 }
 let slot=cache.get_mut(url).expect("image inscrite");
 if let ImageSlot::Pending(rx)=slot {if let Ok(result)=rx.try_recv(){*slot=match result {Some(image)=>ImageSlot::Ready(ui.ctx().load_texture(url,image,egui::TextureOptions::LINEAR)),None=>ImageSlot::Failed};}}
 if let ImageSlot::Ready(texture)=slot {return ui.add(egui::Image::new((texture.id(),size)).sense(egui::Sense::click())).clicked();}
 ui.label(if matches!(slot,ImageSlot::Pending(_)){tr("Chargement de l'image…")}else{tr("Image indisponible")});
 false
}
fn localized<'a>(fr:Option<&'a str>,en:&'a str)->&'a str {if crate::i18n::current()=="fr" {fr.unwrap_or(en)}else{en}}
fn category_name(category:&str)->&'static str {match category {
 "system"=>tr("Système"),"audio"=>tr("Audio"),"streaming"=>tr("Streaming"),"productivity"=>tr("Productivité"),"smart-home"=>tr("Maison connectée"),"communication"=>tr("Communication"),"gaming"=>tr("Jeux"),"utilities"=>tr("Utilitaires"),_=>tr("Autres")
}}
fn matching(entry:&Entry,search:&str,category:&str)->bool {
 (category.is_empty() || entry.category.as_deref()==Some(category)) &&
 (search.is_empty() || [&entry.name,&entry.author,&entry.description].into_iter().map(String::as_str)
 .chain(entry.summary.iter().map(String::as_str)).chain(entry.tags.iter().map(String::as_str))
 .any(|v|v.to_lowercase().contains(&search.to_lowercase())))
}
impl App {
 pub(super) fn plugin_market(&mut self,ctx:&egui::Context){
  let mut view=market().lock().unwrap_or_else(|e|e.into_inner());
  if !view.open{return;}
  let mut open=view.open;
  let screen=ctx.content_rect().size();
  egui::Window::new(tr("Magasin de plugins")).open(&mut open).resizable(true)
   .default_size(egui::vec2(screen.x.min(1000.0),screen.y.min(680.0)))
   .max_size(egui::vec2(screen.x.max(200.0),screen.y.max(200.0))).show(ctx,|ui|{
    let mut state=community_state().lock().unwrap_or_else(|e|e.into_inner());
    if let Some(rx)=state.fetching.as_ref(){if let Ok(result)=rx.try_recv(){state.fetching=None;state.loading=false;match result{Ok(entries)=>{state.entries=entries;state.message=tr("Bibliothèque actualisée.").into()},Err(error)=>state.message=trf("Bibliothèque indisponible : {error}",&[("error",&error)])}}}
    if let Some(rx)=state.installing.as_ref(){if let Ok(result)=rx.try_recv(){state.installing=None;match result{Ok(installed)=>{state.message=if installed{tr("Plugin communautaire importé.")}else{tr("Plugin déjà présent.")}.into();self.refresh_plugin_catalog();},Err(error)=>state.message=trf("Installation impossible : {error}",&[("error",&error)])}}}
    if !state.consent {
     ui.add(egui::Label::new(tr("Le magasin se connecte à GitHub. Rien n'est installé sans confirmation.")).wrap());
     if ui.button(tr("Se connecter au magasin")).clicked(){state.consent=true;let(tx,rx)=mpsc::channel();state.fetching=Some(rx);state.loading=true;let ctx=ctx.clone();std::thread::spawn(move||{let _=tx.send(community_plugins::fetch().map_err(|e|e.to_string()));ctx.request_repaint();});}
     return;
    }
    ui.horizontal_wrapped(|ui|{if ui.add_enabled(!state.loading,egui::Button::new(tr("Actualiser la bibliothèque"))).clicked(){let(tx,rx)=mpsc::channel();state.fetching=Some(rx);state.loading=true;let ctx=ctx.clone();std::thread::spawn(move||{let _=tx.send(community_plugins::fetch().map_err(|e|e.to_string()));ctx.request_repaint();});}});
    if state.loading{ui.label(tr("Chargement de la bibliothèque…"));}
    if !state.message.is_empty(){ui.add(egui::Label::new(&state.message).wrap());}
    ui.separator();
    egui::ScrollArea::vertical().show(ui,|ui|{
     if let Some(entry)=view.selected.as_ref().and_then(|id|state.entries.iter().find(|entry|&entry.id==id)).cloned(){
      if ui.button(tr("← Retour")).clicked(){view.selected=None;return;}
      if let Some(icon)=entry.icon.as_deref(){thumbnail(ui,icon,true,egui::vec2(96.0,96.0));}
      ui.heading(&entry.name);
      ui.label(trf("Auteur : {author}",&[("author",&entry.author)]));
      ui.label(trf("Version : {version}",&[("version",&entry.version)]));
      ui.label(trf("Licence : {license}",&[("license",&entry.license)]));
      ui.label(category_name(entry.category.as_deref().unwrap_or("")));
      if !entry.tested_decks.is_empty(){ui.label(trf("Decks testés : {decks}",&[("decks",&entry.tested_decks.join(", "))]));}
      if let Some(requires)=&entry.requires{ui.label(trf("Prérequis : {requires}",&[("requires",requires)]));}
      ui.label(trf("NovyDeck minimum : {version}",&[("version",&entry.min_novydeck)]));
      ui.add(egui::Label::new(localized(entry.description_fr.as_deref(),&entry.description)).wrap());
      if !entry.screenshots.is_empty(){ui.heading(tr("Captures"));egui::ScrollArea::horizontal().show(ui,|ui|{ui.horizontal(|ui|{for url in &entry.screenshots {if thumbnail(ui,url,false,egui::vec2(180.0,110.0)){view.enlarged=Some(url.clone());}}});});}
      if !entry.actions.is_empty(){ui.heading(tr("Actions"));for action in &entry.actions{ui.label(localized(action.name_fr.as_deref(),&action.name));}}
      if ui.button(tr("Page du projet")).clicked(){ctx.open_url(egui::OpenUrl::new_tab(&entry.homepage));}
      ui.label(tr("Communauté — non vérifié"));ui.add(egui::Label::new(tr("Ce plugin exécute du code tiers avec vos droits utilisateur.")).wrap());
      let installed=entry.uuid.as_ref().is_some_and(|uuid|self.plugin_inventory().installed.iter().any(|p|&p.uuid==uuid));
      if !entry.compatible{ui.label(tr("Incompatible"));}
      if ui.add_enabled(entry.compatible && !installed && state.installing.is_none(),egui::Button::new(if installed{tr("Installé")}else{tr("Installer")})).clicked(){state.confirmation=Some(entry);}
     }else{
      ui.horizontal(|ui|{ui.label(tr("Rechercher"));ui.text_edit_singleline(&mut view.search);});
      ui.horizontal_wrapped(|ui|{for (key,title) in [("",tr("Toutes")),("system",category_name("system")),("audio",category_name("audio")),("streaming",category_name("streaming")),("productivity",category_name("productivity")),("smart-home",category_name("smart-home")),("communication",category_name("communication")),("gaming",category_name("gaming")),("utilities",category_name("utilities"))]{if ui.selectable_label(view.category==key,title).clicked(){view.category=key.into();}}});
      let mut entries:Vec<_>=state.entries.iter().filter(|e|matching(e,&view.search,&view.category)).collect();
      entries.sort_by(|a,b|a.name.to_lowercase().cmp(&b.name.to_lowercase()));
      if entries.is_empty() && !state.loading{ui.label(tr("Aucun plugin ne correspond à votre recherche."));if ui.button(tr("Proposer un plugin")).clicked(){ctx.open_url(egui::OpenUrl::new_tab(community_plugins::CONTRIBUTE_URL));}}
      ui.horizontal_wrapped(|ui|{for entry in entries{ui.push_id(&entry.id,|ui|{ui.group(|ui|{ui.set_width(210.0_f32.min(ui.available_width().max(120.0)));if let Some(icon)=entry.icon.as_deref(){thumbnail(ui,icon,true,egui::vec2(64.0,64.0));}ui.label(RichText::new(&entry.name).strong());ui.label(&entry.author);ui.add(egui::Label::new(localized(entry.summary_fr.as_deref(),entry.summary.as_deref().unwrap_or(&entry.description))).truncate());ui.label(category_name(entry.category.as_deref().unwrap_or("")));let installed=entry.uuid.as_ref().is_some_and(|uuid|self.plugin_inventory().installed.iter().any(|p|&p.uuid==uuid));if installed{ui.label(tr("Installé"));}else if !entry.compatible{ui.label(tr("Incompatible"));}if ui.button(tr("Voir la fiche")).clicked(){view.selected=Some(entry.id.clone());}});});}});
     }
    });
   });
  view.open=open;
  if let Some(url)=view.enlarged.clone(){let mut enlarged=true;egui::Window::new(tr("Capture du plugin")).open(&mut enlarged).resizable(true).show(ctx,|ui|{thumbnail(ui,&url,false,egui::vec2(ui.available_width().min(800.0),ui.available_height().min(600.0)));});if !enlarged{view.enlarged=None;}}
 }
}
