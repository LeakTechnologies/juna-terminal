//! Window-specific ephemeral capture. Pixels never enter reports or terminal state.
use windows::{
    core::Interface,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::{HMODULE, HWND},
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::*,
            Dxgi::{Common::DXGI_SAMPLE_DESC, IDXGIDevice},
        },
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
            RoInitialize, RO_INIT_MULTITHREADED,
        },
    },
};
pub struct Apartment(std::marker::PhantomData<std::rc::Rc<()>>);
impl Apartment {
    pub fn new() -> windows::core::Result<Self> {
        // SAFETY: the benchmark's main thread initializes its MTA once for the process lifetime.
        // WinRT factory caches outlive local capture objects; keep COM alive through process exit.
        // See BENCHMARK_DESIGN.md. This guard cannot move to a different thread.
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }
        Ok(Self(std::marker::PhantomData))
    }
}
pub struct Capture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
}
impl Capture {
    pub fn new(hwnd: HWND) -> windows::core::Result<Self> {
        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        // SAFETY: caller verifies this live HWND belongs to its freshly owned child; no monitor capture is used.
        let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(hwnd)? };
        let (mut device, mut context) = (None, None);
        // SAFETY: output slots are writable and live; default adapter/hardware selection and no software module.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        let device = device.ok_or_else(|| {
            windows::core::Error::from_hresult(windows::core::HRESULT(0x80004003u32 as i32))
        })?;
        let context = context.ok_or_else(|| {
            windows::core::Error::from_hresult(windows::core::HRESULT(0x80004003u32 as i32))
        })?;
        let dxgi: IDXGIDevice = device.cast()?;
        // SAFETY: dxgi is the live DXGI interface for our owned D3D device.
        let direct: IDirect3DDevice =
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? }.cast()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &direct,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            item.Size()?,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        session.StartCapture()?;
        Ok(Self {
            pool,
            session,
            device,
            context,
        })
    }
    pub fn marker_count(&self, color: [u8; 3]) -> windows::core::Result<Option<usize>> {
        let frame = match self.pool.TryGetNextFrame() {
            Ok(frame) => frame,
            // windows-rs 0.61 maps a successful native call with a null optional frame
            // to Error::empty (HRESULT S_OK). Actual failing HRESULTs remain errors.
            Err(error) if error.code().is_ok() => return Ok(None),
            Err(error) => return Err(error),
        };
        let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
        // SAFETY: the captured surface exposes the requested D3D texture interface, retained through this read.
        let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: desc is writable and texture remains alive.
        unsafe {
            texture.GetDesc(&mut desc);
        }
        if desc.Width == 0
            || desc.Height == 0
            || u64::from(desc.Width) * u64::from(desc.Height) > 16_777_216
        {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80070057u32 as i32,
            )));
        }
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        desc.SampleDesc = DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        };
        let mut staging = None;
        // SAFETY: valid bounded texture description, no initial data and writable owned output slot.
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut staging))?;
        }
        let staging = staging.ok_or_else(|| {
            windows::core::Error::from_hresult(windows::core::HRESULT(0x80004003u32 as i32))
        })?;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: resources belong to this single-threaded context; staging has CPU-read access.
        unsafe {
            self.context.CopyResource(&staging, &texture);
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }
        let mut count = 0;
        if !mapped.pData.is_null() && mapped.RowPitch >= desc.Width * 4 {
            for row in 0..desc.Height as usize {
                // SAFETY: D3D guarantees RowPitch bytes for each mapped row; we read only Width*4 BGRA bytes.
                let pixels = unsafe {
                    std::slice::from_raw_parts(
                        (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                        desc.Width as usize * 4,
                    )
                };
                for p in pixels.chunks_exact(4) {
                    if p[2].abs_diff(color[0]) <= 3
                        && p[1].abs_diff(color[1]) <= 3
                        && p[0].abs_diff(color[2]) <= 3
                    {
                        count += 1;
                    }
                }
            }
        }
        // SAFETY: exactly balances the successful Map above; no mapped references escape.
        unsafe {
            self.context.Unmap(&staging, 0);
        }
        frame.Close()?;
        Ok(Some(count))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
