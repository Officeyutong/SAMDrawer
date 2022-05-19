use std::fmt::Display;

use log::error;
use serde_json::json;

pub fn log_ise<T: Display>(e: T) -> actix_web::Error {
    error!("Error: {}", e);
    actix_web::error::ErrorInternalServerError(json!({
        "code":-1,
        "message":format!("{}",e)
    }))
}