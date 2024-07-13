use std::collections::HashMap;

use egui::Widget;
use hyperpuzzle::{ColorSystem, DefaultColor, Rgb};
use strum::IntoEnumIterator;

use crate::{
    gui::util::set_widget_spacing_to_space_width,
    preferences::{ColorScheme, DefaultColorGradient, GlobalColorPalette},
    util::BeforeOrAfter,
};

use super::{DragAndDropResponse, TextEditPopup};

/// Pixel resolution of gradients.
const GRADIENT_RESOLUTION: usize = 1;

/// Factor by which gradients in tooltips are wider than single colors.
const GRADIENT_WIDTH_MULTIPLIER: f32 = 5.0;
/// Factor by which gradients in headers are taller than single colors.
const GRADIENT_HEIGHT_MULTIPLIER: f32 = 1.5;
/// Factor by which gradients in headers are taller than single colors when they
/// do not need to be interacted with.
const GRADIENT_COMPACT_HEIGHT_MULTIPLIER: f32 = 0.5;

/// Rounding of the colored box in the big color preview tooltip.
const TOOLTIP_COLOR_RECT_ROUNDING: f32 = 3.0;

fn show_color_schemes_help_ui(allow_dragging: bool) -> impl FnOnce(&mut egui::Ui) {
    move |ui| {
        // TODO: markdown renderer
        ui.spacing_mut().item_spacing.y = 9.0;
        ui.heading("Color assignments");
        ui.label("Each facet on the puzzle is assigned a different color.");
        if allow_dragging {
            ui.label("Drag a facet name to assign a different color to it.");
        }
        crate::gui::util::bullet_list(
            ui,
            // TODO: rewrite this explanation
            &[
                "Single colors are best for small puzzles",
                "Color sets are best for medium puzzles",
                "Gradients are best for large puzzles",
                "Colors within a color set are designed to contrast with \
                each other and with other color sets of the same size",
            ],
        );
        ui.horizontal_wrapped(|ui| {
            set_widget_spacing_to_space_width(ui);
            ui.label("Color values can be customized in the");
            ui.strong("global color palette");
            ui.label("settings.");
        });
    }
}

#[derive(Debug)]
pub struct ColorsUi<'a> {
    default_color_to_puzzle_color: HashMap<DefaultColor, String>,
    gradient_totals: HashMap<DefaultColorGradient, usize>,
    palette: &'a GlobalColorPalette,

    pub clickable: bool,
    pub show_puzzle_colors: bool,
    dnd: Option<super::DragAndDrop<String, DefaultColor>>,

    hovered_color: Option<DefaultColor>,
    clicked_color: Option<DefaultColor>,
}
impl<'a> ColorsUi<'a> {
    pub fn new(palette: &'a GlobalColorPalette) -> Self {
        Self {
            default_color_to_puzzle_color: HashMap::new(),
            gradient_totals: HashMap::new(),
            palette,

            show_puzzle_colors: false,
            clickable: false,
            dnd: None,

            hovered_color: None,
            clicked_color: None,
        }
    }

    pub fn clickable(mut self, clickable: bool) -> Self {
        self.clickable = clickable;
        self
    }
    pub fn drag_puzzle_colors(mut self, ui: &mut egui::Ui, drag_puzzle_colors: bool) -> Self {
        if drag_puzzle_colors {
            self.dnd = Some(super::DragAndDrop::new(ui));
            self.show_puzzle_colors(true)
        } else {
            self.dnd = None;
            self
        }
    }
    pub fn show_puzzle_colors(mut self, show_puzzle_colors: bool) -> Self {
        self.show_puzzle_colors = show_puzzle_colors;
        self
    }

