use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use crate::server;

pub fn serve(port: u16) {
    let cwd = env::current_dir().unwrap_or_default();
    if !cwd.join("dist").join("glasshouse.bundle.js").exists() {
        eprintln!("glas: no build found in dist/");
        eprintln!("  Run 'glas build' first, or use 'glas dev' for development.");
        return;
    }
    let index = if let Ok(c) = fs::read_to_string(cwd.join("dist").join("index.html")) { c } else { server::generate_serve_index() };
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr) { Ok(l) => l, Err(e) => { eprintln!("glas: {}", e); return; } };
    println!("● Production server at http://localhost:{}", port);
    println!("  Serving from project root");
    println!("  Press Ctrl+C to stop");
    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => { let ix = index.clone(); let c = cwd.clone(); thread::spawn(move || { handle_serve(&mut s, &c, &ix); }); }
            Err(_) => {}
        }
    }
}

fn handle_serve(stream: &mut TcpStream, root: &std::path::Path, index: &str) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut rl = String::new(); if reader.read_line(&mut rl).is_err() { return; }
    let parts: Vec<&str> = rl.trim().split_whitespace().collect();
    if parts.len() < 2 { server::send_404(stream); return; }
    let rp = if parts[1] == "/" { "/dist/index.html" } else { parts[1] };
    if rp == "/dist/index.html" {
        let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", index.len(), index);
        let _ = stream.write_all(resp.as_bytes()); return;
    }
    server::serve_file(stream, root, rp);
}
