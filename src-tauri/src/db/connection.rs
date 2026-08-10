use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type DbState = Arc<Mutex<Connection>>;

pub fn get_app_dir() -> PathBuf {
    let mut dir = dirs_next().unwrap_or_else(|| PathBuf::from("."));
    dir.push("4d_bordro_data");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn dirs_next() -> Option<PathBuf> {
    #[allow(deprecated)]
    std::env::home_dir().map(|h| h.join(".4d_bordro"))
}

pub fn create_connection(
    db_path: Option<PathBuf>,
) -> Result<Connection, Box<dyn std::error::Error>> {
    let path = match db_path {
        Some(p) => p,
        None => {
            let app_dir = get_app_dir();
            app_dir.join("bordro.sqlite")
        }
    };

    let mut conn = Connection::open(path)?;
    super::migrations::initialize_db(&mut conn)?;
    Ok(conn)
}

pub fn create_in_memory_connection() -> Result<Connection, Box<dyn std::error::Error>> {
    let mut conn = Connection::open_in_memory()?;
    super::migrations::initialize_db(&mut conn)?;
    Ok(conn)
}
