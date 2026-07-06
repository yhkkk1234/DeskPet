use base64::Engine;

pub struct ScreenshotService;

impl ScreenshotService {
    pub fn new() -> Self {
        Self
    }

    pub fn capture_screen() -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            capture_screen_win32()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err("Screenshot capture is only supported on Windows".into())
        }
    }

    /// 返回虚拟屏幕的物理边界（所有显示器合集）。
    /// x/y 可能为负（副屏在主屏左侧时），width/height 是所有屏幕的总尺寸。
    pub fn virtual_screen_bounds() -> Result<(i32, i32, i32, i32), String> {
        #[cfg(target_os = "windows")]
        {
            unsafe {
                let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
                let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
                let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
                if w <= 0 || h <= 0 {
                    return Err(format!("Invalid virtual screen: {}x{} at ({},{})", w, h, x, y));
                }
                Ok((x, y, w, h))
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err("Virtual screen bounds is only supported on Windows".into())
        }
    }

}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Default)]
struct BitmapInfoHeader {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[cfg(target_os = "windows")]
extern "system" {
    fn GetDC(hwnd: isize) -> isize;
    fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn CreateCompatibleBitmap(hdc: isize, w: i32, h: i32) -> isize;
    fn SelectObject(hdc: isize, h: isize) -> isize;
    fn BitBlt(
        hdc: isize,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src_dc: isize,
        sx: i32,
        sy: i32,
        rop: u32,
    ) -> i32;
    fn DeleteDC(hdc: isize) -> i32;
    fn DeleteObject(h: isize) -> i32;
    fn GetDIBits(
        hdc: isize,
        hbm: isize,
        start: u32,
        lines: u32,
        bits: *mut u8,
        bmi: *mut BitmapInfoHeader,
        usage: u32,
    ) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
}

#[cfg(target_os = "windows")]
const SRCCOPY: u32 = 0x00CC0020;
#[cfg(target_os = "windows")]
const SM_XVIRTUALSCREEN: i32 = 76;
#[cfg(target_os = "windows")]
const SM_YVIRTUALSCREEN: i32 = 77;
#[cfg(target_os = "windows")]
const SM_CXVIRTUALSCREEN: i32 = 78;
#[cfg(target_os = "windows")]
const SM_CYVIRTUALSCREEN: i32 = 79;
#[cfg(target_os = "windows")]
const DIB_RGB_COLORS: u32 = 0;
#[cfg(target_os = "windows")]
const BI_RGB: u32 = 0;

#[cfg(target_os = "windows")]
fn capture_screen_win32() -> Result<String, String> {
    unsafe {
        // 用虚拟屏幕 API 截取所有显示器的合集，支持多屏幕。
        // SM_XVIRTUALSCREEN/SM_YVIRTUALSCREEN 可能为负（副屏在主屏左侧时）。
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if w <= 0 || h <= 0 {
            return Err(format!("Invalid virtual screen size: {}x{} at ({},{})", w, h, x, y));
        }

        let pixels = capture_rect(x, y, w, h)?;
        let png_data = encode_rgba_as_png(w as u32, h as u32, &pixels)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&png_data))
    }
}

#[cfg(target_os = "windows")]
unsafe fn capture_rect(x: i32, y: i32, w: i32, h: i32) -> Result<Vec<u8>, String> {
    let hdc_screen = GetDC(0);
    if hdc_screen == 0 {
        return Err("GetDC failed".into());
    }

    let hdc_mem = CreateCompatibleDC(hdc_screen);
    if hdc_mem == 0 {
        ReleaseDC(0, hdc_screen);
        return Err("CreateCompatibleDC failed".into());
    }

    let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
    if hbm == 0 {
        DeleteDC(hdc_mem);
        ReleaseDC(0, hdc_screen);
        return Err("CreateCompatibleBitmap failed".into());
    }

    let old_bm = SelectObject(hdc_mem, hbm);

    let blt_ok = BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, x, y, SRCCOPY);
    if blt_ok == 0 {
        SelectObject(hdc_mem, old_bm);
        DeleteObject(hbm);
        DeleteDC(hdc_mem);
        ReleaseDC(0, hdc_screen);
        return Err("BitBlt failed".into());
    }

    let mut bmi = BitmapInfoHeader {
        bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
        bi_width: w,
        bi_height: -h,
        bi_planes: 1,
        bi_bit_count: 32,
        bi_compression: BI_RGB,
        ..Default::default()
    };

    let pixel_bytes = (w as usize) * (h as usize) * 4;
    let mut pixels = vec![0u8; pixel_bytes];

    let scan_lines = GetDIBits(
        hdc_mem,
        hbm,
        0,
        h as u32,
        pixels.as_mut_ptr(),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    SelectObject(hdc_mem, old_bm);
    DeleteObject(hbm);
    DeleteDC(hdc_mem);
    ReleaseDC(0, hdc_screen);

    if scan_lines == 0 {
        return Err("GetDIBits failed".into());
    }

    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
        chunk[3] = 255;
    }

    Ok(pixels)
}

fn encode_rgba_as_png(w: u32, h: u32, data: &[u8]) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("PNG header error: {}", e))?;
        writer
            .write_image_data(data)
            .map_err(|e| format!("PNG data error: {}", e))?;
    }
    Ok(buf)
}
