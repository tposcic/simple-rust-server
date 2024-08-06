fn read_and_check_headers(mut stream: &TcpStream, conn: &mut PooledConn) -> Result<()> {
    let mut headers = HashMap::new();
    let buffer_reader = BufReader::new(&mut stream);

    for line in buffer_reader.lines() {
        let line = line.unwrap();
        if line.is_empty() {
            break;
        }
        let parts: Vec<&str> = line.splitn(2, ": ").collect();
        if parts.len() == 2 {
            headers.insert(parts[0].to_string(), parts[1].to_string());
        }
    }

    if let Some(auth_header) = headers.get("Authorization") {
        let token = auth_header.trim_start_matches("Bearer ");

        match check_token(conn, token) {
            Ok(true) => {},  // Token is valid, do nothing
            Ok(false) | Err(_) => send_unauthorized_response(stream),  // Token is invalid or an error occurred
        }
    } else {
        send_unauthorized_response(stream);
    }

    Ok(())
}

fn check_token(conn: &mut PooledConn, token: &str) -> Result<bool> {
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