use alloc::string::String;
use core::{
    fmt::{self, Arguments, Write},
    ptr,
};

use boot::{BootInfo, FrameBufferPixelFormat};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle},
    text::{Baseline, Text},
};
use spin::Mutex;
use x86_64::instructions::port::Port;

use crate::proc;

#[derive(Copy, Clone)]
struct ClockTime {
    hour: usize,
    minute: usize,
    second: usize,
}

const BYTES_PER_PIXEL: usize = 4;
const CHAR_WIDTH: usize = 6;
const CHAR_HEIGHT: usize = 10;
const TERM_PADDING: usize = 4;
const OUTPUT_QUEUE_SIZE: usize = 65535;

once_mutex!(pub VGA: VgaDisplay);
guard_access_fn!(pub get_vga(VGA: VgaDisplay));

static OUTPUT_QUEUE: Mutex<OutputQueue> = Mutex::new(OutputQueue::new());

pub struct VgaDisplay {
    buffer: *mut u8,
    size: usize,
    width: usize,
    height: usize,
    stride: usize,
    pixel_format: FrameBufferPixelFormat,
    term_top: usize,
    cursor_x: usize,
    cursor_y: usize,
    foreground: Rgb888,
    background: Rgb888,
    escape_active: bool,
}

unsafe impl Send for VgaDisplay {}

pub fn init(boot_info: &'static BootInfo) {
    let Some(info) = boot_info.framebuffer else {
        return;
    };

    if info.bytes_per_pixel != BYTES_PER_PIXEL {
        warn!(
            "Unsupported framebuffer pixel size: {} bytes",
            info.bytes_per_pixel
        );
        return;
    }

    let buffer = (boot_info.physical_memory_offset + info.phys_addr) as *mut u8;
    let mut display = VgaDisplay {
        buffer,
        size: info.size,
        width: info.width,
        height: info.height,
        stride: info.stride,
        pixel_format: info.pixel_format,
        term_top: info.height / 2,
        cursor_x: TERM_PADDING,
        cursor_y: info.height / 2 + TERM_PADDING,
        foreground: Rgb888::new(230, 238, 220),
        background: Rgb888::new(7, 10, 12),
        escape_active: false,
    };

    display.clear_screen();
    display.draw_static_layout();
    init_VGA(display);
}

pub fn spawn_clock_thread() {
    if VGA.get().is_some() {
        proc::spawn_kernel_thread(clock_thread, String::from("vga-clock"), None);
    }
}

pub fn clear_terminal() {
    if let Some(vga) = VGA.get() {
        vga.lock().terminal_clear();
    }
}

pub fn write_fmt(args: Arguments) -> fmt::Result {
    let Some(vga) = VGA.get() else {
        return Ok(());
    };

    if let Some(mut vga) = vga.try_lock() {
        vga.flush_output_queue();
        vga.write_fmt(args)?;
        return Ok(());
    }

    QueueWriter.write_fmt(args)
}

fn clock_thread() -> ! {
    let mut last_second = usize::MAX;

    loop {
        let time = current_utc8_time();

        if let Some(mut vga) = get_vga() {
            if time.second != last_second {
                vga.flush_output_queue();
                vga.draw_clock(time);
                last_second = time.second;
            }
            vga.flush_output_queue();
        }

        for _ in 0..32 {
            x86_64::instructions::hlt();
        }
    }
}

impl VgaDisplay {
    fn pixel_offset(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let offset = (y * self.stride + x) * BYTES_PER_PIXEL;
        (offset + BYTES_PER_PIXEL <= self.size).then_some(offset)
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        let Some(offset) = self.pixel_offset(x, y) else {
            return;
        };

        unsafe {
            let pixel = self.buffer.add(offset);
            match self.pixel_format {
                FrameBufferPixelFormat::Rgb => {
                    ptr::write_volatile(pixel, color.r());
                    ptr::write_volatile(pixel.add(1), color.g());
                    ptr::write_volatile(pixel.add(2), color.b());
                }
                FrameBufferPixelFormat::Bgr => {
                    ptr::write_volatile(pixel, color.b());
                    ptr::write_volatile(pixel.add(1), color.g());
                    ptr::write_volatile(pixel.add(2), color.r());
                }
            }
            ptr::write_volatile(pixel.add(3), 0);
        }
    }

