use std::
{
    io::{prelude::*, BufReader},//buffer reader
    net::{TcpListener, TcpStream},
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant}
};

use mysql::prelude::*;
use mysql::*;
use serde::Serialize;
use dotenv::dotenv;
use std::env;

const MAX_CONNECTIONS: usize = 60; // Maximum number of connections allowed per client in the time defined in TIME_INTERVAL
const TIME_INTERVAL: Duration = Duration::from_secs(60); // Time interval in seconds

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Player {
    id: i64,
    username: String,
    score: i64
}

#[derive(Serialize)]
struct PlayersResponse {
    players: Vec<Player>,
}

struct Throttle {
    count: usize,
    last_reset: Instant,
}

impl Throttle {
    fn new() -> Self {
        Throttle {
            count: 0,
            last_reset: Instant::now(),
        }
    }

    fn check_and_increment(&mut self) -> bool {
        if self.last_reset.elapsed() > TIME_INTERVAL {
            self.count = 0;
            self.last_reset = Instant::now();
        }

        if self.count < MAX_CONNECTIONS {
            self.count += 1;
            true
        } else {
            false
        }
    }
}

fn main() 
{
    dotenv().ok();
    let database_url = env::var("DB").expect("DB (database url) must be set");

    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let throttle_map = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming()
    {
        let stream = stream.unwrap();
        let throttle_map = Arc::clone(&throttle_map);
        
        let database_url = database_url.clone(); // Clone the database_url for each thread

        std::thread::spawn(move || {
            let peer_addr = stream.peer_addr().unwrap().ip();
            let mut throttle_map = throttle_map.lock().unwrap();
            let throttle = throttle_map.entry(peer_addr).or_insert_with(Throttle::new);

            if throttle.check_and_increment() {
                handle_connection(stream, &database_url);
            } else {
                send_too_many_requests_response(&stream);
            }
        });
    }
}

fn handle_connection(mut stream: TcpStream, database_url: &str)
{
    let mut buffer_reader = BufReader::new(&mut stream);
    let request_line = buffer_reader.by_ref().lines().next().unwrap().unwrap();

    let pool = Pool::new(database_url).unwrap();
    let mut conn = pool.get_conn().unwrap();

    if request_line == "GET /players/top HTTP/1.1" {
        match fetch_top_players(&mut conn) {
            Ok(players) => send_players_response(&mut stream, "HTTP/1.1 200 OK", players),
            Err(e) => eprintln!("Error fetching players: {}", e),
        }
    }
    else if request_line == "POST /player HTTP/1.1"
    {
        let mut headers = String::new();
        let mut content_length = 0;
    
        for line in buffer_reader.by_ref().lines() {
            let line = line.unwrap();
            if line.is_empty() {
                break;
            }
            headers.push_str(&line);
            headers.push_str("\r\n");

            if line.starts_with("Content-Length:") {
                let parts: Vec<&str> = line.split(':').collect();
                content_length = parts[1].trim().parse::<usize>().unwrap();
            }
        }
    
        let mut body = vec![0; content_length];
        buffer_reader.read_exact(&mut body).unwrap();
        let body_str = String::from_utf8(body).unwrap();
    
        // Assuming the body is in JSON format
        let player_data: serde_json::Value = serde_json::from_str(&body_str).unwrap();
        let username = player_data["username"].as_str().unwrap();
        let score = player_data["score"].as_i64().unwrap() as i32;
    
        match insert_player(&mut conn, username, score.into()) {
            Ok(player) => send_players_response(&mut stream, "HTTP/1.1 200 OK", vec![player]),
            Err(e) => eprintln!("Error inserting player: {}", e),
        }
    }
    else
    {
        send_not_found_response(&mut stream, "HTTP/1.1 404 NOT FOUND");
    };
}

fn fetch_top_players(conn: &mut PooledConn) -> std::result::Result<Vec<Player>, mysql::Error> 
{
    conn.query_map(
        "SELECT id, `username`, score FROM top_players",
        |(id, username, score)| {
            Player { id, username, score }
        }
    )
}

fn insert_player(conn: &mut PooledConn, username: &str, score: i64) -> std::result::Result<Player, mysql::Error> {
    conn.exec_drop(
        "DELETE FROM top_players WHERE username = :username",
        params! {
            "username" => username,
            "score" => score,
        },
    )?;

    conn.exec_drop(
        "INSERT INTO top_players (username, score) VALUES (:username, :score)",
        params! {
            "username" => username,
            "score" => score,
        },
    )?;

    let player_id = conn.last_insert_id();
    let player = Player {
        id: player_id as i64,
        username: username.to_string(),
        score,
    };

    Ok(player)
}

fn send_players_response(stream: &mut TcpStream, status_line: &str, players: Vec<Player>) 
{
    let response_body = PlayersResponse { players };
    let players_json = serde_json::to_string(&response_body).unwrap();
    let length = players_json.len();
    let response = format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\n\r\n{players_json}"
    );

    stream.write_all(response.as_bytes()).unwrap();
}

fn send_not_found_response(stream: &mut TcpStream, status_line: &str)
{
    let response_body = PlayersResponse { players: vec![] };
    let players_json = serde_json::to_string(&response_body).unwrap();
    let length = players_json.len();
    let response = format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\n\r\n{players_json}"
    );

    stream.write_all(response.as_bytes()).unwrap();
}

fn send_too_many_requests_response(mut stream: &TcpStream) {
    let response = "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n";
    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

// fn send_bad_request_response(stream: &mut TcpStream, status: &str) {
//     let response = format!(
//         "{}\r\nContent-Length: 0\r\n\r\n",
//         status
//     );
//     stream.write_all(response.as_bytes()).unwrap();
//     stream.flush().unwrap();
// }