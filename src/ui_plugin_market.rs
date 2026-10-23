//! Fenêtre du magasin communautaire ; aucune requête tant que l'utilisateur n'a pas consenti.
use super::App;
use super::plugin_store_ui::community_state;
use crate::community_plugins::{self,Entry};
use crate::i18n::{tr,trf};
use eframe::egui::{self, RichText};
use crate::{iconset::{self, Ico, Family}, theme};
use std::collections::HashMap;
use std::sync::{mpsc::{self,Receiver},Mutex,OnceLock};

#[derive(Default)]
struct Market { open:bool, search:String, category:String, selected:Option<String>, enlarged:Option<String> }
fn market()->&'static Mutex<Market> { static STATE:OnceLock<Mutex<Market>>=OnceLock::new(); STATE.get_or_init(||Mutex::new(Market::default())) }
pub(super) fn open(){market().lock().unwrap_or_else(|e|e.into_inner()).open=true;}
/// Le magasin est une page de l'application (comme l'Atelier), pas une fenêtre flottante.
pub(super) fn is_open()->bool{market().lock().unwrap_or_else(|e|e.into_inner()).open}

enum ImageSlot { Pending(Receiver<Option<egui::ColorImage>>), Ready(egui::TextureHandle), Failed }
fn images()->&'static Mutex<HashMap<String,ImageSlot>> {static CACHE:OnceLock<Mutex<HashMap<String,ImageSlot>>>=OnceLock::new();CACHE.get_or_init(||Mutex::new(HashMap::new()))}
#[cfg(test)]
pub(super) fn test_image_png(ctx:&egui::Context,url:&str,bytes:&[u8]){
 let decoded=image::load_from_memory(bytes).expect("PNG de test").to_rgba8();
 let image=egui::ColorImage::from_rgba_unmultiplied([decoded.width() as usize,decoded.height() as usize],decoded.as_raw());
 images().lock().unwrap().insert(url.into(),ImageSlot::Ready(ctx.load_texture(url,image,egui::TextureOptions::LINEAR)));
}
#[cfg(test)]
pub(super) fn test_close(){let mut view=market().lock().unwrap();*view=Market::default();}
#[cfg(test)]
pub(super) fn test_view(selected:Option<&str>){let mut view=market().lock().unwrap();view.open=true;view.selected=selected.map(str::to_owned);view.search.clear();view.category.clear();}
#[cfg(test)]
pub(super) fn test_image(ctx:&egui::Context,url:&str){
 let image=egui::ColorImage::new([2,2],egui::Color32::WHITE);
 images().lock().unwrap().insert(url.into(),ImageSlot::Ready(ctx.load_texture(url,image,egui::TextureOptions::LINEAR)));
}
fn thumbnail(ui:&mut egui::Ui,url:Option<&str>,icon:bool,size:egui::Vec2) -> bool {
 let texture=url.and_then(|url|{
  let mut cache=images().lock().unwrap_or_else(|e|e.into_inner());
  if !cache.contains_key(url){
   let (tx,rx)=mpsc::channel();let address=url.to_owned();let ctx=ui.ctx().clone();
   std::thread::spawn(move||{let result=community_plugins::image(&address,icon).ok();let _=tx.send(result);ctx.request_repaint();});
   cache.insert(url.to_owned(),ImageSlot::Pending(rx));
  }
  let slot=cache.get_mut(url).expect("image inscrite");
  if let ImageSlot::Pending(rx)=slot {if let Ok(result)=rx.try_recv(){*slot=match result {Some(image)=>ImageSlot::Ready(ui.ctx().load_texture(url,image,egui::TextureOptions::LINEAR)),None=>ImageSlot::Failed};}}
  if let ImageSlot::Ready(texture)=slot {Some(texture.id())}else{None}
 });
 let (rect,response)=ui.allocate_exact_size(size,if icon{egui::Sense::hover()}else{egui::Sense::click()});
 let p=theme::palette(ui.ctx());
 ui.painter().rect_filled(rect,theme::RADIUS_MD,p.card);
 if let Some(texture)=texture {
  theme::rounded_image(ui.painter(),texture,rect,theme::RADIUS_MD as f32);
 } else if icon {
  iconset::paint(ui.painter(),rect.center(),size.x.min(size.y)*0.58,Ico::Plugin,Family::Plugin,1.0,25);
 } else {
  iconset::paint(ui.painter(),rect.center(),42.0,Ico::Picture,Family::Image,0.7,25);
 }
 if !icon {ui.painter().rect_stroke(rect,theme::RADIUS_MD,egui::Stroke::new(1.0,p.border),egui::StrokeKind::Inside);}
 response.clicked()
}
fn meta(ui:&mut egui::Ui,text:String){let p=theme::palette(ui.ctx());theme::pill(ui,&text,p.text_secondary);}
fn excerpt(text:&str,limit:usize)->String {
 let mut chars=text.chars();let short:String=chars.by_ref().take(limit).collect();
 if chars.next().is_some(){format!("{}…",short.trim_end())}else{short}
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
  // Échap : fiche → grille, grille → retour au deck (sauf saisie en cours).
  if view.enlarged.is_none() && !ctx.wants_keyboard_input() && ctx.input(|i|i.key_pressed(egui::Key::Escape)) {
   if view.selected.is_some(){view.selected=None;}else{open=false;}
  }
  egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(24,16))).show(ctx,|ui|{
    let p=theme::palette(ctx);
    ui.horizontal(|ui|{
     let back=if view.selected.is_some(){tr("← Retour")}else{tr("← Retour au deck")};
     if ui.button(back).clicked(){if view.selected.is_some(){view.selected=None;}else{open=false;}}
     ui.add_space(12.0);
     if let Some(image)=iconset::image(ctx,Ico::Store,22.0){ui.add(image);}
     ui.label(RichText::new(tr("Magasin de plugins")).size(22.0).strong().color(p.ink));
    });
    ui.add_space(10.0);
    let mut state=community_state().lock().unwrap_or_else(|e|e.into_inner());
    if let Some(rx)=state.fetching.as_ref(){if let Ok(result)=rx.try_recv(){state.fetching=None;state.loading=false;match result{Ok(entries)=>{state.entries=entries;state.message=tr("Bibliothèque actualisée.").into()},Err(error)=>state.message=trf("Bibliothèque indisponible : {error}",&[("error",&error)])}}}
    if let Some(rx)=state.installing.as_ref(){if let Ok(result)=rx.try_recv(){state.installing=None;match result{Ok(installed)=>{state.message=if installed{tr("Plugin communautaire importé.")}else{tr("Plugin déjà présent.")}.into();self.refresh_plugin_catalog();},Err(error)=>state.message=trf("Installation impossible : {error}",&[("error",&error)])}}}
    if !state.consent {
     ui.add_space(24.0);
     ui.add(egui::Label::new(tr("Le magasin se connecte à GitHub. Rien n'est installé sans confirmation.")).wrap());
     ui.add_space(8.0);
     if ui.button(tr("Se connecter au magasin")).clicked(){state.consent=true;let(tx,rx)=mpsc::channel();state.fetching=Some(rx);state.loading=true;let ctx=ctx.clone();std::thread::spawn(move||{let _=tx.send(community_plugins::fetch().map_err(|e|e.to_string()));ctx.request_repaint();});}
     return;
    }
    ui.horizontal(|ui|{
     let width=(ui.available_width()-130.0).max(160.0);
     egui::Frame::new().fill(p.surface).stroke(egui::Stroke::new(1.0,p.border)).corner_radius(8)
      .inner_margin(egui::Margin::symmetric(10,7)).show(ui,|ui|{
       ui.set_width(width-20.0);
       ui.horizontal(|ui|{
        // Loupe dessinée comme la recherche de la bibliothèque d'actions.
        let (icon_r,_)=ui.allocate_exact_size(egui::Vec2::splat(16.0),egui::Sense::hover());
        let c=icon_r.center()-egui::Vec2::new(1.5,1.5);
        ui.painter().circle_stroke(c,4.5,egui::Stroke::new(1.5_f32,p.muted));
        ui.painter().line_segment([c+egui::Vec2::new(3.2,3.2),c+egui::Vec2::new(6.5,6.5)],egui::Stroke::new(1.5_f32,p.muted));
        ui.add(egui::TextEdit::singleline(&mut view.search).frame(false).desired_width(f32::INFINITY).hint_text(tr("Rechercher un plugin…")));
       });
      });
     if ui.add_enabled(!state.loading,iconset::secondary_button(ctx,Ico::Redo,tr("Actualiser"))).clicked(){let(tx,rx)=mpsc::channel();state.fetching=Some(rx);state.loading=true;let ctx=ctx.clone();std::thread::spawn(move||{let _=tx.send(community_plugins::fetch().map_err(|e|e.to_string()));ctx.request_repaint();});}
    });
    if state.loading{ui.label(tr("Chargement de la bibliothèque…"));}
    if !state.message.is_empty(){ui.add(egui::Label::new(RichText::new(&state.message).color(p.text_secondary)).wrap());}
    ui.add_space(12.0);
    egui::ScrollArea::vertical().id_salt("market_content").auto_shrink([false,false]).show(ui,|ui|{
     if let Some(entry)=view.selected.as_ref().and_then(|id|state.entries.iter().find(|entry|&entry.id==id)).cloned(){
      let installed=entry.uuid.as_ref().is_some_and(|uuid|self.plugin_inventory().installed.iter().any(|p|&p.uuid==uuid));
      theme::panel_frame(p).show(ui,|ui|{
       ui.horizontal(|ui|{
        thumbnail(ui,entry.icon.as_deref(),true,egui::vec2(96.0,96.0));
        ui.vertical(|ui|{
         ui.label(RichText::new(&entry.name).size(26.0).strong());
         ui.label(RichText::new(&entry.author).color(p.text_secondary));
         theme::pill(ui,category_name(entry.category.as_deref().unwrap_or("")),p.accent);
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
         if ui.add_enabled_ui(entry.compatible && !installed && state.installing.is_none(),|ui|ui.add_sized([120.0,44.0],theme::primary(if installed{tr("Installé")}else{tr("Installer")}))).inner.clicked(){state.confirmation=Some(entry.clone());}
         if !entry.compatible{theme::pill(ui,tr("Incompatible"),p.warning);}
        });
       });
      });
      ui.add_space(16.0);
      ui.horizontal_wrapped(|ui|{
       meta(ui,trf("Version : {version}",&[("version",&entry.version)]));
       meta(ui,trf("Licence : {license}",&[("license",&entry.license)]));
       meta(ui,trf("NovyDeck minimum : {version}",&[("version",&entry.min_novydeck)]));
       if !entry.tested_decks.is_empty(){meta(ui,trf("Decks testés : {decks}",&[("decks",&entry.tested_decks.join(", "))]));}
       if let Some(requires)=&entry.requires{meta(ui,trf("Prérequis : {requires}",&[("requires",requires)]));}
      });
      ui.add_space(18.0);
      if !entry.screenshots.is_empty(){
       theme::section_title(ui,tr("Captures"));ui.add_space(8.0);
       egui::ScrollArea::horizontal().id_salt("market_shots").show(ui,|ui|{ui.horizontal(|ui|{for url in &entry.screenshots {if thumbnail(ui,Some(url),false,egui::vec2(320.0,180.0)){view.enlarged=Some(url.clone());}}});});
       ui.add_space(18.0);
      }
      theme::section_title(ui,tr("À propos"));ui.add_space(8.0);
      ui.add(egui::Label::new(localized(entry.description_fr.as_deref(),&entry.description)).wrap());
      ui.add_space(18.0);
      if !entry.actions.is_empty(){theme::section_title(ui,tr("Actions"));ui.add_space(8.0);for action in &entry.actions{ui.label(trf("• {action}",&[("action",&localized(action.name_fr.as_deref(),&action.name))]));}ui.add_space(18.0);}
      egui::Frame::new().fill(p.warning.gamma_multiply(0.12)).stroke(egui::Stroke::new(1.0,p.warning.gamma_multiply(0.65))).corner_radius(theme::RADIUS_MD).inner_margin(egui::Margin::same(16)).show(ui,|ui|{
       ui.label(RichText::new(tr("Communauté — non vérifié")).color(p.warning).strong());
       ui.add(egui::Label::new(tr("Ce plugin exécute du code tiers avec vos droits utilisateur.")).wrap());
      });
      ui.add_space(12.0);
      if ui.link(tr("Page du projet")).clicked(){ctx.open_url(egui::OpenUrl::new_tab(&entry.homepage));}
     }else{
      ui.horizontal_wrapped(|ui|{for (key,title) in [("",tr("Toutes")),("system",category_name("system")),("audio",category_name("audio")),("streaming",category_name("streaming")),("productivity",category_name("productivity")),("smart-home",category_name("smart-home")),("communication",category_name("communication")),("gaming",category_name("gaming")),("utilities",category_name("utilities"))]{if ui.add(egui::Button::new(RichText::new(title).color(if view.category==key{egui::Color32::WHITE}else{p.ink})).fill(if view.category==key{p.accent}else{p.card}).corner_radius(20)).clicked(){view.category=key.into();}}});
      ui.add_space(14.0);
      let mut entries:Vec<_>=state.entries.iter().filter(|e|matching(e,&view.search,&view.category)).collect();
      entries.sort_by(|a,b|a.name.to_lowercase().cmp(&b.name.to_lowercase()));
      if entries.is_empty() && !state.loading{ui.label(tr("Aucun plugin ne correspond à votre recherche."));if ui.button(tr("Proposer un plugin")).clicked(){ctx.open_url(egui::OpenUrl::new_tab(community_plugins::CONTRIBUTE_URL));}}
      let available=ui.available_width();
      let cols=(((available+12.0)/332.0).floor() as usize).clamp(2,5);
      let card_width=((available-(cols-1) as f32*12.0)/cols as f32).max(180.0);
      for row in entries.chunks(cols){ui.horizontal(|ui|{ui.spacing_mut().item_spacing.x=12.0;for entry in row{ui.push_id(&entry.id,|ui|{
       let card=egui::Frame::new().fill(p.card).stroke(egui::Stroke::new(1.0,p.border)).corner_radius(theme::RADIUS_MD).inner_margin(egui::Margin::same(14)).show(ui,|ui|{
        ui.set_width(card_width-28.0);
        // La rangée de cartes est horizontale : le contenu d'une carte s'empile verticalement.
        ui.vertical(|ui|{
        ui.horizontal(|ui|{thumbnail(ui,entry.icon.as_deref(),true,egui::vec2(64.0,64.0));ui.vertical(|ui|{
         ui.set_max_width((card_width-112.0).max(80.0));
         ui.add(egui::Label::new(RichText::new(&entry.name).strong().size(16.0)).truncate());
         ui.add(egui::Label::new(RichText::new(&entry.author).color(p.text_secondary)).truncate());
        });});
        ui.add_space(8.0);
        let summary=localized(entry.summary_fr.as_deref(),entry.summary.as_deref().unwrap_or(&entry.description));
        ui.allocate_ui_with_layout(egui::vec2(card_width-28.0,42.0),egui::Layout::top_down(egui::Align::Min),|ui|{
         ui.set_min_height(42.0);
         ui.add(egui::Label::new(RichText::new(excerpt(summary,72)).color(p.text_secondary)).wrap());
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui|{
         theme::pill(ui,category_name(entry.category.as_deref().unwrap_or("")),p.accent);
         let installed=entry.uuid.as_ref().is_some_and(|uuid|self.plugin_inventory().installed.iter().any(|item|&item.uuid==uuid));
         if installed{theme::pill(ui,tr("Installé"),p.success);}else if !entry.compatible{theme::pill(ui,tr("Incompatible"),p.warning);}
        });
        ui.add_space(8.0);
        if ui.link(tr("Voir la fiche")).clicked(){view.selected=Some(entry.id.clone());}
        });
       });
       let response=ui.interact(card.response.rect,ui.id().with("card_click"),egui::Sense::click());
       if response.hovered(){ui.painter().rect_stroke(card.response.rect,theme::RADIUS_MD,egui::Stroke::new(1.5,p.accent),egui::StrokeKind::Inside);}
       if response.clicked(){view.selected=Some(entry.id.clone());}
      });}});ui.add_space(12.0);}
     }
    });
    drop(state);
    // La confirmation d'installation s'affiche aussi quand le magasin occupe la page.
    self.community_confirmation(ui);
   });
  view.open=open;
  if let Some(url)=view.enlarged.clone(){let mut enlarged=true;crate::theme::fixed_window(tr("Capture du plugin")).open(&mut enlarged).collapsible(false).movable(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).default_size(egui::vec2(800.0,480.0)).show(ctx,|ui|{let width=ui.available_width().min(800.0);thumbnail(ui,Some(&url),false,egui::vec2(width,width*9.0/16.0));});if !enlarged{view.enlarged=None;}}
 }
}
