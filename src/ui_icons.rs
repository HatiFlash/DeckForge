use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Destination {
    Key,
    State(bool),
    Audio { path: Vec<usize>, state: usize },
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Target {
    pub profile: usize,
    pub page: String,
    pub key: usize,
    pub destination: Destination,
}
impl Target {
    fn binding<'a>(&self, c: &'a Config) -> Result<&'a Key, String> {
        c.profiles
            .get(self.profile)
            .and_then(|p| p.keys_at(&self.page))
            .and_then(|keys| keys.get(self.key))
            .ok_or_else(|| crate::i18n::tr("La touche n’existe plus. Rouvre la galerie.").into())
    }
    pub(super) fn image<'a>(&self, c: &'a Config) -> Result<&'a str, String> {
        let key = self.binding(c)?;
        match &self.destination {
            Destination::Key => Ok(&key.image),
            Destination::State(active) => Ok(key.state_display.as_ref().and_then(|s| (if *active { &s.on.image } else { &s.off.image }).as_deref()).unwrap_or("")),
            Destination::Audio { path, state } => match step_at(&key.steps, path) {
                Some(Step::AudioSwitch {
                    first_icon,
                    second_icon,
                    ..
                }) => Ok(if *state == 0 { first_icon } else { second_icon }),
                _ => Err(crate::i18n::tr("Le bloc audio a changé. Rouvre la galerie depuis ce bloc.").into()),
            },
        }
    }
    pub(super) fn label(&self, c: &Config) -> String {
        let base = crate::i18n::trf("Touche {key}", &[("key", &format!("{:02}", self.key + 1))]);
        match &self.destination {
            Destination::Key => base,
            Destination::State(active) if self.binding(c).ok().and_then(|k| k.state_display.as_ref()).is_some_and(|s| s.toggle) => {
                let state = if *active { crate::i18n::tr("Activé") } else { crate::i18n::tr("Désactivé") };
                crate::i18n::trf("{base} · {label}", &[("base", &base), ("label", &state)])
            }
            Destination::State(true) => crate::i18n::trf("{base} · Quand c'est vrai", &[("base", &base)]),
            Destination::State(false) => crate::i18n::trf("{base} · Sinon", &[("base", &base)]),
            Destination::Audio { path, state } => {
                let label = self
                    .binding(c)
                    .ok()
                    .and_then(|k| step_at(&k.steps, path))
                    .and_then(|s| {
                        if let Step::AudioSwitch {
                            first_label,
                            second_label,
                            ..
                        } = s
                        {
                            Some(if *state == 0 {
                                first_label.as_str()
                            } else {
                                second_label.as_str()
                            })
                        } else {
                            None
                        }
                    })
                    .unwrap_or("");
                let state_label = if label.is_empty() {
                    if *state == 0 { crate::i18n::tr("État 1") } else { crate::i18n::tr("État 2") }
                } else { label };
                crate::i18n::trf("{base} · {label}", &[("base", &base), ("label", &state_label)])
            }
        }
    }
    pub(super) fn assign(&self, c: &mut Config, image: String) -> Result<(), String> {
        let key = c
            .profiles
            .get_mut(self.profile)
            .and_then(|p| p.keys_at_mut(&self.page))
            .and_then(|keys| keys.get_mut(self.key))
            .ok_or(crate::i18n::tr("La touche n’existe plus. Rouvre la galerie."))?;
        match &self.destination {
            Destination::Key => key.image = image,
            Destination::State(active) => {
                let display = key.state_display.as_mut().ok_or(crate::i18n::tr("Affichage selon l'état désactivé"))?;
                (if *active { &mut display.on } else { &mut display.off }).image = Some(image);
            },
            Destination::Audio { path, state } => match extras::step_mut(&mut key.steps, path) {
                Some(Step::AudioSwitch {
                    first_icon,
                    second_icon,
                    ..
                }) => {
                    *if *state == 0 { first_icon } else { second_icon } = image;
                }
                _ => return Err(crate::i18n::tr("Le bloc audio a changé. Rouvre la galerie depuis ce bloc.").into()),
            },
        }
        Ok(())
    }
}
fn step_at<'a>(steps: &'a [Step], path: &[usize]) -> Option<&'a Step> {
    let step = steps.get(*path.first()?)?;
    if path.len() == 1 {
        return Some(step);
    }
    let branch = *path.get(1)?;
    let children = match step {
        Step::Repeat { steps, .. } if branch == 0 => steps,
        Step::IfWindow {
            then_steps,
            else_steps,
            ..
        }
        | Step::IfVariable {
            then_steps,
            else_steps,
            ..
        } => {
            if branch == 0 {
                then_steps
            } else {
                else_steps
            }
        }
        _ => return None,
    };
    step_at(children, &path[2..])
}
pub(super) fn texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    if path.is_empty() || crate::audio::builtin_kind(path).is_some() {
        return None;
    }
    let id = egui::Id::new(("icon-preview", path));
    if let Some(cached) = ctx.data_mut(|d| d.get_temp::<Option<egui::TextureHandle>>(id)) {
        return cached;
    }
    let loaded = (|| {
        if std::fs::metadata(path).ok()?.len() > 8_000_000 {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let image = crate::assets::decode_image(&bytes, 64).ok()?.into_rgba8();
        Some(ctx.load_texture(
            path,
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                &image,
            ),
            egui::TextureOptions::LINEAR,
        ))
    })();
    ctx.data_mut(|d| d.insert_temp(id, loaded.clone()));
    loaded
}
pub(super) fn selection_border(ui: &egui::Ui, rect: egui::Rect, selected: bool, hovered: bool) {
    if selected {
        ui.painter()
            .rect_filled(rect.expand(2.0), 9.0, accent().gamma_multiply(0.13));
        ui.painter().rect_stroke(
            rect.expand(2.0),
            9.0,
            egui::Stroke::new(2.0_f32, accent()),
            egui::StrokeKind::Inside,
        );
        let c = rect.right_top() + Vec2::new(-9.0, 9.0);
        ui.painter().circle_filled(c, 8.0, accent());
        let stroke = egui::Stroke::new(1.8_f32, Color32::WHITE);
        ui.painter()
            .line_segment([c + Vec2::new(-4.0, 0.0), c + Vec2::new(-1.0, 3.0)], stroke);
        ui.painter()
            .line_segment([c + Vec2::new(-1.0, 3.0), c + Vec2::new(4.0, -3.0)], stroke);
    } else if hovered {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            9.0,
            egui::Stroke::new(1.0_f32, theme::palette(ui.ctx()).muted),
            egui::StrokeKind::Inside,
        );
    }
}
impl App {
    pub(super) fn open_icon_gallery(&mut self, audio: Option<(Vec<usize>, usize)>) {
        let audio = audio.or_else(|| {
            single_audio_path(&self.config.keys()[self.selected].steps).map(|path| {
                let state = match step_at(&self.config.keys()[self.selected].steps, &path) {
                    Some(Step::AudioSwitch { second, input, .. })
                        if second
                            == if *input {
                                &self.audio.input
                            } else {
                                &self.audio.output
                            } =>
                    {
                        1
                    }
                    _ => 0,
                };
                (path, state)
            })
        });
        let target = Target {
            profile: self.config.active,
            page: self.config.page.clone(),
            key: self.selected,
            destination: audio
                .map(|(path, state)| Destination::Audio { path, state })
                .unwrap_or(Destination::Key),
        };
        self.icon_selected = target
            .image(&self.config)
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        self.icon_notice = String::new();
        self.icon_target = Some(target);
        self.asset_tab = 0;
        self.reload_icons();
        self.show_assets = true;
    }
    pub(super) fn open_state_icon_gallery(&mut self, active: bool) {
        self.open_icon_gallery(None);
        let target = Target { profile: self.config.active, page: self.config.page.clone(),
            key: self.selected, destination: Destination::State(active) };
        self.icon_selected = target.image(&self.config).ok().filter(|s| !s.is_empty()).map(PathBuf::from);
        self.icon_target = Some(target);
    }
    pub(super) fn retarget_gallery(&mut self) {
        if !self.show_assets { return; }
        if let Some(target) = &self.icon_target {
            if target.profile == self.config.active && target.page == self.config.page && target.key == self.selected { return; }
        }
        let icons = self.available_icons.clone();
        self.open_icon_gallery(None);
        self.available_icons = icons;
    }
    pub(super) fn asset_browser(&mut self, ctx: &egui::Context) {
        if !self.show_assets {
            return;
        }
        if self.icon_target.is_none() {
            self.open_icon_gallery(None);
        }
        self.retarget_gallery();
        let target = self.icon_target.clone().unwrap();
        let title = target.label(&self.config);
        let selected = self.icon_selected.clone();
        let mut choose = None;
        let mut done = false;
        let mut open = true;
        let mut import = false;
        let mut import_icon = false;
        let mut folder = false;
        let mut rescan = false;
        let mut marketplace = false;
        let mut switch_target = None;
        egui::Window::new(crate::i18n::tr("Icônes et extensions")).open(&mut open).default_width(780.0).default_height(610.0).show(ctx,|ui|{
            ui.horizontal(|ui|{if iconset::tab(ui,self.asset_tab==0,Ico::Picture,crate::i18n::tr("Icônes")).clicked(){self.asset_tab=0;}if iconset::tab(ui,self.asset_tab==1,Ico::Plugin,crate::i18n::tr("Plugins installés")).clicked(){self.asset_tab=1;}ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.button(crate::i18n::tr("Terminé")).clicked(){done=true;}if ui.button(crate::i18n::tr("Marketplace Elgato ↗")).clicked(){marketplace=true;}});});
            if self.asset_tab==0{
                ui.label(RichText::new(crate::i18n::trf("Choisir une icône · {title}", &[("title", &title)])).strong());
                if let Destination::Audio{path,state}=&target.destination{
                    if let Ok(key)=target.binding(&self.config){if let Some(Step::AudioSwitch{first_label,second_label,..})=step_at(&key.steps,path){
                        ui.horizontal(|ui|{for(i,label)in [first_label,second_label].into_iter().enumerate(){if ui.selectable_label(*state==i,crate::i18n::trf("État {state} · {label}", &[("state", &(i+1)), ("label", &label)])).clicked(){let mut next=target.clone();next.destination=Destination::Audio{path:path.clone(),state:i};switch_target=Some(next);}}});
                    }}
                    ui.horizontal(|ui|{for(id,label,kind)in crate::audio::BUILTIN_ICONS{
                        let(r,response)=ui.allocate_exact_size(Vec2::new(96.0,65.0),egui::Sense::click());theme::symbol(ui.painter(),r.center_top()+Vec2::new(0.0,24.0),27.0,*kind,accent());ui.painter().text(r.center_bottom()-Vec2::new(0.0,3.0),egui::Align2::CENTER_BOTTOM,*label,egui::FontId::proportional(11.0),theme::palette(ctx).ink);
                        selection_border(ui,r,selected.as_deref()==Some(std::path::Path::new(id)),response.hovered());if response.clicked(){choose=Some((id.to_string(),label.to_string()));}
                    }});
                }
                ui.horizontal_wrapped(|ui|{if ui.add(theme::primary(RichText::new(crate::i18n::tr("Importer une icône…")).color(Color32::WHITE))).on_hover_text(crate::i18n::tr("Image, GIF ou vidéo depuis ton PC, appliquée à cette touche")).clicked(){import_icon=true;}if ui.button(crate::i18n::tr("Importer un pack…")).clicked(){import=true;}if ui.button(crate::i18n::tr("Parcourir un dossier…")).clicked(){folder=true;}if ui.button(crate::i18n::tr("Actualiser")).clicked(){rescan=true;}ui.label(crate::i18n::trf("{count} icônes", &[("count", &self.available_icons.len())]));});
                ui.add(egui::TextEdit::singleline(&mut self.icon_search).hint_text(crate::i18n::tr("Rechercher un nom ou un pack…")).desired_width(f32::INFINITY));
                let mut packs=self.available_icons.iter().map(|i|i.pack.clone()).collect::<Vec<_>>();packs.sort();packs.dedup();
                egui::ComboBox::from_id_salt("icon_pack").selected_text(if self.icon_pack.is_empty(){crate::i18n::tr("Tous les packs")}else{&self.icon_pack}).show_ui(ui,|ui|{ui.selectable_value(&mut self.icon_pack,String::new(),crate::i18n::tr("Tous les packs"));for name in packs{ui.selectable_value(&mut self.icon_pack,name.clone(),name);}});
                let query=self.icon_search.to_lowercase();let items=self.available_icons.iter().filter(|i|(self.icon_pack.is_empty()||self.icon_pack==i.pack)&&format!("{} {}",i.pack,i.name).to_lowercase().contains(&query)).cloned().collect::<Vec<_>>();
                let columns=((ui.available_width()/92.0)as usize).max(1);let rows=items.len().div_ceil(columns);
                egui::ScrollArea::vertical().id_salt("icons04").max_height(310.0).show_rows(ui,94.0,rows,|ui,range|{for row in range{ui.horizontal(|ui|{for icon in items.iter().skip(row*columns).take(columns){
                    let(r,response)=ui.allocate_exact_size(Vec2::new(82.0,88.0),egui::Sense::click());let path=icon.path.to_string_lossy().into_owned();
                    if let Some(texture)=texture(ctx,&path){ui.painter().image(texture.id(),egui::Rect::from_center_size(r.center_top()+Vec2::new(0.0,33.0),Vec2::splat(62.0)),egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.0,1.0)),Color32::WHITE);}
                    ui.painter().text(r.center_bottom(),egui::Align2::CENTER_BOTTOM,icon.name.chars().take(13).collect::<String>(),egui::FontId::proportional(10.0),theme::palette(ctx).ink);
                    if icon.path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("gif")) {
                        let badge=egui::Rect::from_min_size(r.left_top()+Vec2::new(4.0,4.0),Vec2::new(27.0,15.0));
                        ui.painter().rect_filled(badge,3.0,accent());
                        ui.painter().text(badge.center(),egui::Align2::CENTER_CENTER,"GIF",egui::FontId::proportional(10.0),Color32::WHITE);
                    }
                    selection_border(ui,r,selected.as_deref()==Some(icon.path.as_path()),response.hovered());
                    if response.clicked(){choose=Some((path,icon.name.clone()));}response.on_hover_text(format!("{}\n{}",icon.name,icon.pack));
                }});}});
                if self.icon_notice.is_empty(){ui.label(RichText::new(crate::i18n::tr("Un clic applique l’icône immédiatement. Le contour bleu et la coche indiquent la sélection.")).small());}else{ui.colored_label(GREEN,&self.icon_notice);}
                ui.label(RichText::new(crate::i18n::tr("Packs locaux FIFINE et Elgato · PNG, JPEG et SVG. Pour de nouveaux packs, utilise le Marketplace puis actualise ou importe le fichier.")).small());
            }else{
                ui.label(crate::i18n::tr("Inventaire local des plugins FIFINE et Elgato. Aucun plugin n’est lancé pendant l’analyse."));ui.colored_label(Color32::from_rgb(228,165,82),crate::i18n::tr("Les plugins tiers ne sont pas encore exécutables. L’import de profils convertit les actions prises en charge."));
                egui::ScrollArea::vertical().max_height(440.0).show(ui,|ui|{for p in &self.available_plugins{ui.group(|ui|{ui.label(RichText::new(&p.name).strong());ui.label(crate::i18n::trf("{count} actions · {id}", &[("count", &p.actions), ("id", &p.id)]));ui.label(RichText::new(p.path.to_string_lossy()).small());});}});
            }
        });
        self.show_assets = open && !done;
        if let Some(target) = switch_target {
            self.icon_selected = target
                .image(&self.config)
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from);
            self.icon_target = Some(target);
            self.icon_notice.clear();
        }
        if let Some((path, name)) = choose {
            if let Err(e) = self.choose_icon(&path, &name) {
                self.fail(e);
            }
            ctx.request_repaint();
        }
        if marketplace {
            let _ = std::process::Command::new("explorer.exe")
                .arg("https://marketplace.elgato.com/stream-deck")
                .spawn();
        }
        if rescan {
            self.reload_icons();
        }
        if folder {
            let tx=self.dialog_tx.clone();let assets=self.path.parent().unwrap().join("assets");
            std::thread::spawn(move || {
                if let Some(path)=rfd::FileDialog::new().pick_folder() {
                    let icons=crate::assets::scan(&[path,assets]);
                    let _=tx.send(Some(Box::new(move |app: &mut App| app.available_icons=icons) as DialogEffect));
                }
            });
        }
        if import_icon {
            let tx=self.dialog_tx.clone();
            let assets=self.path.parent().unwrap().join("assets/mes-icones");
            let size=self.config.image_size;
            let target=target.clone();
            std::thread::spawn(move || {
                let Some(path)=rfd::FileDialog::new()
                    .add_filter(crate::i18n::tr("Images et vidéos"), &["png","jpg","jpeg","webp","bmp","ico","svg","gif","mp4","webm","mov","mkv","avi"])
                    .pick_file() else {return};
                let svg=path.extension().is_some_and(|e|e.eq_ignore_ascii_case("svg"));
                if matches!(target.destination, Destination::Key) && !svg {
                    // Touche entière : passer par l'éditeur de recadrage (aperçu avant validation).
                    let _=tx.send(Some(Box::new(move |app: &mut App| {
                        app.show_assets=false;
                        app.open_crop_editor(target.profile, target.page.clone(), target.key, path, size);
                    }) as DialogEffect));
                    return;
                }
                let imported: Result<PathBuf,String> = if crate::assets::is_video(&path) {
                    crate::assets::import_video(&path,&assets,size).map_err(|e|e.to_string())
                } else {
                    Ok(path)
                };
                let _=tx.send(Some(Box::new(move |app: &mut App| {
                    let result=imported.and_then(|p|app.import_icon_from(&target,&p));
                    if let Err(e)=result {app.fail(crate::i18n::trf("Import impossible : {error}", &[("error", &e)]));}
                }) as DialogEffect));
            });
        }
        if import {
            let tx=self.dialog_tx.clone();let assets=self.path.parent().unwrap().join("assets/mes-icones");
            let size=self.config.image_size;
            std::thread::spawn(move || {
                let Some(path)=rfd::FileDialog::new()
                    .add_filter(crate::i18n::tr("Images, vidéos et packs"), &["png","jpg","jpeg","svg","gif","mp4","webm","mov","mkv","avi","zip","streamDeckIconPack","sdIconPack"])
                    .pick_file() else {return};
                let video=crate::assets::is_video(&path);
                if video { let _=tx.send(Some(Box::new(|app: &mut App| app.log(crate::i18n::tr("Conversion de la vidéo…").into())) as DialogEffect)); }
                let result: Result<String, String> = if video {
                    crate::assets::import_video(&path,&assets,size).map(|_| crate::i18n::tr("Vidéo convertie en GIF animé.").into()).map_err(|e|e.to_string())
                } else if path.extension().is_some_and(|e| ["zip","streamDeckIconPack","sdIconPack"].iter().any(|x| e.eq_ignore_ascii_case(x))) {
                    crate::assets::import_pack(&path,&assets).map(|(count,skipped)|crate::i18n::trf("{count} icônes importées - {skipped} images non prises en charge.", &[("count", &count), ("skipped", &skipped)])).map_err(|e|e.to_string())
                } else {
                    std::fs::read(&path).map_err(|e|e.to_string()).and_then(|bytes|
                        crate::assets::import_image(&bytes,&assets,path.file_stem().and_then(|s|s.to_str()).unwrap_or("icone"))
                            .map(|_| crate::i18n::tr("Icône importée.").into()).map_err(|e|e.to_string()))
                };
                let icons=if result.is_ok(){Some(crate::assets::scan(&[assets]))}else{None};
                let _=tx.send(Some(Box::new(move |app: &mut App| match result {
                    Ok(message)=>{if let Some(icons)=icons {app.available_icons=icons;} app.log(message);}
                    Err(e)=>app.fail(crate::i18n::trf("Import impossible : {error}", &[("error", &e)])),
                }) as DialogEffect));
            });
        }
    }
    /// Copie une image de l'utilisateur dans assets/mes-icones (sans écraser) et l'applique
    /// à la cible de la galerie. Les GIF déjà convertis depuis une vidéo sont repris tels quels.
    pub(super) fn import_icon_from(&mut self, target: &Target, source: &std::path::Path) -> Result<PathBuf, String> {
        target.image(&self.config)?;
        let assets = self.path.parent().unwrap().join("assets/mes-icones");
        let name = source.file_stem().and_then(|s| s.to_str()).unwrap_or("icone").to_owned();
        let stored = if source.starts_with(&assets) {
            source.to_path_buf()
        } else {
            let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
            crate::assets::import_image(&bytes, &assets, &name).map_err(|e| e.to_string())?
        };
        target.assign(&mut self.config, stored.to_string_lossy().into_owned())?;
        self.icon_selected = Some(stored.clone());
        self.edited();
        self.reload_icons();
        self.icon_notice = crate::i18n::trf("Icône « {name} » importée et appliquée · {target}", &[("name", &name), ("target", &target.label(&self.config))]);
        self.log(self.icon_notice.clone());
        Ok(stored)
    }
    pub(super) fn choose_icon(&mut self, path: &str, name: &str) -> Result<(), String> {
        let target = self
            .icon_target
            .clone()
            .ok_or(crate::i18n::tr("Sélectionne d’abord une touche."))?;
        // Check the destination before copying. No assignment can silently target another key.
        target.image(&self.config)?;
        let assigned = if crate::audio::builtin_kind(path).is_some() {
            if matches!(target.destination, Destination::Key) {
                return Err(crate::i18n::tr("Les icônes d’état se choisissent depuis le bloc audio.").into());
            }
            path.into()
        } else {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            crate::assets::import_image(
                &bytes,
                &self.path.parent().unwrap().join("assets/mes-icones"),
                name,
            )
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned()
        };
        target.assign(&mut self.config, assigned)?;
        self.icon_selected = Some(PathBuf::from(path));
        self.edited();
        self.icon_notice = crate::i18n::trf(
            "Icône « {name} » appliquée · {target}",
            &[("name", &name), ("target", &target.label(&self.config))]
        );
        self.log(self.icon_notice.clone());
        Ok(())
    }
}
fn single_audio_path(steps: &[Step]) -> Option<Vec<usize>> {
    fn collect(steps: &[Step], parent: &[usize], paths: &mut Vec<Vec<usize>>) {
        for (i, s) in steps.iter().enumerate() {
            let mut path = parent.to_vec();
            path.push(i);
            match s {
                Step::AudioSwitch { .. } => paths.push(path),
                Step::Repeat { steps, .. } => {
                    path.push(0);
                    collect(steps, &path, paths);
                }
                Step::IfWindow {
                    then_steps,
                    else_steps,
                    ..
                }
                | Step::IfVariable {
                    then_steps,
                    else_steps,
                    ..
                } => {
                    let mut branch = path.clone();
                    branch.push(0);
                    collect(then_steps, &branch, paths);
                    path.push(1);
                    collect(else_steps, &path, paths);
                }
                _ => {}
            }
        }
    }
    let mut found = vec![];
    collect(steps, &[], &mut found);
    if found.len() == 1 {
        found.pop()
    } else {
        None
    }
}

