use std::thread;
use std::io::{self, Read, Write};
use std::collections::HashMap;
use tiny_http::{Server, Response};
use keyring_core::Entry;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

// STRUKTURY JSON
#[derive(Serialize, Deserialize, Debug)]
struct RejestracjaPaczka {
    nick: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct WiadomoscPaczka {
    cel_hash: String,
    nadawca_hash: String,
    nadawca_nick: String,
    tresc: String,
}

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
                
                if let Ok(paczka_rej) = serde_json::from_str::<RejestracjaPaczka>(&body) {
                    let nick = &paczka_rej.nick;

                    let wpis_sprawdzenia_nicku = Entry::new("Comunicator-server", &format!("{}_hash", nick)).unwrap();
                    if wpis_sprawdzenia_nicku.get_secret().is_ok() {
                        println!("[REJESTRACJA] Odmowa. Nick \"{}\" jest już zajęty.", nick);
                        let response = Response::from_string("BŁĄD: Nick jest już zajęty.");
                        let _ = request.respond(response);
                        return;
                    }
                    
                    let mut nowy_hash = String::new();
                    loop {
                        let potencjalny_hash = Uuid::new_v4().to_string();
                        let czy_unikalne = Entry::new("Comunicator-server", &potencjalny_hash).unwrap().get_secret().is_err();
                        if czy_unikalne {
                            nowy_hash = potencjalny_hash;
                            break;
                        }
                    }
                    
                    println!("\n[NOWY UŻYTKOWNIK] Rejestracja nicku: \"{}\"", nick);
                    println!("  -> Wygenerowano stały Hash: {}", nowy_hash);
                    println!("--------------------------------------------------");

                    let wpis_hash = Entry::new("Comunicator-server", &format!("{}_hash", nick)).unwrap();
                    let wpis_nick = Entry::new("Comunicator-server", &format!("{}_nick", nick)).unwrap();
                    let wpis_odwrotny = Entry::new("Comunicator-server", &nowy_hash).unwrap();
                    
                    wpis_odwrotny.set_secret(nick.as_bytes()).unwrap();
                    wpis_hash.set_secret(nowy_hash.as_bytes()).unwrap();
                    wpis_nick.set_secret(nick.as_bytes()).unwrap();

                    let response = Response::from_string(nowy_hash);
                    let _ = request.respond(response);
                    return;
                } 
                
                if let Ok(paczka_msg) = serde_json::from_str::<WiadomoscPaczka>(&body) {
                    println!("\n[PACZKA DANYCH JSON]");
                    println!("  Od (Hash):   {}", paczka_msg.nadawca_hash);
                    println!("  Nick:        {}", paczka_msg.nadawca_nick);
                    println!("  Do (Hash):   {}", paczka_msg.cel_hash);
                    println!("  Wiadomość:   {}", paczka_msg.tresc);
                    println!("--------------------------------------------------");
                    
                    let response = Response::from_string("Serwer: Wiadomość przetworzona.");
                    let _ = request.respond(response);
                    return;
                }

                let response = Response::from_string("Serwer: Błędny format danych (Oczekiwano JSON).");
                let _ = request.respond(response);
            }
        });
    }
}