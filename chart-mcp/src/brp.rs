use anyhow::Result;
use serde_json::{json, Value};

pub struct BrpClient {
    endpoint: String,
    client: reqwest::blocking::Client,
}

impl BrpClient {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            endpoint: format!("http://{}:{}", host, port),
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn call(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let response = self.client
            .post(&self.endpoint)
            .json(&request)
            .send()?
            .json::<Value>()?;

        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else {
            Err(anyhow::anyhow!("BRP error: {:?}", response.get("error")))
        }
    }
}
