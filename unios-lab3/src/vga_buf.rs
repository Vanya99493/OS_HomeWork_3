use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::{interrupts::without_interrupts, port::{Port, PortGeneric, ReadWriteAccess}};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buf::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    without_interrupts(|| {
        SCREEN.lock().write_fmt(args).unwrap();
    });
}

const BUF_HEIGHT: u32 = 25;
const BUF_WIDTH: u32 = 80;
const BUF_SIZE: usize = (BUF_HEIGHT * BUF_WIDTH * 2) as usize;

lazy_static! {
    pub static ref SCREEN: Mutex<Screen> = Mutex::new(
        {
            let mut screen = Screen {
                color: 0xa,
                buffer: unsafe {&mut *(0xb8000 as *mut [u8; BUF_SIZE])},
                line: 0,
                col: 0
            };
            screen.clear();
            screen
        }
    );
}

pub struct AsciiChar {
    pub char_byte: u8,
    pub color_byte: u8,
}

pub struct Screen {
    color: u8,
    pub buffer: &'static mut [u8; BUF_SIZE],
    pub line: u32,
    pub col: u32
}

impl core::fmt::Write for Screen {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.print(s);
        Ok(())
    }
}

impl Screen {
    pub fn clear(&mut self) {
        for i in 0..BUF_HEIGHT {
            for j in 0..BUF_WIDTH {
                self.write_char_byte(i * BUF_WIDTH + j, 0x00)
            }
        }
        self.col = 0;
        self.line = 0;
        self.move_cursor();
    }

    pub fn print(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                b'\n' => {
                    if self.line == BUF_HEIGHT - 1 {
                        self.scroll_up();
                    } else {
                        self.line += 1;
                    }
                    self.col = 0;
                }
                b => {
                    self.write_char_byte(self.line * BUF_WIDTH + self.col, b);
                    self.col += 1;
                    if self.col == BUF_WIDTH {
                        self.col = 0;
                        self.print("\n");
                    }
                }
            }
            self.move_cursor();
        }
    }

    pub fn get_buffer(&mut self) -> [u8; (BUF_HEIGHT * BUF_WIDTH) as usize]{
        let mut buf = [b' '; (BUF_HEIGHT * BUF_WIDTH) as usize];
        for i in 0..BUF_HEIGHT{
            for j in 0..BUF_WIDTH{
                buf[(i * BUF_WIDTH + j) as usize] = self.read_char(i * BUF_WIDTH + j).char_byte;
            }
        }
        return buf;
    }

    pub fn delete_last_char(&mut self, min_index:u32){
        if self.col > min_index{
            self.col -= 1;
        }
        self.write_char_byte(self.line * BUF_WIDTH + self.col, b' ');
        self.move_cursor();
    }

    pub fn set_cursor_position(&mut self, position: u16){
        unsafe{
            let mut cmd_port: PortGeneric<u16, ReadWriteAccess> = Port::new(0x3D4);
            let mut data_port: PortGeneric<u16, ReadWriteAccess> = Port::new(0x3D5);

            cmd_port.write(14 as u16);
            data_port.write((position >> 8) & 0x00FF);
            cmd_port.write(15 as u16);
            data_port.write(position & 0x00FF);
        }
    }

    pub fn move_cursor(&mut self){
        self.set_cursor_position((self.line * BUF_WIDTH + self.col) as u16);
    }

    pub fn shift_line_to_right(&mut self, row_start:u32){
        let mut curr_column = BUF_WIDTH - 2;
        while curr_column != row_start{
            self.write_char(self.line * BUF_WIDTH + curr_column + 1, self.read_char(self.line * BUF_WIDTH + curr_column));
            curr_column -= 1;
        }
        self.write_char(self.line * BUF_WIDTH + curr_column + 1, self.read_char(self.line * BUF_WIDTH + curr_column));
    }

    pub fn shift_input_to_col(&mut self, x:u32){
        self.col = x;
    }

    fn scroll_up(&mut self) {
        for i in 0..self.line {
            for j in 0..BUF_WIDTH {
                let char_to_copy = self.read_char(BUF_WIDTH * (i + 1) + j);
                self.write_char(BUF_WIDTH * i + j, char_to_copy);
            }
        }
        for i in 0..BUF_WIDTH {
            self.write_char(self.line * BUF_WIDTH + i, AsciiChar { char_byte: b' ', color_byte: 0x00 });
        }
    }

    fn write_char_byte(&mut self, offset: u32, char_byte: u8) {
        self.write_char(offset, AsciiChar { char_byte, color_byte: self.color })
    }

    fn write_char(&mut self, offset: u32, char: AsciiChar) {
        self.buffer[offset as usize * 2] = char.char_byte;
        self.buffer[offset as usize * 2 + 1] = char.color_byte;
    }

    fn read_char(&self, offset: u32) -> AsciiChar {
        unsafe {
            return AsciiChar {
                char_byte: self.buffer[offset as usize * 2],
                color_byte: self.buffer[offset as usize * 2 + 1],
            };
        }
    }
}