    pub fn clear_screen(&mut self) {
        self.fill_rect(0, 0, self.width, self.height, self.background);
    }

    fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Rgb888) {
        let max_y = y.saturating_add(height).min(self.height);
        let max_x = x.saturating_add(width).min(self.width);
        let value = self.pixel_value(color);

        for py in y..max_y {
            for px in x..max_x {
                let Some(offset) = self.pixel_offset(px, py) else {
                    continue;
                };
                unsafe {
                    ptr::write_volatile(self.buffer.add(offset).cast::<u32>(), value);
                }
            }
        }
    }

    fn pixel_value(&self, color: Rgb888) -> u32 {
        match self.pixel_format {
            FrameBufferPixelFormat::Rgb => {
                u32::from(color.r()) | (u32::from(color.g()) << 8) | (u32::from(color.b()) << 16)
            }
            FrameBufferPixelFormat::Bgr => {
                u32::from(color.b()) | (u32::from(color.g()) << 8) | (u32::from(color.r()) << 16)
            }
        }
    }

    fn draw_static_layout(&mut self) {
        let accent = Rgb888::new(92, 180, 168);
        let line_y = self.term_top.saturating_sub(1) as i32;

        let _ = Line::new(Point::new(0, line_y), Point::new(self.width as i32, line_y))
            .into_styled(PrimitiveStyle::with_stroke(accent, 2))
            .draw(self);

        let style = MonoTextStyle::new(&FONT_6X10, accent);
        let _ = Text::with_baseline(
            "YatSenOS VGA Display",
            Point::new(TERM_PADDING as i32, TERM_PADDING as i32),
            style,
            Baseline::Top,
        )
        .draw(self);
    }

    fn terminal_clear(&mut self) {
        self.fill_rect(
            0,
            self.term_top,
            self.width,
            self.height - self.term_top,
            self.background,
        );
        self.cursor_x = TERM_PADDING;
        self.cursor_y = self.term_top + TERM_PADDING;
    }

    fn newline(&mut self) {
        self.cursor_x = TERM_PADDING;
        self.cursor_y += CHAR_HEIGHT;

        if self.cursor_y + CHAR_HEIGHT >= self.height {
            self.scroll_terminal();
            self.cursor_y = self.height.saturating_sub(TERM_PADDING + CHAR_HEIGHT);
        }
    }

    fn backspace(&mut self) {
        if self.cursor_x >= TERM_PADDING + CHAR_WIDTH {
            self.cursor_x -= CHAR_WIDTH;
            self.fill_rect(
                self.cursor_x,
                self.cursor_y,
                CHAR_WIDTH,
                CHAR_HEIGHT,
                self.background,
            );
        }
    }

    fn scroll_terminal(&mut self) {
        let src_start = self.term_top + TERM_PADDING + CHAR_HEIGHT;
        let dst_start = self.term_top + TERM_PADDING;
        let copy_height = self.height.saturating_sub(src_start + TERM_PADDING);
        let row_bytes = self.stride * BYTES_PER_PIXEL;
        let src = src_start * row_bytes;
        let dst = dst_start * row_bytes;
        let copy_bytes = copy_height * row_bytes;

        if src + copy_bytes <= self.size && dst + copy_bytes <= self.size {
            unsafe {
                ptr::copy(self.buffer.add(src), self.buffer.add(dst), copy_bytes);
            }
        }

        self.fill_rect(
            0,
            self.height.saturating_sub(TERM_PADDING + CHAR_HEIGHT),
            self.width,
            TERM_PADDING + CHAR_HEIGHT,
            self.background,
        );
    }

    fn write_visible_char(&mut self, ch: char) {
        if self.cursor_x + CHAR_WIDTH >= self.width {
            self.newline();
        }

        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        let style = MonoTextStyle::new(&FONT_6X10, self.foreground);
        let _ = Text::with_baseline(
            s,
            Point::new(self.cursor_x as i32, self.cursor_y as i32),
            style,
            Baseline::Top,
        )
        .draw(self);

        self.cursor_x += CHAR_WIDTH;
    }

    fn write_char_to_terminal(&mut self, ch: char) {
        if self.escape_active {
            if ch.is_ascii_alphabetic() {
                self.escape_active = false;
            }
            return;
        }

        match ch {
            '\x1b' => {
                self.terminal_clear();
                self.escape_active = true;
            }
            '\n' => self.newline(),
            '\r' => self.cursor_x = TERM_PADDING,
            '\x08' | '\x7f' => self.backspace(),
            ch if !ch.is_control() => self.write_visible_char(ch),
            _ => {}
        }
    }

    fn draw_clock(&mut self, time: ClockTime) {
        let graph_height = self.term_top.saturating_sub(2);
        let radius = (graph_height.min(self.width) / 2)
            .saturating_sub(26)
            .max(20);
        let center = Point::new((self.width / 2) as i32, (graph_height / 2 + 10) as i32);
        let cyan = Rgb888::new(92, 180, 168);
        let amber = Rgb888::new(238, 188, 93);
        let red = Rgb888::new(214, 95, 86);
        let dim = Rgb888::new(68, 86, 88);

        let clear_margin = radius + 18;
        let clear_x = (center.x - clear_margin as i32).max(0) as usize;
        let clear_y = (center.y - clear_margin as i32).max(0) as usize;
        let clear_size = clear_margin * 2;
        self.fill_rect(clear_x, clear_y, clear_size, clear_size, self.background);
        self.fill_rect(
            TERM_PADDING,
            graph_height.saturating_sub(22),
            180,
            CHAR_HEIGHT + 4,
            self.background,
        );
        self.draw_static_layout();

        let _ = Circle::with_center(center, (radius * 2) as u32)
            .into_styled(PrimitiveStyle::with_stroke(cyan, 2))
            .draw(self);

        for mark in 0..12 {
            let angle = mark as f32 / 12.0 * core::f32::consts::TAU - core::f32::consts::FRAC_PI_2;
            let outer = clock_point(center, radius as f32, angle);
            let inner = clock_point(center, radius.saturating_sub(8) as f32, angle);
            let _ = Line::new(inner, outer)
                .into_styled(PrimitiveStyle::with_stroke(dim, 2))
                .draw(self);
        }

        let hour12 = time.hour % 12;

        self.draw_hand(center, radius as f32 * 0.82, time.second, 60, red, 1);
        self.draw_hand(center, radius as f32 * 0.65, time.minute, 60, amber, 3);
        self.draw_hand(center, radius as f32 * 0.45, hour12, 12, cyan, 4);

        let label = {
            use alloc::format;
            format!(
                "UTC+8 {:02}:{:02}:{:02}",
                time.hour, time.minute, time.second
            )
        };
        let style = MonoTextStyle::new(&FONT_6X10, amber);
        let _ = Text::with_baseline(
            label.as_str(),
            Point::new(
                TERM_PADDING as i32,
                (graph_height.saturating_sub(18)) as i32,
            ),
            style,
            Baseline::Top,
        )
        .draw(self);
    }

    fn draw_hand(
        &mut self,
        center: Point,
        length: f32,
        value: usize,
        modulo: usize,
        color: Rgb888,
        width: u32,
    ) {
        let angle =
            value as f32 / modulo as f32 * core::f32::consts::TAU - core::f32::consts::FRAC_PI_2;
        let end = clock_point(center, length, angle);
        let _ = Line::new(center, end)
            .into_styled(PrimitiveStyle::with_stroke(color, width))
            .draw(self);
    }

    fn flush_output_queue(&mut self) {
        while let Some(byte) = pop_output_byte() {
            self.write_char_to_terminal(byte as char);
        }
    }
}

