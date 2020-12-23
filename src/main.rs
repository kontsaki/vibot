use redis::{FromRedisValue, RedisResult};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, json};
use std::convert::Infallible;
use warp::{reply, Filter};

#[derive(Default, Serialize, Deserialize, Debug, Eq, PartialEq)]
struct User {
    id: String,
    name: String,
    avatar: Option<String>,
    country: Option<String>,
    language: Option<String>,
    api_version: Option<i8>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ConversationStarted {
    event: String,
    timestamp: i64,
    message_token: i64,
    r#type: String,
    context: String,
    user: User,
    subscribed: bool,
}

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

pub fn events() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    conversation_started().or(unrelated_event())
}

pub fn conversation_started(
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("viber" / "events")
        .and(warp::body::json())
        .and_then(conversation_started_handler)
}

async fn conversation_started_handler(
    event: ConversationStarted,
) -> Result<impl warp::Reply, Infallible> {
    add_user(&format!("id:{}", event.user.id), &event.user)
        .await
        .expect("Failed to add user to db.");
    Ok(reply::json(&json!({
        "type": "picture",
        "text": "Welcome",
        "media": "https://a-picture",
    })))
}

pub fn unrelated_event() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
{
    warp::path!("viber" / "events")
        .and(warp::any())
        .map(|| warp::reply())
}

#[derive(Debug, Eq, PartialEq)]
enum RedisUser {
    Some(User),
    None,
}

impl FromRedisValue for RedisUser {
    fn from_redis_value(v: &redis::Value) -> RedisResult<Self> {
        match v {
            redis::Value::Data(bytes) => Ok(RedisUser::Some(from_slice::<User>(bytes).unwrap())),
            redis::Value::Nil => Ok(RedisUser::None),
            _ => Ok(RedisUser::None),
        }
    }
}

async fn add_user(key: &str, user: &User) -> RedisResult<()> {
    let client = redis::Client::open("redis://localhost/").unwrap();
    let mut con = client.get_async_connection().await?;
    redis::cmd("JSON.SET")
        .arg(&[
            key,
            ".",
            &serde_json::to_string(&user).expect("User to json failed"),
        ])
        .query_async(&mut con)
        .await
}

async fn get_user(key: &str) -> Option<User> {
    let client = redis::Client::open("redis://localhost/").unwrap();
    let mut con = client.get_async_connection().await.unwrap();
    match redis::cmd("JSON.GET")
        .arg(&[key])
        .query_async(&mut con)
        .await
        .ok()?
    {
        RedisUser::Some(user) => Some(user),
        RedisUser::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use warp::http::StatusCode;
    use warp::test::request;

    #[tokio::test]
    async fn test_get_user() -> redis::RedisResult<()> {
        let user = User {
            id: "user-id".to_string(),
            name: "user-name".to_string(),
            ..Default::default()
        };

        let client = redis::Client::open("redis://localhost/").unwrap();
        let mut con = client.get_async_connection().await.unwrap();

        redis::cmd("JSON.SET")
            .arg(&["user:id", ".", &serde_json::to_string(&user).unwrap()])
            .query_async(&mut con)
            .await?;

        assert_eq!(get_user("user:id").await, Some(user));
        Ok(())
    }

    #[tokio::test]
    async fn test_add_user() -> redis::RedisResult<()> {
        let user = User {
            id: "user-id".to_string(),
            name: "user-name".to_string(),
            ..Default::default()
        };

        add_user("user:id", &user).await?;

        let client = redis::Client::open("redis://localhost/").unwrap();
        let mut con = client.get_async_connection().await.unwrap();
        let result: RedisUser = redis::cmd("JSON.GET")
            .arg(&["user:id"])
            .query_async(&mut con)
            .await?;

        assert_eq!(result, RedisUser::Some(user));
        Ok(())
    }

    // #[tokio::test]
    // async fn test_list_subscribed() -> redis::RedisResult<()> {
    //     let client = redis::Client::open("redis://localhost/").unwrap();
    //     let mut con = client.get_async_connection().await.unwrap();

    //     let result: RedisUser = redis::cmd("JSON.GET")
    //         .arg(&["user:id"])
    //         .query_async(&mut con)
    //         .await?;

    //     assert_eq!(result, RedisUser::Some(user));
    //     Ok(())
    // }

    #[tokio::test]
    async fn test_event_conversation_started() {
        let api = events();

        let resp = request()
            .method("POST")
            .path("/viber/events")
            .json(&json!({
                "event":"conversation_started",
                "timestamp":1457764197627 as i64,
                "message_token":4912661846655238145 as i64,
                "type":"open",
                "context":"context information",
                "user":{
                    "id":"01234567890A=",
                    "name":"John McClane",
                    "avatar":"http://avatar.example.com",
                    "country":"UK",
                    "language":"en",
                    "api_version":1
                },
                "subscribed":false}))
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

        let user = get_user("id:01234567890A=")
            .await
            .expect("User doesn't exist");
        assert_eq!(user.name, "John McClane")
    }

    #[tokio::test]
    async fn test_event_subscribed() {
        let api = events();

        let resp = request()
            .method("POST")
            .path("/viber/events")
            .json(&json!({
               "event":"subscribed",
               "timestamp":1457764197627 as i64,
               "user":{
                   "id":"01234567890A=",
                   "name":"John McClane",
                   "avatar":"http://avatar.example.com",
                   "country":"UK",
                   "language":"en",
                   "api_version":1
               },
               "message_token":4912661846655238145 as i64
            }))
            .reply(&api)
            .await;

        let subscribed = list_subscribed();
        assert!(subscribed.contains(&"01234567890A="));

        assert_eq!(resp.status(), StatusCode::OK);
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
