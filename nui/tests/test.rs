use std::sync::{Mutex, Arc, Weak};

use nui::{self, Color};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_entry() {
    let tcp = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    let mut weak_srv: Weak<nui::Server> = Weak::new();
    let shared_entry: Arc<Mutex<Box<dyn nui::Widget>>> = Arc::new(Mutex::new(Box::new(nui::Entry {
        x: 12, y: 12, 
        format: nui::Format { fg: Color::BrightGreen, bg: Color::Black, underline: true, bold: false},
        text: vec!['H' as u8, 'i' as u8],
        max: 16,
        handle_input: Box::new(move |text| {
            let srv = weak_srv.clone().upgrade();
            eprintln!("problem: weak_srv is copied by value {}", srv.is_none());
            if let Some(srv) = srv {
                tokio::spawn(async move { srv.redraw_all().await });
            }
        }),
    })));

    let srv = nui::Server::new(tcp, 
        move |srv, addr| {
            let personal_entry:  Arc<Mutex<Box<dyn nui::Widget>>> = Arc::new(Mutex::new(Box::new(nui::Entry {
                x: 12, y: 24, 
                format: nui::Format { fg: Color::BrightMagenta, bg: Color::Black, underline: true, bold: false},
                text: vec!['H' as u8, 'i' as u8],
                max: 16,
                handle_input: Box::new(|text| {}),
            })));

            nui::Screen::new(vec![
                Arc::clone(&shared_entry),
                personal_entry,
            ], None, 128, 32)
        });
    weak_srv = Arc::downgrade(&srv);
    
    nui::Server::serve(&srv).await;
}