use std::sync::Mutex;

use nui::{self, Color};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_entry() {
    let tcp = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    let srv = nui::Server::new(tcp, 
        |srv, addr| {
            nui::Screen  {
                active: None,
                widgets: vec![
                    Mutex::new(Box::new(nui::Entry {
                        x: 12, y: 12, 
                        format: nui::Format { fg: Color::Red, bg: Color::Black, underline: true, bold: false},
                        text: vec!['H' as u8, 'i' as u8],
                        max: 16,
                    })),
                    Mutex::new(Box::new(nui::Entry {
                        x: 12, y: 24, 
                        format: nui::Format { fg: Color::Green, bg: Color::Black, underline: true, bold: false},
                        text: vec!['B' as u8, 'y' as u8, 'e' as u8],
                        max: 16,
                    }))
                ],

                width: 128,
                height: 64,
            }
        });
    
    nui::Server::serve(&srv).await;
}