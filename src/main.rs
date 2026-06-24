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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MessagePacket {
    target_hash: String,
    sender_hash: String,
    sender_nick: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ListRequest{
    action: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct UserInfo{
    nick: String,
    hash: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ListResponse{
    users: Vec<UserInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChangeNickRequest {
    action: String,
    hash: String,
    new_nick: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChangeHashRequest {
    action: String,
    old_hash: String,
}

type PendingMessages = Arc<Mutex<HashMap<String, Vec<MessagePacket>>>>;

type RoutingTable = Arc<Mutex<HashMap<String, String>>>;

fn forward_message(packet_body: &str, client_ip: String, routing_table: &RoutingTable, http_client: &reqwest::blocking::Client, pending: &PendingMessages) -> Result<String, String>{
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
            Err(e) => {
                pending.lock().unwrap().entry(message.target_hash.clone()).or_default().push(message.clone());
                Err(format!("Target is registered, but connection to port 4001 failed: {}", e))
            }
        }
    }   else{
        pending.lock().unwrap().entry(message.target_hash.clone()).or_default().push(message.clone());
        Err(format!("Target hash {} is offline or not existing(not in routing table)", message.target_hash))
    }
}

fn save_user_list(users: &Vec<String>) {
    let entry = Entry::new("Comunicator-server", "user_list").unwrap();
    let json = serde_json::to_string(users).unwrap();
    entry.set_secret(json.as_bytes()).unwrap();
}

fn main() {
    let config = HashMap::new();
    keyring::use_sqlite_store(&config).unwrap();

    let user_list_key = "Comunicator-server-userlist";
    let pending: PendingMessages = Arc::new(Mutex::new(HashMap::new()));
    let routing_table: RoutingTable = Arc::new(Mutex::new(HashMap::new()));
    let http_client = reqwest::blocking::Client::new();
    let user_list: Arc<Mutex<Vec<String>>> = {
        let entry = Entry::new("Comunicator-server", "user_list").unwrap();
        match entry.get_secret(){
            Ok(bytes) => {
                let users: Vec<String> = serde_json::from_slice(&bytes).unwrap_or_default();
                println!("[SERVER] Loaded {} user from persistent storage", users.len());
                Arc::new(Mutex::new(users))
            }
            Err(_) => Arc::new(Mutex::new(Vec::new())),
        }
    };

    println!("[COMUNICATOR-SERVER] Running a server node on threads...");

    let server = Server::http("0.0.0.0:4001").unwrap();
    println!("[COMUNICATOR-SERVER] Server is ready on port 4001...");
    println!("--------------------------------------------------");

    for mut request in server.incoming_requests() {
        let routing_table = Arc::clone(&routing_table);
        let http_client = http_client.clone();
        let user_list = Arc::clone(&user_list);
        let pending = Arc::clone(&pending);
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

                    user_list.lock().unwrap().push(nick.clone());
                    save_user_list(&user_list.lock().unwrap());

                    let response = Response::from_string(new_hash);
                    let _ = request.respond(response);
                    return;
                } 

                if let Ok(change_req) = serde_json::from_str::<ChangeNickRequest>(&body) {
                    if change_req.action == "change_nick" {
                        let reverse_entry = Entry::new("Comunicator-server", &change_req.hash).unwrap();
                        let old_nick = reverse_entry.get_secret().ok()
                            .and_then(|b| String::from_utf8(b).ok())
                            .unwrap_or_default();
                        let check = Entry::new("Comunicator-server", &format!("{}_hash", change_req.new_nick)).unwrap();
                        if check.get_secret().is_ok() {
                            let response = Response::from_string("[ERROR] Nick already taken.");
                            let _ = request.respond(response);
                            return;
                        }

                        let hash_val = Entry::new("Comunicator-server", &format!("{}_hash", old_nick)).unwrap().get_secret().unwrap();
                        Entry::new("Comunicator-server", &format!("{}_hash", change_req.new_nick)).unwrap().set_secret(&hash_val).unwrap();
                        Entry::new("Comunicator-server", &format!("{}_nick", change_req.new_nick)).unwrap().set_secret(change_req.new_nick.as_bytes()).unwrap();

                        let _ = Entry::new("Comunicator-server", &format!("{}_hash", old_nick)).unwrap().set_secret(b"");
                        let _ = Entry::new("Comunicator-server", &format!("{}_nick", old_nick)).unwrap().set_secret(b"");

                        reverse_entry.set_secret(change_req.new_nick.as_bytes()).unwrap();

                        let mut list = user_list.lock().unwrap();
                        if let Some(pos) = list.iter().position(|n| n == &old_nick) {
                            list[pos] = change_req.new_nick.clone();
                        }
                        save_user_list(&list);

                        let response = Response::from_string("[OK] Nick Changed.");
                        let _ = request.respond(response);
                        return;
                    }
                }

                if let Ok(change_req) = serde_json::from_str::<ChangeHashRequest>(&body) {
                    if change_req.action == "change_hash" {
                        let reverse_entry = Entry::new("Comunicator-server", &change_req.old_hash).unwrap();
                        let nick = reverse_entry.get_secret().ok()
                            .and_then(|b| String::from_utf8(b).ok())
                            .unwrap_or_default();
                        if nick.is_empty() {
                            let response = Response::from_string("[ERROR] Invalid hash.");
                            let _ = request.respond(response);
                            return;
                        }
                        let new_hash = Uuid::new_v4().to_string();
                        Entry::new("Comunicator-server", &format!("{}_hash", nick)).unwrap().set_secret(new_hash.as_bytes()).unwrap();
                        let _ = Entry::new("Comunicator-server", &change_req.old_hash).unwrap().set_secret(b"");
                        Entry::new("Comunicator-server", &new_hash).unwrap().set_secret(nick.as_bytes()).unwrap();

                        let response = Response::from_string(new_hash);
                        let _ = request.respond(response);
                        return;
                    }
                }

                if let Ok(list_req) = serde_json::from_str::<ListRequest>(&body) {
                    if list_req.action == "list_users" {
                        let nicks = user_list.lock().unwrap().clone();
                        let users: Vec<UserInfo> = nicks.iter().filter_map(|nick|{
                            let entry = Entry::new("Comunicator-server", &format!("{}_hash", nick)).unwrap();
                            entry.get_secret().ok().and_then(|bytes| {
                                String::from_utf8(bytes).ok().map(|hash| UserInfo {
                                    nick: nick.clone(),
                                    hash,
                                })
                            })
                        }).collect();
                        let response_body = serde_json::to_string(&ListResponse { users }).unwrap();
                        let response = Response::from_string(response_body);
                        let _ = request.respond(response);
                        return;
                    }
                }

                if let Ok(sync_req) = serde_json::from_str::<serde_json::Value>(&body) {
                    if sync_req.get("action").and_then(|v| v.as_str()) == Some("sync") {
                        if let Some(hash) = sync_req.get("hash").and_then(|v| v.as_str()){
                            let mut pending_map = pending.lock().unwrap();
                            let msgs = pending_map.remove(hash).unwrap_or_default();
                            let response_body = serde_json::to_string(&serde_json::json!({"messages" : msgs})).unwrap();
                            let response = Response::from_string(response_body);
                            let _ = request.respond(response);
                            return;
                        }
                    } 
                }
                
                if let Ok(packet_msg) = serde_json::from_str::<MessagePacket>(&body) {
                    println!("\n[JSON DATA PACKET]");
                    println!("  From (Hash):   {}", packet_msg.sender_hash);
                    println!("  Nick:          {}", packet_msg.sender_nick);
                    println!("  To (Hash):     {}", packet_msg.target_hash);
                    println!("  Message:       {}", packet_msg.content);
                    println!("--------------------------------------------------");

                    let sender_ip = request.remote_addr().unwrap().ip().to_string();

                    if body.contains("target_hash") {
                        match forward_message(&body, sender_ip, &routing_table, &http_client, &pending) {
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