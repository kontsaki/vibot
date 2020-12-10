use reqwest;
use serde_json::json;
use std::collections::HashMap;
use warp::{reply, Filter};

#[tokio::main]
async fn main() {}

fn webhook(webhook_url: &str, api_key: String, site_url: String) -> reqwest::RequestBuilder {
    let client = reqwest::Client::new();
    client
        .post(webhook_url)
        .header("X-Viber-Auth-Token", api_key)
        .json(&json!({
            "url": site_url,
            "send_name": true,
        }))
}

#[cfg(test)]
mod tests {
    use super::webhook;
    use serde_json::json;

    #[test]
    fn test_webhook_request_data() {
        let request = webhook(
            "https://webhook-url/",
            "api-key".to_string(),
            "https://my-site/".to_string(),
        )
        .build()
        .unwrap();
        assert_eq!(request.method(), "POST");
        assert_eq!(request.url().as_str(), "https://webhook-url/");
        assert_eq!(
            request.headers().get("X-Viber-Auth-Token").unwrap(),
            "api-key"
        );
        assert_eq!(
            request.body().unwrap().as_bytes().unwrap(),
            json!({
                 "url": "https://my-site/",
                 "send_name": true,
            })
            .to_string()
            .into_bytes()
        );
    }
}
