use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Player {
    pub id: i64,
    pub username: String,
    pub score: i64
}

#[derive(Serialize)]
pub struct PlayersResponse {
    pub players: Vec<Player>,
}