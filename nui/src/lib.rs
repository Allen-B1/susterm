use std::{net::SocketAddr, sync::{Arc, Weak, Mutex}, fmt::Write};

use dashmap::DashMap;
use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt, AsyncWrite}};

mod widgets;
pub use widgets::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black   = 0,
    Red     = 1,
    Green   = 2,
    Yellow  = 3,
    Blue    = 4,
    Magenta = 5,
    Cyan    = 6,
    White   = 7,
    Default = 9,
    BrightBlack   = 60,
    BrightRed     = 61,
    BrightGreen   = 62,
    BrightYellow  = 63,
    BrightBlue    = 64,
    BrightMagenta = 65,
    BrightCyan    = 66,
    BrightWhite   = 67,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Format {
    pub fg: Color,
    pub bg: Color,

    pub bold: bool,
    pub underline: bool,
}

impl Format {
    pub fn write(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(writer, "\x1b[{};{};{};{}m",
            if self.bold { 1 } else { 22 },
            if self.underline { 4 } else { 24 },
            self.fg as u8 + 30,
            self.bg as u8 + 40,
        )
    }
}

#[derive(Clone)]
pub struct ScreenBuffer {
    pub chars: Vec<u8>,
    pub formats: Vec<Format>,
    pub width: usize,
}

pub trait Widget: Send {
    /// Draw onto the given buffer. Returns the
    /// cursor position, which used if the widget
    /// is focused.
    fn draw(&self, buf: &mut ScreenBuffer) -> (usize, usize);

    fn focusable(&self) -> bool { false }
    fn keypress(&mut self, ch: u8) {}
}

pub struct Screen {
    pub widgets: Vec<Mutex<Box<dyn Widget>>>,
    pub active: Option<usize>,
    pub width: usize,
    pub height: usize,
}

impl Screen {
    pub async fn draw(&self, writer: &mut (impl AsyncWrite + Unpin), old_buf: ScreenBuffer) -> ScreenBuffer {
        let mut new_buf=  ScreenBuffer {
            chars: vec![' ' as u8 ; old_buf.chars.len()],
            formats: vec![Format { fg: Color::White, bg: Color::Black, bold: false, underline: false }; old_buf.chars.len()],
            width: old_buf.width
        };

        let mut cursor_x: usize = 0;
        let mut cursor_y: usize = 0;
        // Draw all widgets onto the new buffer
        for (i, widget) in self.widgets.iter().enumerate() {
            let (x, y) = widget.lock().expect("widget poisoning").draw(&mut new_buf);
            if self.active == Some(i) {
                cursor_x = x;
                cursor_y = y;
            }
        }

        // Write message onto a string buffer first,
        // before sending it over TCP.
        let mut msg = String::new();
        for idx in 0..new_buf.chars.len() {
            if new_buf.chars[idx] != old_buf.chars[idx] || new_buf.formats[idx] != old_buf.formats[idx] {
                let x = idx % new_buf.width;
                let y = idx / new_buf.width;

                write!(msg, "\x1b[{};{}H", y, x).unwrap();
                new_buf.formats[idx].write(&mut msg).unwrap();
                write!(msg, "{}", new_buf.chars[idx] as char).unwrap();
            }
        }
        write!(msg, "\x1b[{};{}H", cursor_y, cursor_x);

        // Send the message over TCP.
        if let Err(err) = writer.write(msg.as_bytes()).await { 
            eprintln!("error drawing screen: {}", err);
        }

        new_buf
    }
}

pub struct Server {
    srv: TcpListener,
    streams: DashMap<SocketAddr, TcpStream>,
    screens: DashMap<SocketAddr, Screen>,

    handle_connect: Box<dyn (Fn(Weak<Self>, SocketAddr) -> Screen) + Sync + Send + 'static>,
}

impl Server {    
    pub fn new(srv: TcpListener, handle_connect: impl (Fn(Weak<Self>, SocketAddr) -> Screen) + Sync + Send + 'static) -> Arc<Self> {
        Arc::new(Server {
            srv, 

            streams: DashMap::new(),
            screens: DashMap::new(),

            handle_connect: Box::new(handle_connect),
        })
    }

    /// Runs an infinite loop handling events for the given address.
    /// Invariants: `TcpStream` and `Screen` must be set.
    async fn event_thread(this: Weak<Self>, addr: SocketAddr) {
        let mut buffer: ScreenBuffer;
        {   // Clear screen & initialize buffer
            let this = this.upgrade();
            let this = match this {
                None => return,
                Some(v) => v,
            };

            let stream = this.streams.get_mut(&addr);
            let mut stream = match stream {
                None => return,
                Some(v) => v,
            };
            
            match stream.write("\x1bc\x1b[49m\x1b[H\x1b[2J\x1b[3J".as_bytes()).await {
                Ok(_) => {},
                Err(screen_err) => {
                    dbg!(screen_err);
                    return;
                }
            }

            let screen = this.screens.get(&addr);
            let screen = match screen {
                None => return,
                Some(v) => v,
            };

            buffer = ScreenBuffer {
                chars: vec![' ' as u8; screen.width * screen.height],
                formats: vec![Format { fg: Color::Default, bg: Color::Default, bold: false, underline: false }; screen.width * screen.height ],
                width: screen.width,
            };
            buffer = screen.draw(stream.value_mut(), buffer).await;

            drop(screen);
            drop(stream);
        }


        loop {
            let this = this.upgrade();
            let this = match this {
                None => return,
                Some(v) => v,
            };

            let stream = this.streams.get_mut(&addr);
            let mut stream = match stream {
                None => return,
                Some(v) => v,
            };

            let screen = this.screens.get_mut(&addr);
            let mut screen = match screen {
                None => continue,
                Some(v) => v,
            };

            let mut buf: [u8; 1] = [0u8];
            match stream.read(&mut buf).await {
                Ok(bytes) => {
                    if bytes == 0 {
                        return
                    }

                    let ch = buf[0];
                    if ch == '\t' as u8 {
                        let start = match screen.active {
                            Some(i) => i,
                            None => screen.widgets.len() - 1
                        };

                        for i in 0..screen.widgets.len() {
                            if screen.widgets[(i + 1 + start) % screen.widgets.len()].lock().expect("widget poisoning").focusable() {
                                screen.active = Some((i + 1 + start) % screen.widgets.len());
                                break
                            }
                        }
                    } else {
                        if let Some(active) = screen.active {
                            screen.widgets[active].lock().expect("widget lock poisoned").keypress(ch);
                        }    
                    }
                },
                Err(read_err) => {
                    eprintln!("error reading from client: {}", read_err);
                    return;
                }
            }

            buffer = screen.draw(stream.value_mut(), buffer).await;

            drop(screen);
        }
    }

    pub async fn serve(this: &Arc<Self>) {
        loop {
            match this.srv.accept().await {
                Err(e) => {
                    dbg!(e);
                },
                Ok((stream, addr)) => {
                    this.streams.insert(addr, stream);
                    this.screens.insert(addr, (this.handle_connect)(Arc::downgrade(this), addr));

                    let this = Arc::downgrade(this);
                    tokio::spawn(Server::event_thread(this, addr));
                }
            }
        }
    }
}