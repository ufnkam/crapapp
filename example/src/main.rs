use std::num::NonZeroU32;
use std::sync::Arc;

use egui::{Color32, Context, Pos2, Stroke, ViewportId};
use egui_wgpu::{RendererOptions, WgpuConfiguration};
use egui_winit::State as EguiWinitState;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = ExampleApplication::default();

    event_loop.run_app(&mut app)?;

    Ok(())
}

#[derive(Default)]
struct ExampleApplication {
    window: Option<Arc<Window>>,
    egui: Option<EguiState>,
}

impl ApplicationHandler for ExampleApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(window_attributes())
                .expect("failed to create window"),
        );
        let egui = pollster::block_on(EguiState::new(event_loop, Arc::clone(&window)))
            .expect("failed to initialize egui renderer");

        self.window = Some(window);
        self.egui = Some(egui);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        let Some(egui) = self.egui.as_mut() else {
            return;
        };

        let response = egui.state.on_window_event(window, &event);
        if response.repaint {
            window.request_redraw();
        }
        if response.consumed {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    egui.painter
                        .on_window_resized(ViewportId::ROOT, width, height);
                }
            }
            WindowEvent::RedrawRequested => egui.paint(window),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

struct EguiState {
    context: Context,
    state: EguiWinitState,
    painter: egui_wgpu::winit::Painter,
}

impl EguiState {
    async fn new(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::default();
        let display_handle = event_loop.owned_display_handle();
        let config = WgpuConfiguration {
            wgpu_setup: egui_wgpu::WgpuSetup::from_display_handle(display_handle.clone()),
            ..Default::default()
        };
        let mut painter = egui_wgpu::winit::Painter::new(
            context.clone(),
            config,
            false,
            RendererOptions::default(),
        )
        .await;

        painter
            .set_window(ViewportId::ROOT, Some(window.clone()))
            .await?;

        let state = EguiWinitState::new(
            context.clone(),
            ViewportId::ROOT,
            &display_handle,
            Some(window.scale_factor() as f32),
            window.theme(),
            painter.max_texture_side(),
        );

        Ok(Self {
            context,
            state,
            painter,
        })
    }

    fn paint(&mut self, window: &Arc<Window>) {
        let mut input = self.state.take_egui_input(window);
        self.painter.handle_screenshots(&mut input.events);
        egui_winit::update_viewport_info(
            input.viewports.entry(ViewportId::ROOT).or_default(),
            &self.context,
            window,
            false,
        );

        let output = self.context.run_ui(input, draw_animation);
        self.state
            .handle_platform_output(window, output.platform_output);

        let pixels_per_point = egui_winit::pixels_per_point(&self.context, window);
        let clipped_primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);

        self.painter.paint_and_update_textures(
            ViewportId::ROOT,
            pixels_per_point,
            [0.04, 0.05, 0.06, 1.0],
            &clipped_primitives,
            &output.textures_delta,
            Vec::new(),
            window,
        );
    }
}

fn draw_animation(ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let time = ui.input(|input| input.time) as f32;
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.24;
    let dot = Pos2::new(
        center.x + radius * time.cos(),
        center.y + radius * time.sin(),
    );

    painter.circle_stroke(center, radius, Stroke::new(2.0, Color32::from_gray(90)));
    painter.circle_filled(dot, 24.0, Color32::from_rgb(90, 210, 170));
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        "Example App",
        egui::FontId::proportional(28.0),
        Color32::WHITE,
    );

    ui.ctx().request_repaint();
}

fn window_attributes() -> WindowAttributes {
    Window::default_attributes()
        .with_title("Example App")
        .with_inner_size(LogicalSize::new(520.0, 360.0))
        .with_min_inner_size(LogicalSize::new(360.0, 260.0))
}
