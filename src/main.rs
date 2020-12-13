use redis::{FromRedisValue, RedisError, RedisResult};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, json};
use std::collections::HashMap;
use warp::{reply, Filter};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct User {
    id: String,
    name: String,
}

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

impl FromRedisValue for User {
    fn from_redis_value(v: &redis::Value) -> RedisResult<Self> {
        match v {
            redis::Value::Data(bytes) => Ok(from_slice::<User>(bytes).unwrap()),
            _ => panic!("Not a User value"),
        }
    }
}

async fn add_user(key: &str, user: &User) -> RedisResult<()> {
    let client = redis::Client::open("redis://localhost/").unwrap();
    let mut con = client.get_async_connection().await.unwrap();
    redis::cmd("JSON.SET")
        .arg(&[key, ".", &serde_json::to_string(&user).unwrap()])
        .query_async(&mut con)
        .await
}

async fn get_user(key: &str) -> RedisResult<User> {
    let client = redis::Client::open("redis://localhost/").unwrap();
    let mut con = client.get_async_connection().await.unwrap();
    redis::cmd("JSON.GET")
        .arg(&[key])
        .query_async(&mut con)
        .await
}

#[cfg(test)]
mod tests {
    use super::{add_user, events, get_user, webhook, User};
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

    #[tokio::test]
    async fn test_get_user() -> redis::RedisResult<()> {
        let user = User {
            id: "user-id".to_string(),
            name: "user-name".to_string(),
        };

        let client = redis::Client::open("redis://localhost/").unwrap();
        let mut con = client.get_async_connection().await.unwrap();

        redis::cmd("JSON.SET")
            .arg(&["user:id", ".", &serde_json::to_string(&user).unwrap()])
            .query_async(&mut con)
            .await?;

        assert_eq!(get_user("user:id").await.unwrap(), user);
        Ok(())
    }

    #[tokio::test]
    async fn test_add_user() -> redis::RedisResult<()> {
        let user = User {
            id: "user-id".to_string(),
            name: "user-name".to_string(),
        };

        add_user("user:id", &user).await?;

        let client = redis::Client::open("redis://localhost/").unwrap();
        let mut con = client.get_async_connection().await.unwrap();
        let result: User = redis::cmd("JSON.GET")
            .arg(&["user:id"])
            .query_async(&mut con)
            .await?;

        assert_eq!(result, user);
        Ok(())
    }
}
