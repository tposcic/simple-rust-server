use std::collections::HashMap;

// pub fn parse_query_string(query: &str) -> HashMap<String, String> {
//     query.split('&')
//     .filter_map(|pair| {
//         let mut iter = pair.split('=');
//         if let (Some(key), Some(value)) = (iter.next(), iter.next()) {
//             Some((key.to_string(), value.to_string()))
//         } else {
//             None
//         }
//     })
//     .collect()
// }

pub fn parse_query_parameters(path_and_query: &str) -> i64 {
    if let Some(pos) = path_and_query.find('?') {
        let query = &path_and_query[pos + 1..];
        let query_params: HashMap<_, _> = query.split('&')
            .filter_map(|pair| {
                let mut iter = pair.split('=');
                if let (Some(key), Some(value)) = (iter.next(), iter.next()) {
                    Some((key, value))
                } else {
                    None
                }
            })
            .collect();
    
        match query_params.get("limit") {
            Some(limit) => match limit.parse::<i64>() {
                Ok(value) => value,
                Err(_) => {
                    eprintln!("Invalid limit value, using default of 15");
                    15
                }
            },
            None => 15,
        }
    } else {
        15
    }
}