use reqwest;
use serde_json::json;
use std::collections::HashMap;
use warp::{reply, Filter};

#[tokio::main]
async fn main() {}

pub fn events() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("viber" / "events").and(warp::body::json()).map(
        |event_info: HashMap<String, String>| {
            if let Some(event) = event_info.get("event") {
                match event.as_str() {
                    "conversation_started" => conversaton_started(),
                    _ => reply::json(&json!({})),
                }
            } else {
                reply::json(&json!({}))
            }
        },
    )
}

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

fn conversaton_started() -> warp::reply::Json {
    reply::json(&json!({
        "type": "picture",
        "text": "Welcome",
        "media": "https://a-picture",
    }))
}

#[cfg(test)]
mod tests {
    use super::{events, webhook};
    use serde_json::json;
    use warp::http::StatusCode;
    use warp::test::request;

    #[tokio::test]
    async fn test_events_conversation_started() {
        let api = events();

        let resp = request()
            .method("POST")
            .path("/viber/events")
            .json(&json!({
                "event": "conversation_started"
            }))
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.into_body(),
            json!({
                "type": "picture",
                "text": "Welcome",
                "media": "https://a-picture"
            })
            .to_string()
        );
    }

    #[tokio::test]
    async fn test_events_unrelated() {
        let api = events();

        let resp = request()
            .method("POST")
            .path("/viber/events")
            .json(&json!({
                "event": "unrelated"
            }))
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.into_body(), json!({}).to_string());
    }

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
