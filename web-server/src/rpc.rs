use crate::api;
use serde_json::{json, Value};
fn arg<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(v.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|_| format!("Invalid argument: {key}"))
}
pub async fn dispatch(state: &api::Api, cmd: &str, args: Value) -> Result<Value, String> {
    match cmd {
        "net_diag" => api::net_diag()
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "line_state" => api::line_state()
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "line_set" => api::line_set(state, arg(&args, "url")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "line_mirrors" => api::line_mirrors()
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "line_probe" => api::line_probe(arg(&args, "urls")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "check_update" => api::check_update()
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "app_boot" => api::app_boot(state)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_login" => api::api_login(state, arg(&args, "username")?, arg(&args, "password")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_register" => api::api_register(
            state,
            arg(&args, "username")?,
            arg(&args, "password")?,
            arg(&args, "email")?,
        )
        .await
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_logout" => api::api_logout(state)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_me" => api::api_me(state)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_feed" => api::api_feed(state, arg(&args, "page")?, arg(&args, "size")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_category" => api::api_category(
            state,
            arg(&args, "cate")?,
            arg(&args, "page")?,
            arg(&args, "size")?,
        )
        .await
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_rank" => api::api_rank(state, arg(&args, "kind")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_search" => api::api_search(
            state,
            arg(&args, "q")?,
            arg(&args, "page")?,
            arg(&args, "size")?,
        )
        .await
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_posts" => api::api_posts(state, arg(&args, "page")?, arg(&args, "size")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_post" => api::api_post(state, arg(&args, "id")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_video" => api::api_video(state, arg(&args, "id")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_play" => api::api_play(state, arg(&args, "id")?, arg(&args, "ep")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_comments" => api::api_comments(
            state,
            arg(&args, "id")?,
            arg(&args, "ep")?,
            arg(&args, "page")?,
            arg(&args, "size")?,
        )
        .await
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_comment_send" => api::api_comment_send(
            state,
            arg(&args, "id")?,
            arg(&args, "ep")?,
            arg(&args, "text")?,
            arg(&args, "replyId")?,
        )
        .await
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_like" => api::api_like(
            state,
            arg(&args, "id")?,
            arg(&args, "ep")?,
            arg(&args, "on")?,
        )
        .await
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_collect" => api::api_collect(state, arg(&args, "id")?, arg(&args, "on")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_follow" => api::api_follow(state, arg(&args, "uid")?, arg(&args, "on")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "cache_clear" => api::cache_clear()
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_post_like" => api::api_post_like(state, arg(&args, "id")?, arg(&args, "on")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_post_collect" => api::api_post_collect(state, arg(&args, "id")?, arg(&args, "on")?)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "api_post_comments" => api::api_post_comments(
            state,
            arg(&args, "id")?,
            arg(&args, "page")?,
            arg(&args, "size")?,
        )
        .await
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "security_set" => {
            if args["strict"] != true {
                return Err("Server security cannot be disabled".into());
            }
            Ok(json!(true))
        }
        "debug_log" => Ok(Value::Null),
        "playback_log_status" => serde_json::to_value(crate::playback_log::playback_log_status()?)
            .map_err(|e| e.to_string()),
        "playback_log_write" => Ok(Value::Null),
        _ => Err("Unsupported command".into()),
    }
}
