use std::thread;
use std::io::{self, Read, Write};
use std::collections::HashMap;
use tiny_http::{Server, Response};
use keyring_core::Entry;
use uuid::Uuid;

fn main() {
    let config = HashMap::new();
    keyring::use_sqlite_store(&config).unwrap();

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

                    let wpis_sprawdzenia_nicku = Entry::new("Comunicator-server", &(nick.to_string() + "_hash")).unwrap();
                    if wpis_sprawdzenia_nicku.get_secret().is_ok() {
                        println!("[REJESTRACJA] Odmowa. Nick \"{}\" jest już zajęty.", nick);
                        let response = Response::from_string("BŁĄD: Nick jest już zajęty.");
                        let _ = request.respond(response);
                        return;
                    }
                    
                    let mut nowy_hash = String::new();
                    loop{
                        let potencjalny_hash = Uuid::new_v4().to_string();
                        let czyUnikalne = Entry::new("Comunicator-server", &potencjalny_hash).unwrap().get_secret().is_err();
                        if czyUnikalne {
                            nowy_hash = potencjalny_hash;
                            break;
                        }
                    }
                    
                    println!("\n[NOWY UŻYTKOWNIK] Rejestracja nicku: \"{}\"", nick);
                    println!("  -> Wygenerowano stały Hash: {}", nowy_hash);
                    println!("--------------------------------------------------");

                    let wpis_hash = Entry::new("Comunicator-server", &(nick.to_string() + "_hash")).unwrap();
                    let wpis_nick = Entry::new("Comunicator-server", &(nick.to_string() + "_nick")).unwrap();
                    let wpis_odwrotny = Entry::new("Comunicator-server", &nowy_hash).unwrap();
                    wpis_odwrotny.set_secret(nick.as_bytes()).unwrap();

                    wpis_hash.set_secret(nowy_hash.as_bytes()).unwrap();
                    wpis_nick.set_secret(nick.as_bytes()).unwrap();

                    if wpis_hash.get_secret().is_ok() && wpis_nick.get_secret().is_ok() {
                        println!("[INFO] Dane zostały zapisane w bezpiecznym magazynie SQLite.");
                    } else {
                        println!("[BŁĄD] Nie udało się zapisać danych.");
                    }

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