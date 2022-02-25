mod painter;
mod pipeline;
use painter::Painter;
pub use pipeline::Pipeline;
use wgpu::{Adapter, Device, TextureView};
pub struct EguiWgpu {
    pub egui_ctx: egui::Context,
    pub egui_winit: egui_winit::State,
    painter: painter::Painter,
    shapes: Vec<egui::epaint::ClippedShape>,
    textures_delta: egui::TexturesDelta,
}

pub struct RenderTarget<'a> {
    pub view: &'a TextureView,
    pub clear_color: Option<wgpu::Color>,
    pub width: u32,
    pub height: u32,
}
impl EguiWgpu {
    pub fn new(
        adapter: &Adapter,
        device: &Device,
        window: &egui_winit::winit::window::Window,
    ) -> Self {
        let max_texture_side = adapter.limits().max_texture_dimension_2d as usize;
        Self {
            egui_ctx: egui::Context::default(),
            egui_winit: egui_winit::State::new(max_texture_side, window),
            painter: Painter::new(device),
            shapes: Default::default(),
            textures_delta: Default::default(),
        }
    }

    /// Returns `true` if egui wants exclusive use of this event
    /// (e.g. a mouse click on an egui window, or entering text into a text field).
    /// For instance, if you use egui for a game, you want to first call this
    /// and only when this returns `false` pass on the events to your game.
    ///
    /// Note that egui uses `tab` to move focus between elements, so this will always return `true` for tabs.
    pub fn on_event(&mut self, event: &egui_winit::winit::event::WindowEvent<'_>) -> bool {
        self.egui_winit.on_event(&self.egui_ctx, event)
    }

    /// Returns `true` if egui requests a repaint.
    ///
    /// Call [`Self::paint`] later to paint.
    pub fn run(
        &mut self,
        window: &egui_winit::winit::window::Window,
        run_ui: impl FnOnce(&egui::Context),
    ) -> bool {
        let raw_input = self.egui_winit.take_egui_input(window);
        let egui::FullOutput {
            platform_output,
            needs_repaint,
            textures_delta,
            shapes,
        } = self.egui_ctx.run(raw_input, run_ui);
        self.egui_winit.handle_platform_output(
            window,
            &self.egui_ctx,
            platform_output,
        );

        self.shapes = shapes;
        self.textures_delta.append(textures_delta);

        needs_repaint
    }

    /// Paint the results of the last call to [`Self::run`].
    pub fn paint(
        &mut self,
        device: &Device,
        queue: &wgpu::Queue,
        pipeline: &Pipeline,
        target: RenderTarget,
    ) {
        let shapes = std::mem::take(&mut self.shapes);
        let textures_delta = std::mem::take(&mut self.textures_delta);
        let clipped_meshes = self.egui_ctx.tessellate(shapes);
        self.painter.paint_and_update_textures(
            device,
            queue,
            pipeline,
            target,
            self.egui_ctx.pixels_per_point(),
            clipped_meshes,
            &textures_delta,
        );
    }
}
