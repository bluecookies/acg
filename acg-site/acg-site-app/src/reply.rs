use std::borrow::Cow;
use serde::Serialize;
use warp::Reply;
use crate::Error;

// A reply that is JSON on success
//  but a just a string on error
pub enum JsonReply {
    Success(warp::reply::Json),
    BadRequest(Cow<'static, str>),
    Error(Cow<'static, str>),
}

impl JsonReply {
    pub fn to_response(self) -> warp::reply::Response {
        use warp::http::StatusCode;
        match self {
            JsonReply::Success(j) => j.into_response(),
            JsonReply::BadRequest(s) =>
                warp::reply::with_status(s, StatusCode::BAD_REQUEST).into_response(),
            JsonReply::Error(s) =>
                warp::reply::with_status(s, StatusCode::INTERNAL_SERVER_ERROR).into_response(),
        }
    }
}

impl<T: Serialize, E: Into<Error>> From<Result<T, E>> for JsonReply {
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(v) => JsonReply::Success(warp::reply::json(&v)),
            Err(e) => JsonReply::Error(Cow::Owned(e.into().to_string())),
        }
    }
}