fn pop_output_byte() -> Option<u8> {
    OUTPUT_QUEUE.try_lock().and_then(|mut queue| queue.pop())
}

struct OutputQueue {
    buf: [u8; OUTPUT_QUEUE_SIZE],
    head: usize,
    tail: usize,
    full: bool,
}

impl OutputQueue {
    const fn new() -> Self {
        Self {
            buf: [0; OUTPUT_QUEUE_SIZE],
            head: 0,
            tail: 0,
            full: false,
        }
    }

    fn push(&mut self, byte: u8) -> bool {
        if self.full {
            self.head = (self.head + 1) % OUTPUT_QUEUE_SIZE;
            self.full = false;
        }

        self.buf[self.tail] = byte;
        self.tail = (self.tail + 1) % OUTPUT_QUEUE_SIZE;
        self.full = self.tail == self.head;
        true
    }

    fn pop(&mut self) -> Option<u8> {
        if !self.full && self.head == self.tail {
            return None;
        }

        let byte = self.buf[self.head];
        self.head = (self.head + 1) % OUTPUT_QUEUE_SIZE;
        self.full = false;
        Some(byte)
    }
}

struct QueueWriter;

impl Write for QueueWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let Some(mut queue) = OUTPUT_QUEUE.try_lock() else {
            return Ok(());
        };

        for byte in s.bytes() {
            queue.push(byte);
        }
        Ok(())
    }
}