    fn click_zone(&mut self, r: &egui::Response, color: &DefaultColor) {
        if self.clickable {
            if r.hovered() {
                self.hovered_color = Some(color.clone());
            }
            if r.clicked() {
                self.clicked_color = Some(color.clone());
            }
        }
    }
    fn drag_drop_zone(&mut self, ui: &mut egui::Ui, r: &egui::Response, color: &DefaultColor) {
        if let Some(dnd) = &mut self.dnd {
            dnd.drop_zone(ui, r, color.clone());
        }
    }
    fn reorder_drag_drop_zone(
        &mut self,
        ui: &mut egui::Ui,
        r: &egui::Response,
        color: &DefaultColor,
    ) {
        if let Some(dnd) = &mut self.dnd {
            dnd.reorder_drop_zone(ui, r, color.clone());
        }
    }

    fn is_dragging(&self) -> bool {
        self.dnd.as_ref().is_some_and(|dnd| dnd.is_dragging())
    }

    fn update_reverse_color_map(&mut self, color_scheme: &ColorScheme) {
        // Construct a reverse map from default color to puzzle color. This
        // assumes that the color scheme is already valid.
        for (color_name, default_color) in color_scheme {
            self.default_color_to_puzzle_color
                .insert(default_color.clone(), color_name.clone());

            // Record gradient totals
            if let DefaultColor::Gradient {
                gradient_name,
                index: 0,
                total,
            } = default_color
            {
                if let Ok(g) = gradient_name.parse() {
                    self.gradient_totals.insert(g, *total);
                }
            }
        }
    }

