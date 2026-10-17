use super::*;

fn icon_button(ui: &mut egui::Ui, selected: bool, label: &str, import: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect_filled(
        rect,
        6.0,
        if selected {
            BLUE.gamma_multiply(0.25)
        } else {
            visuals.bg_fill
        },
    );
    ui.painter().rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(
            1.0_f32,
            if selected {
                BLUE
            } else {
                visuals.bg_stroke.color
            },
        ),
        egui::StrokeKind::Inside,
    );
    let ink = if selected {
        BLUE
    } else {
        theme::palette(ui.ctx()).ink
    };
    let stroke = egui::Stroke::new(1.5_f32, ink);
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y);
    if import {
        ui.painter().rect_stroke(
            egui::Rect::from_min_max(at(6.0, 7.0), at(22.0, 23.0)),
            2.0,
            stroke,
            egui::StrokeKind::Inside,
        );
        ui.painter().circle_filled(at(11.0, 12.0), 1.5, ink);
        ui.painter()
            .line_segment([at(8.0, 21.0), at(14.0, 16.0)], stroke);
        ui.painter()
            .line_segment([at(14.0, 16.0), at(18.0, 20.0)], stroke);
        ui.painter().rect_filled(
            egui::Rect::from_min_max(at(18.0, 3.0), at(28.0, 13.0)),
            3.0,
            visuals.bg_fill,
        );
        ui.painter()
            .line_segment([at(19.0, 8.0), at(27.0, 8.0)], stroke);
        ui.painter()
            .line_segment([at(23.0, 4.0), at(23.0, 12.0)], stroke);
    } else {
        // Tilted paint bucket, drop and slash; no dependency on emoji fonts.
        for (a, b) in [
            ((8.0, 9.0), (14.0, 5.0)),
            ((14.0, 5.0), (23.0, 14.0)),
            ((23.0, 14.0), (15.0, 22.0)),
            ((15.0, 22.0), (6.0, 13.0)),
            ((6.0, 13.0), (8.0, 9.0)),
            ((7.0, 13.0), (21.0, 13.0)),
        ] {
            ui.painter()
                .line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
        }
        ui.painter().circle_filled(at(24.0, 23.0), 2.0, ink);
        ui.painter().line_segment(
            [at(5.0, 25.0), at(26.0, 4.0)],
            egui::Stroke::new(2.0_f32, ink),
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response.on_hover_text(label)
}

pub(super) fn color(ui: &mut egui::Ui, key: &mut Key) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Couleur");
        changed |= ui
            .add_enabled_ui(key.background, |ui| {
                ui.color_edit_button_srgb(&mut key.color)
            })
            .inner
            .changed();
        if icon_button(
            ui,
            !key.background,
            if key.background {
                "Sans fond coloré · écran noir"
            } else {
                "Rétablir la couleur mémorisée"
            },
            false,
        )
        .clicked()
        {
            key.background = !key.background;
            changed = true;
        }
    });
    changed
}

#[derive(Default)]
pub(super) struct IconActions {
    pub gallery: bool,
    pub pick_file: bool,
    pub clear: bool,
}
pub(super) fn icons(ui: &mut egui::Ui, has_image: bool) -> IconActions {
    let mut actions = IconActions::default();
    ui.horizontal(|ui| {
        let buttons = if has_image { 2.0 } else { 1.0 };
        let width = (ui.available_width() - buttons * (30.0 + ui.spacing().item_spacing.x)).max(80.0);
        actions.gallery = ui
            .add_sized(
                Vec2::new(width, 30.0),
                theme::primary(RichText::new("Galerie").color(Color32::WHITE)),
            )
            .on_hover_text("Choisir dans les packs FIFINE, Elgato et les icônes locales")
            .clicked();
        actions.pick_file = icon_button(
            ui,
            false,
            "Importer une icône depuis le PC (PNG, JPEG, SVG)",
            true,
        )
        .clicked();
        if has_image {
            actions.clear = ui
                .add_sized(Vec2::new(30.0, 30.0), egui::Button::new("×"))
                .on_hover_text("Retirer l’image de la touche")
                .clicked();
        }
    });
    actions
}
