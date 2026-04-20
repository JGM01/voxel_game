use wgpu::{
    DeviceDescriptor, Extent3d, Features, InstanceDescriptor, Limits, MemoryHints, PowerPreference,
    RequestAdapterOptions, SurfaceConfiguration, SurfaceTarget, TextureAspect, TextureDescriptor,
    TextureDimension, TextureUsages, TextureViewDescriptor, TextureViewDimension, Trace,
};

pub struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
}

impl Gpu {
    pub fn aspect_ratio(&self) -> f32 {
        self.surface_config.width as f32 / self.surface_config.height.max(1) as f32
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn create_depth_texture(&self, width: u32, height: u32) -> wgpu::TextureView {
        let texture = self.device.create_texture(&TextureDescriptor {
            label: Some("Depth Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(TextureViewDimension::D2),
            aspect: TextureAspect::DepthOnly,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
            usage: None,
        })
    }

    pub async fn new_async(
        window: impl Into<SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let instance = wgpu::Instance::new(&InstanceDescriptor::default());
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request adapter!");

        log::info!("WGPU Adapter Features: {:#?}", adapter.features());

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("WGPU Device"),
                memory_hints: MemoryHints::default(),
                required_features: Features::default(),
                required_limits: Limits::default().using_resolution(adapter.limits()),
                trace: Trace::Off,
            })
            .await
            .expect("Failed to request a device!");

        let surface_capabilities = surface.get_capabilities(&adapter);

        // Prefer an sRGB surface so the browser compositor receives correctly
        // gamma-encoded values. Without this, linear colour output is
        // reinterpreted as sRGB by the compositor and colours appear washed out.
        // If no sRGB format is available we fall back to whatever is first, but
        // that should never happen on a WebGPU-capable browser.
        let surface_format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or_else(|| {
                log::warn!(
                    "No sRGB surface format available — colours may appear incorrect. \
                     Falling back to {:?}.",
                    surface_capabilities.formats[0]
                );
                surface_capabilities.formats[0]
            });

        log::info!("Selected surface format: {:?}", surface_format);

        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        Self {
            surface,
            device,
            queue,
            surface_config,
            surface_format,
        }
    }
}
