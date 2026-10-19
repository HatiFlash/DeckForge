use eframe::egui::{
    Button, Color32, CornerRadius, Margin, Response, RichText, Stroke, Style, TextStyle, Ui, Vec2, Visuals,
};

// Unified DeckForge 0.8 Design System Tokens (Dark Navy Theme)
pub const BG_APP: Color32 = Color32::from_rgb(21, 28, 41);       // #151C29 - Main app background
pub const BG_PANEL: Color32 = Color32::from_rgb(29, 39, 55);     // #1D2737 - Sidebar & Header surface
pub const BG_CARD: Color32 = Color32::from_rgb(38, 51, 72);       // #263348 - Action cards, block chips
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(48, 64, 90); // Elevated hover surface
pub const BG_INPUT: Color32 = Color32::from_rgb(17, 24, 38);      // Deep dark input wells

pub const BORDER: Color32 = Color32::from_rgb(52, 68, 94);        // #34445E - Subtle clean borders
pub const BORDER_FOCUS: Color32 = Color32::from_rgb(59, 130, 246);// #3B82F6 - Primary focus border

pub const ACCENT: Color32 = Color32::from_rgb(59, 130, 246);      // #3B82F6 - Action accent
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(96, 165, 250);// Brighter interactive hover
pub const ACCENT_ACTIVE: Color32 = Color32::from_rgb(37, 99, 235);// Pressed state

pub const TEXT: Color32 = Color32::from_rgb(238, 242, 248);       // #EEF2F8 - Crisp high contrast text
pub const TEXT_MUTED: Color32 = Color32::from_rgb(167, 180, 199); // #A7B4C7 - Subtitles & metadata
pub const TEXT_DIM: Color32 = Color32::from_rgb(100, 116, 139);   // Deep muted placeholders

pub const DANGER: Color32 = Color32::from_rgb(239, 68, 68);       // Crimson red error
pub const DANGER_HOVER: Color32 = Color32::from_rgb(248, 113, 113);
pub const SUCCESS: Color32 = Color32::from_rgb(34, 197, 94);      // Emerald green success
pub const WARNING: Color32 = Color32::from_rgb(245, 158, 11);     // Amber warning
pub const AI_PURPLE: Color32 = Color32::from_rgb(139, 92, 246);   // #8B5CF6 - AI & Assistant badge

pub fn apply(style: &mut Style) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_PANEL;
    visuals.override_text_color = Some(TEXT);
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CornerRadius::same(8);

    // Noninteractive widgets (frames, panels)
    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    // Inactive interactive widgets (buttons at rest)
    visuals.widgets.inactive.bg_fill = BG_CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    // Hovered widgets
    visuals.widgets.hovered.bg_fill = BG_CARD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_FOCUS);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);

    // Active (clicked/pressed) widgets
    visuals.widgets.active.bg_fill = ACCENT_ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);

    // Open widgets (combobox dropdown open)
    visuals.widgets.open.bg_fill = BG_CARD;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.corner_radius = CornerRadius::same(6);

    // Selection / highlight
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(59, 130, 246, 60);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    style.visuals = visuals;

    // Spacing (consistent 4/8/12/16 scale)
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.window_margin = Margin::same(12);

    // Typography hierarchy
    style.text_styles.insert(
        TextStyle::Heading,
        eframe::egui::FontId::proportional(16.0),
    );
    style.text_styles.insert(
        TextStyle::Body,
        eframe::egui::FontId::proportional(13.0),
    );
    style.text_styles.insert(
        TextStyle::Button,
        eframe::egui::FontId::proportional(13.0),
    );
    style.text_styles.insert(
        TextStyle::Small,
        eframe::egui::FontId::proportional(11.0),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        eframe::egui::FontId::monospace(12.0),
    );
}

// Shared Design System Helper Widgets
pub fn btn_primary(ui: &mut Ui, text: impl Into<String>) -> Response {
    let btn = Button::new(RichText::new(text.into()).color(Color32::WHITE).strong())
        .fill(ACCENT)
        .corner_radius(CornerRadius::same(6));
    ui.add(btn)
}

pub fn btn_secondary(ui: &mut Ui, text: impl Into<String>) -> Response {
    let btn = Button::new(RichText::new(text.into()).color(TEXT))
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(6));
    ui.add(btn)
}

pub fn btn_danger(ui: &mut Ui, text: impl Into<String>) -> Response {
    let btn = Button::new(RichText::new(text.into()).color(Color32::WHITE))
        .fill(DANGER)
        .corner_radius(CornerRadius::same(6));
    ui.add(btn)
}

pub fn badge_ai(ui: &mut Ui, text: &str) {
    let text = RichText::new(format!(" ✨ {text} "))
        .small()
        .color(Color32::WHITE);
    ui.label(text);
}