    /// Shows a compact view of the global color palette, with optional labels
    /// that can be dragged to reassign colors.
    ///
    /// Returns a boolean indicating whether any modification was made to the
    /// global palette, along with an optional temporary color scheme to allow
    /// for just the next frame.
    pub fn show_compact_palette(
        &mut self,
        ui: &mut egui::Ui,
        current_colors: Option<(&mut ColorScheme, &ColorSystem)>,
        color_to_modify: Option<DefaultColor>,
    ) -> (bool, Option<ColorScheme>) {
        self.default_color_to_puzzle_color = HashMap::new();
        self.gradient_totals = HashMap::new();
        if let Some((color_scheme, _color_system)) = &current_colors {
            self.update_reverse_color_map(color_scheme);
        }

        let large_space = ui.spacing().item_spacing.x;
        let small_space = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = large_space;
        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();

        ui.horizontal(|ui| {
            ui.strong("Single colors");
            let allow_dragging = self.dnd.is_some();
            let show_help_ui = show_color_schemes_help_ui(allow_dragging);
            crate::gui::components::HelpHoverWidget::show(ui, show_help_ui);
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.y = ui.spacing().item_spacing.x;
            for color_name in self.palette.singles.keys() {
                self.show_single_color(ui, color_name.clone());
            }
        });

        if !self.palette.custom_singles.is_empty() {
            ui.separator();
            ui.strong("Custom colors");
            ui.add_space(ui.spacing().item_spacing.x - ui.spacing().item_spacing.x);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.y = ui.spacing().item_spacing.x;
                for color_name in self.palette.custom_singles.keys() {
                    self.show_single_color(ui, color_name.clone());
                }
            });
        }

        ui.separator();
        egui::ScrollArea::horizontal()
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut is_first = true;
                    for (group_name, sets) in self.palette.groups_of_sets() {
                        if is_first {
                            is_first = false;
                        } else {
                            ui.separator();
                        }
                        ui.vertical(|ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::from(group_name).strong())
                                    .wrap(false),
                            );
                            ui.spacing_mut().item_spacing.x = small_space;
                            for (set_name, _set) in sets {
                                self.show_color_set(ui, set_name);
                            }
                        });
                    }
                })
                .response
            })
            .inner;

        ui.separator();

        ui.strong("Gradients");
        for gradient in DefaultColorGradient::iter() {
            self.show_color_gradient(ui, gradient);
        }

        let mut temp_modification = None;
        let mut modification = None;

        if let Some((color_scheme, color_system)) = current_colors {
            if let Some(dnd) = &mut self.dnd {
                dnd.paint_reorder_drop_lines(ui);
                temp_modification = dnd.mid_drag().cloned();
                modification = dnd.end_drag();
            } else if let Some(color_to_modify) = color_to_modify
                .and_then(|c| self.default_color_to_puzzle_color.get(&c))
                .cloned()
            {
                if let Some(hovered_color) = self.hovered_color.take() {
                    temp_modification = Some(DragAndDropResponse {
                        payload: color_to_modify.clone(),
                        end: hovered_color,
                        before_or_after: None,
                    });
                }
                if let Some(clicked_color) = self.clicked_color.take() {
                    modification = Some(DragAndDropResponse {
                        payload: color_to_modify,
                        end: clicked_color,
                        before_or_after: None,
                    });
                }
            }

            let changed = modification.is_some();
            if let Some(drag) = modification {
                self.apply_drag(color_scheme, color_system, drag);
            }

            let temp_scheme = temp_modification.map(|drag| {
                let mut temp = color_scheme.clone();
                self.apply_drag(&mut temp, color_system, drag);
                temp
            });

            (changed, temp_scheme)
        } else {
            (false, None)
        }
    }

    fn apply_drag(
        &self,
        map: &mut ColorScheme,
        color_system: &ColorSystem,
        drag: DragAndDropResponse<String, DefaultColor>,
    ) {
        match drag.before_or_after {
            Some(before_or_after) => {
                self.reorder_color_to(map, drag.payload, drag.end, before_or_after)
            }
            None => self.swap_color_to(map, drag.payload, drag.end),
        }
        let _ = self
            .palette
            .ensure_color_scheme_is_valid_for_color_system(map, color_system);
    }
    fn reorder_color_to(
        &self,
        map: &mut ColorScheme,
        name: String,
        new_default_color: DefaultColor,
        before_or_after: BeforeOrAfter,
    ) {
        let DefaultColor::Gradient {
            gradient_name,
            mut index,
            total: _,
        } = &new_default_color
        else {
            log::error!("attempt to reorder color to something other than a gradient");
            return;
        };

        if before_or_after == BeforeOrAfter::After {
            index += 1;
        }

        let Ok(gradient) = gradient_name.parse::<DefaultColorGradient>() else {
            log::error!("unknown gradient name {gradient_name:?}");
            return;
        };

        // Shift other colors up by one.
        let total = *self.gradient_totals.get(&gradient).unwrap_or(&0);
        for i in index..total {
            if let Some(name) = self
                .default_color_to_puzzle_color
                .get(&DefaultColor::Gradient {
                    gradient_name: gradient_name.clone(),
                    index: i,
                    total,
                })
            {
                map.insert(
                    name.clone(),
                    DefaultColor::Gradient {
                        gradient_name: gradient_name.clone(),
                        index: i + 1,
                        total: total + 1,
                    },
                );
            }
        }

        // Insert the new color.
        map.insert(name, new_default_color);
    }
    fn swap_color_to(&self, map: &mut ColorScheme, name: String, new_default_color: DefaultColor) {
        let old_name = self.default_color_to_puzzle_color.get(&new_default_color);
        let old_default_color = map.insert(name, new_default_color);

        if let Some(old_default_color) = old_default_color {
            if let Some(old_name) = old_name {
                map.insert(old_name.clone(), old_default_color);
            }
        }
    }

    fn show_single_color(&mut self, ui: &mut egui::Ui, color_name: String) {
        crate::gui::util::wrap_if_needed_for_color_button(ui);

        let tooltip_pos = ui.cursor().left_top();
        let default_color = DefaultColor::Single { name: color_name };
        let r = self.show_generic_color(ui, &default_color, tooltip_pos);
        self.click_zone(&r, &default_color);
        self.drag_drop_zone(ui, &r, &default_color);
    }

    fn show_color_set(&mut self, ui: &mut egui::Ui, color_set_name: &str) {
        let tooltip_pos = ui.cursor().left_top();
        let Some(color_set) = self.palette.get_set(color_set_name) else {
            super::error_label(ui, format!("missing color set {color_set_name:?}"));
            return;
        };

        ui.horizontal(|ui| {
            set_tight_spacing(ui);

            for i in 0..color_set.len() {
                let default_color = DefaultColor::Set {
                    set_name: color_set_name.to_string(),
                    index: i,
                };
                let r = self.show_generic_color(ui, &default_color, tooltip_pos);
                self.click_zone(&r, &default_color);
                self.drag_drop_zone(ui, &r, &default_color);
            }
        });
    }

    fn show_color_gradient(&mut self, ui: &mut egui::Ui, gradient: DefaultColorGradient) {
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            set_tight_spacing(ui);

            let total = *self.gradient_totals.get(&gradient).unwrap_or(&0);

            let tooltip_pos = ui.cursor().left_top();
            let mut size = ui.spacing().interact_size;
            size.x = ui.available_width();
            if total == 0 || self.clickable {
                size.y *= GRADIENT_HEIGHT_MULTIPLIER;
            } else {
                size.y *= GRADIENT_COMPACT_HEIGHT_MULTIPLIER;
            }

            let r = ColorButton {
                size,
                tooltip_pos,

                color_name: gradient.to_string(),
                color: gradient.clone().into(),
                default_color: None,
                puzzle_color: None,
            }
            .show(ui, self);
            self.click_zone(&r, &gradient.default_color_at_end());

            ui.horizontal_wrapped(|ui| {
                for index in 0..total {
                    let default_color = gradient.default_color_at(index, total);
                    let r = self.show_generic_color(ui, &default_color, tooltip_pos);
                    self.click_zone(&r, &default_color);
                    self.reorder_drag_drop_zone(ui, &r, &default_color);
                }
            });
        });
    }

    fn show_generic_color(
        &mut self,
        ui: &mut egui::Ui,
        default_color: &DefaultColor,
        tooltip_pos: egui::Pos2,
    ) -> egui::Response {
        let size = ui.spacing().interact_size;
        let Some(rgb) = self.palette.get(default_color) else {
            return super::error_label(ui, format!("missing color {default_color}"));
        };
        let puzzle_color = if self.show_puzzle_colors {
            self.default_color_to_puzzle_color
                .get(&default_color)
                .cloned()
        } else {
            None
        };

        ColorButton {
            size,
            tooltip_pos,

            color_name: default_color.to_string(),
            color: rgb.into(),
            default_color: Some(default_color.clone()),
            puzzle_color,
        }
        .show(ui, self)
    }
}

