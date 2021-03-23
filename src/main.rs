use redis::{FromRedisValue, RedisResult};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, from_str, json};
use std::{collections::HashSet, convert::Infallible};
use warp::{reply, Filter};

const REDIS_HOST: &'static str = "redis://localhost/";

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
struct Test {
    event: String,
    user: User,
}

#[derive(Serialize, Deserialize, Debug)]
struct ConversationStarted {
    event: String,
    timestamp: u64,
    message_token: u64,
    r#type: String,
    context: String,
    user: User,
    subscribed: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct Subscribed {
    event: String,
    timestamp: u64,
    user: User,
    message_token: u64,
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
    conversation_started()
        .or(subscribed())
        .or(tt())
        .or(unrelated_event())
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

pub fn subscribed() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("viber" / "events")
        .and(warp::body::json())
        .and_then(subscribed_handler)
}

async fn subscribed_handler(event: Subscribed) -> Result<impl warp::Reply, Infallible> {
    add_user(&format!("id:{}", event.user.id), &event.user)
        .await
        .expect("Failed to add user to db.");
    Ok(reply())
}

pub fn tt() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("viber" / "events")
        .and(warp::body::json())
        .and_then(tt_handler)
}

async fn tt_handler(event: Test) -> Result<impl warp::Reply, Infallible> {
    Ok(reply())
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
            redis::Value::Data(bytes) => Ok(match from_slice::<User>(bytes) {
                Ok(user) => RedisUser::Some(user),
                _ => RedisUser::None,
            }),
            redis::Value::Nil => Ok(RedisUser::None),
            _ => Ok(RedisUser::None),
        }
    }
}

async fn add_user(key: &str, user: &User) -> RedisResult<()> {
    let client = redis::Client::open(REDIS_HOST).unwrap();
    let mut con = client.get_async_connection().await?;
    let serialized_user = serde_json::to_string(&user).expect("User to json failed");
    redis::cmd("JSON.SET")
        .arg(&[key, ".", &serialized_user])
        .query_async(&mut con)
        .await?;

    redis::cmd("SADD")
        .arg(&["subscribed", key])
        .query_async(&mut con)
        .await?;

    Ok(())
}

async fn get_user(key: &str) -> Option<User> {
    let client = redis::Client::open(REDIS_HOST).unwrap();
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

async fn list_subscribed() -> Option<Vec<User>> {
    let client = redis::Client::open(REDIS_HOST).unwrap();
    let mut con = client.get_async_connection().await.unwrap();
    let ids: HashSet<String> = redis::cmd("SMEMBERS")
        .arg("subscribed")
        .query_async(&mut con)
        .await
        .ok()?;
    let mut users = Vec::new();
    for id in ids {
        if let Some(user) = get_user(&id).await {
            users.push(user)
        }
    }
    Some(users)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use warp::http::StatusCode;
    use warp::test::request;

    #[tokio::test]
    async fn test_event_conversation_started() {
        let api = events();

        let resp = request()
            .method("POST")
            .path("/viber/events")
            .json(&json!({
                "event":"conversation_started",
                "timestamp":1457764197627_u64,
                "message_token":4912661846655238145_u64,
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
        assert_eq!(user.name, "John McClane");

        let new_user: User = from_str(
            &json!({
                "id":"01234567890A=",
                "name":"John McClane",
                "avatar":"http://avatar.example.com",
                "country":"UK",
                "language":"en",
                "api_version":1
            })
            .to_string(),
        )
        .unwrap();
        let subscribed = list_subscribed().await.unwrap();
        assert!(subscribed.contains(&new_user));
    }

    #[tokio::test]
    async fn test_event_subscribed() {
        let api = events();

        let resp = request()
            .method("POST")
            .path("/viber/events")
            .json(&json!({
               "event":"subscribed",
               "timestamp":1457764197627_u64,
               "user":{
                   "id":"id-subscribed",
                   "name":"John McClane Subscribed",
                   "avatar":"http://avatar.example.com",
                   "country":"UK",
                   "language":"en",
                   "api_version":1
               },
               "message_token":4912661846655238145_u64
            }))
            .reply(&api)
            .await;

        let new_user: User = from_str(
            &json!({
                "id":"subscribed",
                "name":"John McClane Subscribed",
                "avatar":"http://avatar.example.com",
                "country":"UK",
                "language":"en",
                "api_version":1
            })
            .to_string(),
        )
        .unwrap();
        let subscribed = list_subscribed().await.unwrap();
        assert!(subscribed.contains(&new_user));
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_test() {
        let api = events();

        let resp = request()
            .method("POST")
            .path("/viber/events")
            .json(&json!({
            "event":"test",
            "user":{
                "id":"01234567890A=",
                "name":"John McClane",
                "avatar":"http://avatar.example.com",
                "country":"UK",
                "language":"en",
                "api_version":1
            },
            }))
            .reply(&api)
            .await;

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
