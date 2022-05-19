use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub bind_host: String,
    pub bind_port: u16,
    pub dot_executable: String,
    pub log_level: String,
    pub max_len: i32,
    pub max_count_sametime: usize,
    pub render_timeout: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_host: "127.0.0.1".into(),
            bind_port: 90,
            dot_executable: "dot".into(),
            log_level: "info".into(),
            max_len: 50,
            max_count_sametime: 2,
            render_timeout: 5000,
        }
    }
}
