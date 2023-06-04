use ndpuzzle::geometry::{EuclideanCgaManifold, ShapeArena};
use ndpuzzle::math::cga::Isometry;
use ndpuzzle::puzzle::Mesh;
use std::fmt;

use crate::render::{GraphicsState, PuzzleRenderer, ViewParams};

#[derive(Debug)]
pub struct PuzzleView {
    pub arena: ShapeArena<EuclideanCgaManifold>,
    renderer: PuzzleRenderer,
    pub view_params: ViewParams,

    texture_id: egui::TextureId,
    rect: egui::Rect,
    pub render_engine: RenderEngine,

    pub overlay: Vec<(Overlay, f32)>,
}
impl PuzzleView {
    pub(crate) fn new(gfx: &GraphicsState, egui_renderer: &mut egui_wgpu::Renderer) -> Self {
        let texture_id = egui_renderer.register_native_texture(
            &gfx.device,
            &gfx.dummy_texture_view(),
            wgpu::FilterMode::Linear,
        );

        let arena = ShapeArena::new_euclidean_cga(3);
        let mesh = Mesh::new_example_mesh().unwrap();

        PuzzleView {
            arena,
            renderer: PuzzleRenderer::new(gfx, &mesh),
            view_params: ViewParams::default(),

            texture_id,
            rect: egui::Rect::NOTHING,
            render_engine: RenderEngine::SinglePass,

            overlay: vec![],
        }
    }
    pub(crate) fn set_mesh(
        &mut self,
        gfx: &GraphicsState,
        arena: ShapeArena<EuclideanCgaManifold>,
        mesh: Option<&Mesh>,
    ) {
        self.arena = arena;
        if let Some(mesh) = mesh {
            self.renderer = PuzzleRenderer::new(gfx, mesh);
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        let dpi = ui.ctx().pixels_per_point();

        // Round rectangle to pixel boundary for crisp image.
        let mut pixels_rect = ui.available_rect_before_wrap();
        pixels_rect.set_left((dpi * pixels_rect.left()).ceil());
        pixels_rect.set_bottom((dpi * pixels_rect.bottom()).floor());
        pixels_rect.set_right((dpi * pixels_rect.right()).floor());
        pixels_rect.set_top((dpi * pixels_rect.top()).ceil());

        // Convert back from pixel coordinates to egui coordinates.
        let mut egui_rect = pixels_rect;
        *egui_rect.left_mut() /= dpi;
        *egui_rect.bottom_mut() /= dpi;
        *egui_rect.right_mut() /= dpi;
        *egui_rect.top_mut() /= dpi;

        self.rect = egui_rect;

        let r = ui.put(
            egui_rect,
            egui::Image::new(self.texture_id, egui_rect.size())
                .sense(egui::Sense::click_and_drag()),
        );

        let min_size = egui_rect.size().min_elem();
        const DRAG_SPEED: f32 = 5.0;
        let drag_delta = r.drag_delta() * DRAG_SPEED / min_size.abs();

        let scroll_delta = ui.input(|input| input.scroll_delta);
        if r.hovered() {
            self.view_params.zoom *= (scroll_delta.y / 100.0).exp2();
        }

        self.view_params.rot = Isometry::from_angle_in_axis_plane(0, 2, -drag_delta.x)
            * Isometry::from_angle_in_axis_plane(1, 2, drag_delta.y)
            * &self.view_params.rot;

        // Render overlay
        let transform_point = |p: &ndpuzzle::math::Vector| -> Option<egui::Pos2> {
            let mut p = self.view_params.project_point(p)?;
            p.x *= egui_rect.size().x / 2.0 / 1.5;
            p.y *= egui_rect.size().y / 2.0 / 1.5;
            Some(egui_rect.center() + egui::vec2(p.x, -p.y))
        };
        for (overlay, size) in &self.overlay {
            // IIFE to mimic try_block
            let _ = (|| -> Option<()> {
                match overlay {
                    Overlay::Point(p) => ui.painter().circle_filled(
                        transform_point(p)?,
                        5.0 * size,
                        egui::Color32::BLUE,
                    ),
                    Overlay::Line(p1, p2) => ui.painter().line_segment(
                        [transform_point(p1)?, transform_point(p2)?],
                        egui::Stroke {
                            width: 4.0 * size,
                            color: egui::Color32::LIGHT_GREEN,
                        },
                    ),
                    Overlay::Arrow(p1, p2) => ui.painter().arrow(
                        transform_point(p1)?,
                        transform_point(p2)? - transform_point(p1)?,
                        egui::Stroke {
                            width: 4.0 * size,
                            color: egui::Color32::LIGHT_BLUE,
                        },
                    ),
                }
                None
            })();
        }

        if r.is_pointer_button_down_on() {
            // TODO: request focus not working?
            r.request_focus();
            true
        } else {
            false
        }
    }

    pub(crate) fn render_and_update_texture(
        &mut self,
        gfx: &GraphicsState,
        egui_ctx: &egui::Context,
        egui_renderer: &mut egui_wgpu::Renderer,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let view_params = &mut self.view_params;

        view_params.width = self.rect.width() as u32;
        view_params.height = self.rect.height() as u32;
        let new_texture = match self.render_engine {
            RenderEngine::SinglePass => {
                self.renderer
                    .draw_puzzle_single_pass(gfx, encoder, &view_params)
            }
            RenderEngine::MultiPass => self.renderer.draw_puzzle(gfx, encoder, &view_params),
        };

        // Draw puzzle if necessary.
        if let Ok(texture) = new_texture {
            log::trace!("Updating puzzle texture");

            // Update texture for egui.
            egui_renderer.update_egui_texture_from_wgpu_texture(
                &gfx.device,
                texture,
                wgpu::FilterMode::Linear,
                self.texture_id,
            );

            // Request a repaint.
            egui_ctx.request_repaint();
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub enum RenderEngine {
    SinglePass,
    #[default]
    MultiPass,
}
impl fmt::Display for RenderEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderEngine::SinglePass => write!(f, "Fast"),
            RenderEngine::MultiPass => write!(f, "Fancy"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Overlay {
    Point(ndpuzzle::math::Vector),
    Line(ndpuzzle::math::Vector, ndpuzzle::math::Vector),
    Arrow(ndpuzzle::math::Vector, ndpuzzle::math::Vector),
}
