//! The windowless GPU path: a chosen adapter's device, an `egui_wgpu::RenderState` made
//! without a surface (its target format is `Rgba8Unorm`), the offscreen colour and depth
//! targets, and one egui pass drawn into them.
//!
//! The pass is 1 sample and carries the `Depth24Plus` attachment the viewport's pipelines expect,
//! so the egui renderer is built for that same attachment (`RendererOptions { msaa_samples: 1,
//! depth_stencil_format: Some(Depth24Plus) }`)
//! viewport's pipelines, which `ModelApp::set_wgpu` installs with the same target (depth 24,
//! 1 sample). The app's clear colour is the theme's panel, so parts a widget does not cover
//! look like the desktop window.

use crate::host::Painter;
use fieldcraft_gpu::select;

/// `RenderState` without a surface: format `Rgba8Unorm`, 1 sample, no egui depth.
pub fn render_state(policy: select::Policy) -> Result<egui_wgpu::RenderState, String> {
    let (adapter, _tier) = select::adapter(policy)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no GPU adapter (FIELDCRAFT_GPU=cpu cannot stream frames; software lavapipe is fine)".to_string())?;
    let limits = adapter.limits();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("fieldz-render-server"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default().using_resolution(limits),
        ..Default::default()
    }))
    .map_err(|e| format!("requesting the device: {e}"))?;
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor { backends: select::backends(), ..wgpu::InstanceDescriptor::new_without_display_handle() });
    let config = egui_wgpu::WgpuConfiguration {
        wgpu_setup: egui_wgpu::WgpuSetup::Existing(egui_wgpu::WgpuSetupExisting { instance: instance.clone(), adapter, device, queue }),
        ..Default::default()
    };
    let options = egui_wgpu::RendererOptions { msaa_samples: 1, depth_stencil_format: Some(wgpu::TextureFormat::Depth24Plus), ..Default::default() };
    let state =
        pollster::block_on(egui_wgpu::RenderState::create(&config, &instance, None, options)).map_err(|e| format!("egui-wgpu's RenderState: {e}"))?;
    if state.target_format != wgpu::TextureFormat::Rgba8Unorm {
        return Err(format!("the offscreen target is {:?}, not Rgba8Unorm", state.target_format));
    }
    Ok(state)
}

/// The colour target (copy source) and the depth attachment the viewport's pipelines expect.
struct Target {
    color: wgpu::Texture,
    view: wgpu::TextureView,
    depth: wgpu::TextureView,
}

impl Target {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Target {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("fieldz-render-server colour"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = color.create_view(&Default::default());
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("fieldz-render-server depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth = depth.create_view(&Default::default());
        Target { color, view, depth }
    }
}

/// The render state and the offscreen targets, painted one pass at a time.
pub struct Gpu {
    rs: egui_wgpu::RenderState,
    target: Target,
    width: u32,
    height: u32,
    dark: bool,
}

impl Gpu {
    pub fn new(policy: select::Policy, width: u32, height: u32, dark: bool) -> Result<Gpu, String> {
        let rs = render_state(policy)?;
        let target = Target::new(&rs.device, width, height);
        Ok(Gpu { rs, target, width, height, dark })
    }

    fn recreate(&mut self, width: u32, height: u32) {
        self.target = Target::new(&self.rs.device, width, height);
        self.width = width;
        self.height = height;
    }

    /// The theme's panel colour, as a clear value (the target is 8-bit non-sRGB; egui's colours
    /// are the same sRGB bytes).
    fn clear(&self) -> wgpu::Color {
        let c = fieldcraft_ui_egui::theme::Tokens::get(self.dark).panel;
        wgpu::Color { r: f64::from(c.r()) / 255.0, g: f64::from(c.g()) / 255.0, b: f64::from(c.b()) / 255.0, a: f64::from(c.a()) / 255.0 }
    }
}

impl Painter for Gpu {
    fn render_state(&self) -> Option<&egui_wgpu::RenderState> {
        Some(&self.rs)
    }

    fn max_texture_side(&self) -> usize {
        self.rs.device.limits().max_texture_dimension_2d as usize
    }

    fn resize(&mut self, width_px: u32, height_px: u32) -> Result<(), String> {
        if (width_px, height_px) != (self.width, self.height) {
            self.recreate(width_px, height_px);
        }
        Ok(())
    }

    fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
    }

    fn paint(&mut self, primitives: &[egui::ClippedPrimitive], textures: egui::TexturesDelta, pixels_per_point: f32) -> Result<Vec<u8>, String> {
        let (device, queue) = (&self.rs.device, &self.rs.queue);
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [self.width, self.height], pixels_per_point };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("fieldz-render-server pass") });
        {
            let mut renderer = self.rs.renderer.write();
            // `TexturesDelta::set` holds every delta of a texture id, applied in order.
            for (id, deltas) in &textures.set {
                for delta in deltas {
                    renderer.update_texture(device, queue, *id, delta);
                }
            }
            let mut user = renderer.update_buffers(device, queue, &mut encoder, primitives, &screen);
            {
                let clear = self.clear();
                let mut pass = encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("fieldz-render-server pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.target.view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.target.depth,
                            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    })
                    .forget_lifetime();
                renderer.render(&mut pass, primitives, &screen);
            }
            user.push(encoder.finish());
            queue.submit(user);
            for id in &textures.free {
                renderer.free_texture(id);
            }
        }
        crate::readback::read_texture(device, queue, &self.target.color, self.width, self.height)
    }
}
