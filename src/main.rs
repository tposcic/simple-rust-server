mod db;
mod handlers;

use db::connection::
{
    send_too_many_requests_response, 
    handle_connection
};

use std::
{
    net::TcpListener,
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant}
};

use dotenv::dotenv;
use std::env;

const MAX_CONNECTIONS: usize = 15000; // Maximum number of connections allowed per client in the time defined in TIME_INTERVAL
const TIME_INTERVAL: Duration = Duration::from_secs(60); // Time interval in seconds

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
    dotenv().expect("Please setup the .env file");

    let database_url = env::var("DB").expect("DB (database url) must be set");
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
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