fn current_utc8_time() -> ClockTime {
    read_cmos_time().unwrap_or_else(|| match uefi::runtime::get_time() {
        Ok(time) => ClockTime {
            hour: (usize::from(time.hour()) + 8) % 24,
            minute: usize::from(time.minute()),
            second: usize::from(time.second()),
        },
        Err(_) => ClockTime {
            hour: 0,
            minute: 0,
            second: 0,
        },
    })
}

fn read_cmos_time() -> Option<ClockTime> {
    let status_b = cmos_read(0x0B);
    let binary = status_b & 0x04 != 0;
    let hour_24 = status_b & 0x02 != 0;

    for _ in 0..10 {
        wait_cmos_ready()?;

        let second = cmos_read(0x00);
        let minute = cmos_read(0x02);
        let hour = cmos_read(0x04);

        wait_cmos_ready()?;

        if second == cmos_read(0x00) && minute == cmos_read(0x02) && hour == cmos_read(0x04) {
            let mut hour = hour;
            let pm = !hour_24 && hour & 0x80 != 0;
            hour &= 0x7F;

            let second = cmos_value(second, binary);
            let minute = cmos_value(minute, binary);
            let mut hour = cmos_value(hour, binary);

            if !hour_24 {
                hour %= 12;
                if pm {
                    hour += 12;
                }
            }

            return Some(ClockTime {
                hour: (usize::from(hour) + 8) % 24,
                minute: usize::from(minute),
                second: usize::from(second),
            });
        }
    }

    None
}

fn wait_cmos_ready() -> Option<()> {
    for _ in 0..100_000 {
        if cmos_read(0x0A) & 0x80 == 0 {
            return Some(());
        }
        core::hint::spin_loop();
    }

    None
}

fn cmos_value(value: u8, binary: bool) -> u8 {
    if binary {
        value
    } else {
        (value & 0x0F) + ((value >> 4) * 10)
    }
}

fn cmos_read(register: u8) -> u8 {
    unsafe {
        let mut addr = Port::<u8>::new(0x70);
        let mut data = Port::<u8>::new(0x71);
        addr.write(0x80 | register);
        data.read()
    }
}

fn clock_point(center: Point, length: f32, angle: f32) -> Point {
    Point::new(
        center.x + (libm::cosf(angle) * length) as i32,
        center.y + (libm::sinf(angle) * length) as i32,
    )
}

impl DrawTarget for VgaDisplay {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 {
                self.put_pixel(point.x as usize, point.y as usize, color);
            }
        }
        Ok(())
    }
}

impl OriginDimensions for VgaDisplay {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl Write for VgaDisplay {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.chars() {
            self.write_char_to_terminal(ch);
        }
        Ok(())
    }
}