struct ColorButton {
    pub size: egui::Vec2,
    pub tooltip_pos: egui::Pos2,

    pub color_name: String,
    pub color: ColorOrGradient,
    pub default_color: Option<DefaultColor>,
    pub puzzle_color: Option<String>,
}
impl ColorButton {
    fn show(self, ui: &mut egui::Ui, colors_ui: &mut ColorsUi<'_>) -> egui::Response {
        // This function is based on [`egui::color_picker`]

        // Colored rectangle
        let sense = egui::Sense {
            click: colors_ui.clickable,
            drag: colors_ui.dnd.is_some(),
            focusable: true,
        };
        let (rect, r) = ui.allocate_exact_size(self.size, sense);
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&r);
            let rect = rect.expand(visuals.expansion);
            paint_colored_rect(ui.painter(), rect, 0.0, self.color);

            let rounding = visuals.rounding.at_most(2.0);
            ui.painter()
                .rect_stroke(rect, rounding, (2.0, visuals.bg_fill)); // fill is intentional, because default style has no border
        }

        // Draggable label
        if let Some(puzzle_color) = self.puzzle_color.filter(|_| colors_ui.show_puzzle_colors) {
            let put_puzzle_color_label = |ui: &mut egui::Ui, is_dragging: bool| {
                let text_color = if is_dragging {
                    ui.painter().rect_filled(
                        rect.expand(2.0),
                        3.0,
                        ui.visuals().window_fill.linear_multiply(0.75),
                    );
                    ui.visuals().strong_text_color()
                } else {
                    self.color.constrasting_text_color()
                };

                ui.put(
                    rect,
                    egui::Label::new(egui::RichText::new(&puzzle_color).color(text_color))
                        .selectable(false),
                );

                r.clone()
            };

            ui.allocate_ui_at_rect(rect, |ui| {
                if let Some(dnd) = &mut colors_ui.dnd {
                    dnd.draggable(ui, puzzle_color.clone(), put_puzzle_color_label);
                } else {
                    put_puzzle_color_label(ui, false);
                }
            });
        }

        // Tooltip
        if !colors_ui.is_dragging()
            && (r.hovered() || r.has_focus() || r.is_pointer_button_down_on())
        {
            let id = ui.auto_id_with("hyperspeedcube::color_tooltip");

            let mut color_square_size = egui::Vec2::splat(ui.spacing().interact_size.x);
            if self.color.is_gradient() {
                color_square_size.x *= GRADIENT_WIDTH_MULTIPLIER;
            }

            let left_bottom = self.tooltip_pos + egui::vec2(-ui.spacing().menu_margin.left, -5.0);
            egui::Area::new(id)
                .interactable(false)
                .fixed_pos(left_bottom)
                .constrain(true)
                .pivot(egui::Align2::LEFT_BOTTOM)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .shadow(egui::epaint::Shadow::NONE)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (rect, _response) =
                                    ui.allocate_exact_size(color_square_size, egui::Sense::hover());

                                paint_colored_rect(
                                    ui.painter(),
                                    rect,
                                    TOOLTIP_COLOR_RECT_ROUNDING,
                                    self.color,
                                );

                                ui.vertical(|ui| {
                                    ui.style_mut().wrap = Some(false);
                                    ui.strong(self.color_name);
                                    match self.color {
                                        ColorOrGradient::Color(rgb) => {
                                            let [r, g, b, _a] = rgb.to_array();
                                            ui.monospace(Rgb { rgb: [r, g, b] }.to_string());
                                        }
                                        ColorOrGradient::Gradient(_) => {
                                            ui.label("Built-in gradient");
                                        }
                                    }
                                });
                            });
                        });
                });
        }

        r.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::ColorButton));
        r
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ColorOrGradient {
    Color(egui::Color32),
    Gradient(colorous::Gradient),
}
impl From<Rgb> for ColorOrGradient {
    fn from(value: Rgb) -> Self {
        Self::Color(crate::util::rgb_to_egui_color32(value))
    }
}
impl From<DefaultColorGradient> for ColorOrGradient {
    fn from(value: DefaultColorGradient) -> Self {
        Self::Gradient(value.to_colorous())
    }
}
impl ColorOrGradient {
    pub fn is_gradient(self) -> bool {
        matches!(self, Self::Gradient(_))
    }
    pub fn middle_color(self) -> egui::Color32 {
        match self {
            Self::Color(c) => c,
            Self::Gradient(g) => colorous_color_to_egui_color(g.eval_continuous(0.5)),
        }
    }
    pub fn constrasting_text_color(self) -> egui::Color32 {
        crate::util::contrasting_text_color(self.middle_color())
    }
}

