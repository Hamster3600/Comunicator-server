use std::thread;
use std::io::{self, Read, Write};
use std::collections::HashMap;
use tiny_http::{Server, Response};


fn main() {
    println!("[COMUNICATOR-SERVER] Uruchamiam węzeł serwera na wątkach...");

    let server = Server::http("0.0.0.0:4001").unwrap();
    println!("[COMUNICATOR-SERVER] Serwer gotowy na porcie 4001...");
    println!("--------------------------------------------------");

    for mut request in server.incoming_requests() {
        thread::spawn(move || {
            let mut body = String::new();
            if request.as_reader().read_to_string(&mut body).is_ok() {
                let czesci: Vec<&str> = body.split('|').map(|s| s.trim()).collect();

                if czesci.len() == 2 && czesci[0] == "REJESTRACJA" {
                    let nick = czesci[1];
                    
                    let nowy_hash = Uuid::new_v4().to_string();
                    
                    println!("\n[NOWY UŻYTKOWNIK] Rejestracja nicku: \"{}\"", nick);
                    println!("  -> Wygenerowano stały Hash: {}", nowy_hash);
                    println!("--------------------------------------------------");

                    let response = Response::from_string(nowy_hash);
                    let _ = request.respond(response);
                    return;
                }

                if czesci.len() == 4 {
                    let cel = czesci[0];
                    let ty = czesci[1];
                    let nik = czesci[2];
                    let wiadomosc = czesci[3];

                    println!("\n[PACZKA DANYCH]");
                    println!("  Od (Hash):   {}", ty);
                    println!("  Nick:        {}", nik);
                    println!("  Do (Hash):   {}", cel);
                    println!("  Wiadomość:   {}", wiadomosc);
                    println!("--------------------------------------------------");
                    
                    let response = Response::from_string("Serwer: Wiadomość przetworzona.");
                    let _ = request.respond(response);
                } else {
                    let response = Response::from_string("Serwer: Błędny format danych.");
                    let _ = request.respond(response);
                }
            }
        });
    }
}