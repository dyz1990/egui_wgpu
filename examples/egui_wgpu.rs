use egui_wgpu::EguiWgpu;
use egui_wgpu::RenderTarget;
use egui_winit::winit;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_decorations(true)
        .with_resizable(true)
        .with_transparent(false)
        .with_title("egui-wgpu_winit example")
        .with_inner_size(winit::dpi::PhysicalSize {
            width: 800,
            height: 600,
        })
        .build(&event_loop)
        .unwrap();

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
    let surface = unsafe { instance.create_surface(&window) };

    // WGPU 0.11+ support force fallback (if HW implementation not supported), set it to true or false (optional).
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            features: wgpu::Features::default(),
            limits: wgpu::Limits::default(),
            label: None,
        },
        None,
    ))
    .unwrap();

    let size = window.inner_size();
    let surface_format = surface.get_preferred_format(&adapter).unwrap();
    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width as u32,
        height: size.height as u32,
        present_mode: wgpu::PresentMode::Fifo,
    };
    surface.configure(&device, &surface_config);
    let pipeline = egui_wgpu::Pipeline::new(&device, surface_format, 1);

    let mut egui_wgpu = EguiWgpu::new(&adapter, &device, &window);
    event_loop.run(move |event, _target, cf| {
        //
        match event {
            winit::event::Event::WindowEvent { window_id, event } => {
                match &event {
                    winit::event::WindowEvent::Resized(size) => {
                        surface_config.width = size.width;
                        surface_config.height = size.height;
                        surface.configure(&device, &surface_config);
                    }
                    winit::event::WindowEvent::CloseRequested => {
                        *cf = winit::event_loop::ControlFlow::Exit;
                    }
                    _ => {}
                }
                egui_wgpu.on_event(&event);
                window.request_redraw();
            }
            winit::event::Event::RedrawRequested(_) => {
                let mut quit = false;
                let needs_repaint = egui_wgpu.run(&window, |egui_ctx| {
                    egui::SidePanel::left("my_side_panel").show(egui_ctx, |ui| {
                        ui.heading("Hello World!");
                        if ui.button("Quit").clicked() {
                            quit = true;
                            println!("Quit Click");
                        }
                    });
                });

                *cf = if quit {
                    winit::event_loop::ControlFlow::Exit
                } else if needs_repaint {
                    window.request_redraw();
                    winit::event_loop::ControlFlow::Poll
                } else {
                    winit::event_loop::ControlFlow::Wait
                };

                if let Ok(t) = surface.get_current_texture() {
                    let view = t.texture.create_view(&wgpu::TextureViewDescriptor {
                        ..Default::default()
                    });
                    let target = RenderTarget {
                        view: &view,
                        clear_color: Some(wgpu::Color::TRANSPARENT),
                        width: surface_config.width,
                        height: surface_config.height,
                    };

                    egui_wgpu.paint(&device, &queue, &pipeline, target);
                    t.present();
                }
            }
            _ => {}
        }
    });
}