fn paint_colored_rect(
    painter: &egui::Painter,
    mut rect: egui::Rect,
    rounding: f32,
    color: ColorOrGradient,
) {
    match color {
        ColorOrGradient::Color(c) => {
            painter.rect_filled(rect, rounding, c);
        }
        ColorOrGradient::Gradient(g) => {
            if rounding > 0.0 {
                let mut left = rect;
                left.max.x = left.min.x + rounding * 2.0;
                let left_color = colorous_color_to_egui_color(g.eval_continuous(0.0));
                painter.rect_filled(left, rounding, left_color);

                let mut right = rect;
                right.min.x = right.max.x - rounding * 2.0;
                let right_color = colorous_color_to_egui_color(g.eval_continuous(1.0));
                painter.rect_filled(right, rounding, right_color);

                rect.min.x += rounding;
                rect.max.x -= rounding;
            }

            let block_count = (rect.size().x * painter.ctx().pixels_per_point()
                / GRADIENT_RESOLUTION as f32)
                .round() as usize;
            for i in 0..block_count {
                let sliver = egui::Rect::from_x_y_ranges(
                    egui::Rangef {
                        min: hypermath::util::lerp(
                            rect.min.x,
                            rect.max.x,
                            i as f32 / block_count as f32,
                        ),
                        max: hypermath::util::lerp(
                            rect.min.x,
                            rect.max.x,
                            (i + 1) as f32 / block_count as f32,
                        ),
                    },
                    rect.y_range(),
                );
                let rgb = g.eval_rational(i, block_count - 1).as_array();
                let c = crate::util::rgb_to_egui_color32(Rgb { rgb });
                egui::color_picker::show_color_at(painter, c, sliver);
            }
        }
    }
}

