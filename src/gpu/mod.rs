//! Device, queue, and surface acquisition (`ARCHITECTURE.md` 3.1, D9).
//!
//! Adapter selection is **explicit and ordered**, never "whatever comes first".
//! The enumeration on this machine exposes three DX12 entries, one of which is
//! the Microsoft Basic Render Driver — a CPU software rasteriser. A naive
//! first-DX12 policy would silently deliver software rendering while reporting
//! success, so the order below is a correctness requirement, not a preference.
//!
//! **Nothing here parses VT or holds engine state.** This module owns a device and
//! a surface and nothing else.

use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

/// Which adapter was chosen, and why. Recorded so a surprising choice is
/// diagnosable after the fact rather than invisible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterChoice {
    pub name: String,
    pub backend: wgpu::Backend,
    pub device_type: wgpu::DeviceType,
    pub driver: String,
    /// True when the only option available was a software rasteriser.
    pub software_fallback: bool,
}

pub struct Gpu {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub format: wgpu::TextureFormat,
    pub choice: AdapterChoice,
    /// Every adapter seen, in enumeration order. Kept for diagnostics only.
    pub rejected: Vec<String>,
}

#[derive(Debug)]
pub enum GpuError {
    NoSurface,
    NoAdapter,
    Surface(Box<dyn std::error::Error>),
    DeviceRequest(wgpu::RequestDeviceError),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSurface => write!(f, "could not create a surface"),
            Self::NoAdapter => write!(f, "no adapter available on any backend"),
            Self::Surface(e) => write!(f, "surface creation failed: {e}"),
            Self::DeviceRequest(e) => write!(f, "device request failed: {e}"),
        }
    }
}

/// Why a frame was skipped. wgpu 29 reports these as variants of
/// `CurrentSurfaceTexture` rather than an error `Result`, because most of them are
/// routine: an occluded or timeout frame is normal, not a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameSkip {
    Timeout,
    Occluded,
    Outdated,
    Lost,
    Validation,
}

impl std::error::Error for GpuError {}

/// Drives a future to completion on the calling thread.
///
/// `enumerate_adapters` is async in wgpu 29. Adding an async runtime for one
/// diagnostic-and-startup path is not justified (D2), so this is a ~15-line
/// poller instead. The deadline is deliberate: a silently hung GPU enumeration
/// would look identical to a slow machine, and a hung window is worse than a
/// failed one.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    let mut fut = Box::pin(fut);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => {
                if Instant::now() > deadline {
                    panic!("block_on exceeded 20s; the future never completed");
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

/// D9 policy: discrete hardware first, integrated second, CPU rasterisers only
/// when nothing else exists — and flagged when it is used.
fn rank(t: wgpu::DeviceType) -> u8 {
    match t {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu => 4,
    }
}

pub fn pick_adapter(
    adapters: &[wgpu::Adapter],
) -> Result<(wgpu::Adapter, AdapterChoice, Vec<String>), GpuError> {
    if adapters.is_empty() {
        return Err(GpuError::NoAdapter);
    }

    let mut rejected: Vec<String> = adapters
        .iter()
        .map(|a| {
            let i = a.get_info();
            format!("{} [{}] {:?}", i.name, i.backend, i.device_type)
        })
        .collect();

    // Enumeration contains apparent duplicates on this machine (two identical
    // DX12 entries), so the list is NOT assumed deduplicated.
    let mut best: Option<(u8, usize)> = None;
    for (i, a) in adapters.iter().enumerate() {
        let r = rank(a.get_info().device_type);
        match best {
            Some((br, _)) if br <= r => {}
            _ => best = Some((r, i)),
        }
    }
    let (_, idx) = best.expect("adapters is non-empty");

    let adapter = adapters[idx].clone();
    let info = adapter.get_info();
    let chosen_desc = format!("{} [{}] {:?}", info.name, info.backend, info.device_type);
    rejected.retain(|r| *r != chosen_desc);

    let choice = AdapterChoice {
        name: info.name.clone(),
        backend: info.backend,
        device_type: info.device_type,
        driver: info.driver.clone(),
        software_fallback: info.device_type == wgpu::DeviceType::Cpu,
    };
    Ok((adapter, choice, rejected))
}

impl Gpu {
    pub fn new(
        instance: &wgpu::Instance,
        create_surface: impl FnOnce(&wgpu::Instance) -> Result<wgpu::Surface<'static>, String>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let surface = create_surface(instance).map_err(|e| GpuError::Surface(e.into()))?;

        let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::all()));
        let (adapter, choice, rejected) = pick_adapter(&adapters)?;

        let requested = adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("juna-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        });
        let (device, queue) =
            block_on(requested).map_err(GpuError::DeviceRequest)?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        Ok(Self {
            adapter,
            device,
            queue,
            surface,
            config,
            format,
            choice,
            rejected,
        })
    }

    /// Resize the surface. Losing the surface is reported, not ignored.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    /// Render one frame. Takes `&self`: nothing here mutates the device, and taking
///     `&self` lets the caller hold a `&mut Renderer` at the same time without
    /// splitting the borrow of this struct across the closure.
    ///
    /// `Ok(false)` means the frame was skipped for a routine reason (occluded,
    /// timed out, surface outdated); the caller should not treat that as an
    /// error, but should not assume anything was presented either.
    pub fn render<F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::RenderPass<'_>)>(
        &self,
        clear: wgpu::Color,
        draw: F,
    ) -> Result<bool, FrameSkip> {
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout => return Err(FrameSkip::Timeout),
            wgpu::CurrentSurfaceTexture::Occluded => return Err(FrameSkip::Occluded),
            wgpu::CurrentSurfaceTexture::Outdated => return Err(FrameSkip::Outdated),
            wgpu::CurrentSurfaceTexture::Lost => return Err(FrameSkip::Lost),
            wgpu::CurrentSurfaceTexture::Validation => return Err(FrameSkip::Validation),
        };

        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("juna-frame"),
                });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("juna-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            draw(&self.device, &self.queue, &mut pass);
        }
        self.queue.submit(Some(enc.finish()));
        texture.present();
        Ok(true)
    }
}