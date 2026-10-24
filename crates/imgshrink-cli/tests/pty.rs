use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

#[test]
fn tui_starts_and_restores_terminal_on_quit() {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("no se pudo abrir el PTY");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_imgshrink"));
    command.arg("--no-color");
    command.arg("--ascii");
    command.env("TERM", "xterm-256color");
    let mut child = pair
        .slave
        .spawn_command(command)
        .expect("no se pudo lanzar imgshrink");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let _ = tx.send(buffer[..count].to_vec());
                }
                Err(_) => break,
            }
        }
    });

    let mut output = Vec::new();
    let deadline = Instant::now() + Duration::from_millis(2500);
    while Instant::now() < deadline {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
            output.extend_from_slice(&chunk);
        }
    }
    eprintln!("--- salida inicial ({} bytes) ---", output.len());
    eprintln!("{}", String::from_utf8_lossy(&output));
    eprintln!("--- fin salida inicial ---");

    if String::from_utf8_lossy(&output).contains("[6n") {
        eprintln!("respondiendo a la consulta de posición del cursor de ConPTY");
        writer.write_all(b"\x1b[1;1R").expect("responder CPR");
        writer.flush().expect("flush CPR");
        std::thread::sleep(Duration::from_millis(1200));
        while let Ok(chunk) = rx.recv_timeout(Duration::from_millis(150)) {
            output.extend_from_slice(&chunk);
        }
        eprintln!("--- salida tras CPR ({} bytes) ---", output.len());
        eprintln!("{}", String::from_utf8_lossy(&output));
    }

    writer.write_all(b"q").expect("escribir q");
    writer.flush().expect("flush");
    eprintln!("q enviada");

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    while let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
        output.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&output);
    eprintln!("--- salida total ({} bytes) ---", output.len());
    eprintln!("{}", text);
    assert!(status.is_some(), "el proceso no terminó tras enviar q");
    assert!(status.unwrap().success(), "el proceso terminó con error");
    assert!(text.contains("IMGSHRINK"), "la TUI no llegó a dibujarse");
    assert!(
        text.contains("?1049l") || text.contains("?1049h"),
        "no se observaron secuencias de pantalla alternativa"
    );
    assert!(
        text.contains("?25h") || text.contains("?25l"),
        "no se observaron secuencias de cursor"
    );
}