fn colorous_color_to_egui_color(c: colorous::Color) -> egui::Color32 {
    let rgb = c.as_array();
    crate::util::rgb_to_egui_color32(Rgb { rgb })
}

pub fn color_hex_editor(
    ui: &mut egui::Ui,
    color: &mut Rgb,
    on_delete: Option<impl FnOnce()>,
) -> egui::Response {
    let mut r = ui.add(
        egui::Label::new(egui::RichText::new(color.to_string()).monospace())
            .selectable(false)
            .sense(egui::Sense::click_and_drag()), // for when it's draggable
    );

    // Right-click to copy
    let has_been_copied = crate::gui::util::EguiTempFlag::new(ui);
    if r.secondary_clicked() {
        ui.ctx().copy_text(color.to_string());
        has_been_copied.set();
    }
    if has_been_copied.get() {
        if r.hovered() {
            // Show the tooltip immediately, with no delay
            egui::show_tooltip_for(ui.ctx(), r.id, &r.rect, |ui| ui.label("Copied!"));
        } else {
            // Hide the tooltip when the mouse leaves
            has_been_copied.clear();
        }
    } else {
        r = r.on_hover_ui(|ui| {
            set_widget_spacing_to_space_width(ui);
            ui.horizontal(|ui| {
                ui.strong("Click");
                ui.label("to edit");
            });
            ui.horizontal(|ui| {
                ui.strong("Right-click");
                ui.label("to copy");
            });
            if on_delete.is_some() {
                ui.horizontal(|ui| {
                    ui.strong("Alt + click");
                    ui.label("to delete");
                });
            }
        });
    }
    // Alt+click to delete
    if let Some(on_delete) = on_delete {
        if r.clicked() && ui.input(|input| input.modifiers.alt) {
            on_delete()
        }
    }

    // Left-click to edit
    let mut hex_edit_popup = TextEditPopup::new(ui);
    if r.clicked() && ui.input(|input| !input.modifiers.alt) {
        hex_edit_popup.open(color.to_string());
    }
    let popup_response = hex_edit_popup.if_open(|popup| {
        popup
            .over(&r)
            .text_edit_monospace()
            .confirm_button_validator(Box::new(|s| {
                s.parse::<Rgb>().map(|_| None).map_err(|_| None)
            }))
            .show(ui)
    });
    if let Some(r) = popup_response {
        match r {
            super::TextEditPopupResponse::Confirm(new_hex_string) => {
                if let Ok(new_color) = new_hex_string.parse() {
                    *color = new_color;
                }
            }
            _ => (),
        }
    }

    r
}

pub fn color_edit(
    ui: &mut egui::Ui,
    color: &mut Rgb,
    label: &str,
    clickable: bool,
    on_delete: Option<impl FnOnce()>,
) -> egui::InnerResponse<egui::Response> {
    let mut changed = false;

    let mut r = ui.horizontal(|ui| {
        changed |= super::color_hex_editor(ui, color, on_delete).changed();

        let r = ui.color_edit_button_srgb(&mut color.rgb);
        changed |= r.changed();

        if clickable {
            egui::Label::new(label)
                .selectable(false)
                .sense(egui::Sense::click())
                .ui(ui)
        } else {
            ui.label(label)
        }
    });

    if changed {
        r.response.mark_changed();
    }
    r
}

fn set_tight_spacing(ui: &mut egui::Ui) {
    let item_spacing = &mut ui.spacing_mut().item_spacing;
    *item_spacing = egui::Vec2::splat(item_spacing.min_elem());
}