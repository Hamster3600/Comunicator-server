use std::thread;
use std::io::{self, Read, Write};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tiny_http::{Server, Response};
use keyring_core::Entry;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct RegistrationPacket {
    nick: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct MessagePacket {
    target_hash: String,
    sender_hash: String,
    sender_nick: String,
    content: String,
}

type RoutingTable = Arc<Mutex<HashMap<String, String>>>;

fn forward_message(packet_body: &str, client_ip: String, routing_table: &RoutingTable, http_client: &reqwest::blocking::Client,) -> Result<String, String>{
    let message: MessagePacket = serde_json::from_str(packet_body).map_err(|e| format!("Failed to parse message packet: {}", e))?;

    let client_address = format!("{}:4001", client_ip);

    {
        let mut table = routing_table.lock().unwrap();
        table.insert(message.sender_hash.clone(), client_address);
    }

    let table = routing_table.lock().unwrap();
    if let Some(target_address) = table.get(&message.target_hash){
        let url = format!("http://{}", target_address);
        match http_client.post(&url).body(packet_body.to_string()).send(){
            Ok(_) => Ok(format!("Message rwarded to {} with hash {}", target_address, message.target_hash)),
            Err(e) => Err(format!("Target is registered, but connection to port 4001 failed: {}", e)),
        }
    }   else{
        Err(format!("Target hash {} is offline or not existing(not in routing table)", message.target_hash))
    }
}

fn main() {
    let config = HashMap::new();
    keyring::use_sqlite_store(&config).unwrap();

    let routing_table: RoutingTable = Arc::new(Mutex::new(HashMap::new()));
    let http_client = reqwest::blocking::Client::new();

    println!("[COMUNICATOR-SERVER] Running a server node on threads...");

    let server = Server::http("0.0.0.0:4001").unwrap();
    println!("[COMUNICATOR-SERVER] Server is ready on port 4001...");
    println!("--------------------------------------------------");

    for mut request in server.incoming_requests() {
        let routing_table = Arc::clone(&routing_table);
        let http_client = http_client.clone();
        thread::spawn(move || {
            let mut body = String::new();
            
            if request.as_reader().read_to_string(&mut body).is_ok() {
                
                if let Ok(packet_req) = serde_json::from_str::<RegistrationPacket>(&body) {
                    let nick = &packet_req.nick;

                    let check_entry_nick = Entry::new("Comunicator-server", &format!("{}_hash", nick)).unwrap();
                    if check_entry_nick.get_secret().is_ok() {
                        println!("[REJESTRACJA] Refusal. Nick \"{}\" is already taken.", nick);
                        let response = Response::from_string("[ERROR] Nick is already taken.");
                        let _ = request.respond(response);
                        return;
                    }
                    
                    let mut new_hash = String::new();
                    loop {
                        let potential_hash = Uuid::new_v4().to_string();
                        let is_unique = Entry::new("Comunicator-server", &potential_hash).unwrap().get_secret().is_err();
                        if is_unique {
                            new_hash = potential_hash;
                            break;
                        }
                    }
                    
                    println!("\n[NEW USER] Registration on nick: \"{}\"", nick);
                    println!("  -> Generated permanent Hash: {}", new_hash);
                    println!("--------------------------------------------------");

                    let entry_hash = Entry::new("Comunicator-server", &format!("{}_hash", nick)).unwrap();
                    let entry_nick = Entry::new("Comunicator-server", &format!("{}_nick", nick)).unwrap();
                    let entry_reverse = Entry::new("Comunicator-server", &new_hash).unwrap();
                    
                    entry_reverse.set_secret(nick.as_bytes()).unwrap();
                    entry_hash.set_secret(new_hash.as_bytes()).unwrap();
                    entry_nick.set_secret(nick.as_bytes()).unwrap();

                    let response = Response::from_string(new_hash);
                    let _ = request.respond(response);
                    return;
                } 
                
                if let Ok(packet_msg) = serde_json::from_str::<MessagePacket>(&body) {
                    println!("\n[PACZKA DANYCH JSON]");
                    println!("  From (Hash):   {}", packet_msg.sender_hash);
                    println!("  Nick:          {}", packet_msg.sender_nick);
                    println!("  To (Hash):     {}", packet_msg.target_hash);
                    println!("  Message:       {}", packet_msg.content);
                    println!("--------------------------------------------------");

                    let sender_ip = request.remote_addr().unwrap().ip().to_string();

                    if body.contains("target_hash") {
                        match forward_message(&body, sender_ip, &routing_table, &http_client) {
                            Ok(succes_msg) => {
                                println!("[SERVER] {}", succes_msg);
                                let response = tiny_http::Response::from_string("[SERVER] Message forwarded successfully.");
                                let _ = request.respond(response);
                            }
                            Err(error_msg) => {
                                println!("[ERROR] {}", error_msg);
                                let response = tiny_http::Response::from_string(format!("[SERVER] Error forwarding message: {}", error_msg));
                                let _ = request.respond(response);
                            }
                        }
                    }
                    return;
                }

                let response = Response::from_string("[SERVER] Invalid data format (Expected JSON).");
                let _ = request.respond(response);
            }
        });
        }
    }