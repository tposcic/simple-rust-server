//i want to use mod handlers that is in the parent folder
use crate::handlers::players::{Player, PlayersResponse};
use super::parser::parse_query_parameters;
use mysql::*;
use mysql::prelude::*;

use std::
{
    collections::HashMap, io::{Write, prelude::*, BufReader}, net::TcpStream
};

pub fn establish_connection(database_url: &str) -> mysql::PooledConn {
    let pool = Pool::new(database_url).unwrap();
    pool.get_conn().unwrap()
}

pub fn check_token(conn: &mut PooledConn, token: &str) -> Result<bool> {
    match conn.exec_first::<Option<String>, _, _>(
        "SELECT token FROM tokens WHERE token = :token",
        params! {
            "token" => token,
        },
    ) {
        Ok(Some(_)) => Ok(true),  // Token found
        Ok(None) => Ok(false),    // Token not found
        Err(e) => Err(e),         // Error occurred
    }
}

// pub fn get_limit_from_query(query: &str) -> i64 {
//     let query_params = parse_query_string(query);
//     match query_params.get("limit") {
//         Some(limit) => match limit.parse::<i64>() {
//             Ok(value) => value,
//             Err(_) => {
//                 eprintln!("Invalid limit value, using default of 15");
//                 15
//             }
//         },
//         None => 15,
//     }
// }

pub fn fetch_top_players_test(conn: &mut PooledConn, limit: &i64) -> std::result::Result<Vec<Player>, mysql::Error> 
{
    conn.exec_map(
        "SELECT id, `username`, score FROM top_players LIMIT :limit",
        params! {
            "limit" => limit,
        },
        |(id, username, score)| {
            Player { id, username, score }
        }
    )
}


pub fn fetch_top_players(conn: &mut PooledConn, token: &str, limit: &i64) -> std::result::Result<Vec<Player>, mysql::Error> 
{
    match check_token(conn, token)? {
        true => {
            conn.exec_map(
                "SELECT id, `username`, score FROM top_players LIMIT :limit",
                params! {
                    "limit" => limit,
                },
                |(id, username, score)| {
                    Player { id, username, score }
                }
            )
        },
        false => {
            Err(mysql::Error::MySqlError(mysql::MySqlError {
                code: 400,
                message: "Bad Request".to_string(),
                state: "HY000".to_string(), // Adding the state field
            }))        
        }
    }
}

pub fn insert_player(conn: &mut PooledConn, username: &str, score: i64, token: &str) -> std::result::Result<Player, mysql::Error> {
    match check_token(conn, token)? {
        true => {
            conn.exec_drop(
                "DELETE FROM top_players WHERE username = :username",
                params! {
                    "username" => username,
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
        },
        false => {
            Err(mysql::Error::MySqlError(mysql::MySqlError {
                code: 400,
                message: "Bad Request".to_string(),
                state: "HY000".to_string(), // Adding the state field
            }))        
        }
    }
}

pub fn send_players_response(stream: &mut TcpStream, status_line: &str,players: Vec<Player>) 
{
    let response_body = PlayersResponse { players };
    let players_json = serde_json::to_string(&response_body).unwrap();
    let length = players_json.len();
    let response = format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\n\r\n{players_json}"
    );

    stream.write_all(response.as_bytes()).unwrap();
}

pub fn send_not_found_response(stream: &mut TcpStream, status_line: &str)
{
    let response_body = PlayersResponse { players: vec![] };
    let players_json = serde_json::to_string(&response_body).unwrap();
    let length = players_json.len();
    let response = format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\n\r\n{players_json}"
    );

    stream.write_all(response.as_bytes()).unwrap();
}

pub fn send_too_many_requests_response(mut stream: &TcpStream) {
    let response = "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n";
    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

pub fn handle_connection(mut stream: TcpStream, database_url: &str)
{
    let mut buffer_reader = BufReader::new(&mut stream);

    let request_line = match buffer_reader.by_ref().lines().next() {
        Some(Ok(line)) => line,
        Some(Err(e)) => {
            eprintln!("Error reading request line: {}", e);
            return;
        }
        None => {
            eprintln!("No request line found");
            return;
        }
    };

    let mut headers = HashMap::new();
    let mut token = "";
    let mut content_length = 0;

    for line in buffer_reader.by_ref().lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                eprintln!("Error reading header line: {}", e);
                return;
            }
        };
        
        if line.is_empty() {
            break;
        }
        
        let parts: Vec<&str> = line.splitn(2, ": ").collect();
        
        if line.starts_with("Content-Length:") {
            let parts: Vec<&str> = line.split(':').collect();
            content_length = parts[1].trim().parse::<usize>().unwrap();
        }
        
        if parts.len() == 2 {
            headers.insert(parts[0].to_string(), parts[1].to_string());
        }
    }

    if let Some(auth_header) = headers.get("Authorization") {
        token = auth_header.trim_start_matches("Bearer ");
    }

    let limit = match request_line.split_whitespace().nth(1) {
        Some(path_and_query) => {
            parse_query_parameters(path_and_query)
        }
        None => 15,
    };

    let mut conn: PooledConn = establish_connection(database_url);

    if request_line.starts_with("GET /players/top") {
        match fetch_top_players(&mut conn,token,&limit) {
            Ok(players) => send_players_response(&mut stream, "HTTP/1.1 200 OK", players),
            Err(e) => eprintln!("Error fetching players: {}", e),
        }
    }
    else if request_line.starts_with("GET /test/players") {
        match fetch_top_players_test(&mut conn,&limit) {
            Ok(players) => send_players_response(&mut stream, "HTTP/1.1 200 OK", players),
            Err(e) => eprintln!("Error fetching players: {}", e),
        }
    }
    else if request_line == "POST /player HTTP/1.1"
    {
        let mut body = vec![0; content_length];
        buffer_reader.read_exact(&mut body).unwrap();
        let body_str = String::from_utf8(body).unwrap();
    
        // Assuming the body is in JSON format
        let player_data: serde_json::Value = serde_json::from_str(&body_str).unwrap();
        let username = player_data["username"].as_str().unwrap();
        let score: i32 = player_data["score"].as_i64().unwrap() as i32;
    
        match insert_player(&mut conn, username, score.into(),token) {
            Ok(player) => send_players_response(&mut stream, "HTTP/1.1 200 OK", vec![player]),
            Err(e) => eprintln!("Error inserting player: {}", e),
        }
    }
    else
    {
        send_not_found_response(&mut stream, "HTTP/1.1 404 NOT FOUND");
    };
}