/// Both state icons remain visible side by side, even in the compact inspector.
pub(super) fn audio_options(
    ui: &mut egui::Ui,
    step: &mut Step,
    audio: &crate::audio::Snapshot,
    pick: &mut Option<usize>,
    tx: &Sender<Option<DialogEffect>>,
    target: &StepTarget,
) -> bool {
    let Step::AudioSwitch {
        first,
        second,
        input,
        first_label,
        second_label,
        first_icon,
        second_icon,
    } = step
    else {
        return false;
    };
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        ui.spacing_mut().interact_size.y = 24.0;
        let mut changed = false;
        let palette = theme::palette(ui.ctx());
        ui.horizontal(|ui| {
            ui.label("Audio");
            changed |= ui.selectable_value(input, false, crate::i18n::tr("Sortie")).changed();
            changed |= ui.selectable_value(input, true, crate::i18n::tr("Micro / entrée")).changed();
        });
        let active = if *input { &audio.input } else { &audio.output };
        ui.columns(2, |columns| {
            for ((state, id, label, icon), column) in [
                (0, first, first_label, first_icon),
                (1, second, second_label, second_icon),
            ]
            .into_iter()
            .zip(columns)
            {
                column.push_id(state, |ui| {
                    egui::Frame::new()
                        .fill(palette.bg)
                        .corner_radius(10)
                        .inner_margin(6)
                        .show(ui, |ui| {
                            let kind = crate::audio::builtin_kind(icon).unwrap_or(if *input {
                                28
                            } else if state == 0 {
                                26
                            } else {
                                27
                            });
                            ui.horizontal(|ui| {
                                let (r, response) =
                                    ui.allocate_exact_size(Vec2::splat(43.0), egui::Sense::click());
                                ui.painter().rect_filled(r, 8.0, palette.tile);
                                if let Some(texture) = texture(ui.ctx(), icon) {
                                    ui.painter().image(
                                        texture.id(),
                                        r.shrink(5.0),
                                        egui::Rect::from_min_max(
                                            egui::Pos2::ZERO,
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                } else {
                                    theme::symbol(ui.painter(), r.center(), 25.0, kind, accent());
                                }
                                if response.clicked() {
                                    *pick = Some(state);
                                }
                                response.on_hover_text(crate::i18n::tr("Choisir l’icône de cet état"));
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(crate::i18n::trf(
                                            "État {state}{active}",
                                            &[("state", &(state + 1)), ("active", &(if !id.is_empty() && active == id { crate::i18n::tr(" · Actif") } else { "" }))]
                                        ))
                                        .small()
                                        .strong(),
                                    );
                                    changed |= ui
                                        .add(
                                            egui::TextEdit::singleline(label).desired_width(
                                                (ui.available_width() - 2.0).max(60.0),
                                            ),
                                        )
                                        .changed();
                                });
                            });
                            egui::ComboBox::from_id_salt("endpoint")
                                .width((ui.available_width() - 18.0).max(100.0))
                                .selected_text(
                                    audio
                                        .devices
                                        .iter()
                                        .find(|d| d.id == *id)
                                        .map(|d| d.name.as_str())
                                        .unwrap_or(crate::i18n::tr("Choisir le périphérique…")),
                                )
                                .show_ui(ui, |ui| {
                                    for d in audio.devices.iter().filter(|d| d.input == *input) {
                                        changed |= ui
                                            .selectable_value(id, d.id.clone(), &d.name)
                                            .changed();
                                    }
                                });
                            ui.horizontal_wrapped(|ui| {
                                if ui.small_button(crate::i18n::tr("Galerie")).clicked() {
                                    *pick = Some(state);
                                }
                                if ui.small_button(crate::i18n::tr("Fichier…")).clicked() {
                                    let tx=tx.clone();let target=target.clone();
                                    std::thread::spawn(move || {
                                        let path=rfd::FileDialog::new().add_filter(crate::i18n::tr("Image"), &["png","jpg","jpeg"]).pick_file();
                                        let _=tx.send(path.map(|path| Box::new(move |app: &mut App| {
                                            let (profile,page,key,step_path)=target;
                                            if let Some(Step::AudioSwitch {first_icon,second_icon,..})=
                                                app.config.profiles.get_mut(profile).and_then(|p|p.keys_at_mut(&page))
                                                .and_then(|keys|keys.get_mut(key)).and_then(|key|extras::step_mut(&mut key.steps,&step_path)) {
                                                *(if state==0 {first_icon} else {second_icon})=path.to_string_lossy().into_owned();
                                                app.edited();
                                            }
                                        }) as DialogEffect));
                                    });
                                }
                                if ui.small_button(crate::i18n::tr("Par défaut")).clicked() {
                                    icon.clear();
                                    changed = true;
                                }
                            });
                        });
                });
            }
        });
        if !audio.error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, &audio.error);
        }
        changed
    })
    .inner
}
