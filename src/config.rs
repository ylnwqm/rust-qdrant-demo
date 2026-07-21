use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub qdrant_url: String,
    pub qdrant_api_key: Option<String>,
    pub server_host: String,
    pub server_port: u16,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        Ok(Self {
            qdrant_url: env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".into()),
            qdrant_api_key: env::var("QDRANT_API_KEY").ok(),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port: env::var("SERVER_PORT").unwrap_or_else(|_| "3000".into()).parse()?,
        })
    }
}
