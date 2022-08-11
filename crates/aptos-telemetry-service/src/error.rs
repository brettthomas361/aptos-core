// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use aptos_logger::error;
use serde::Serialize;
use std::convert::Infallible;
use thiserror::Error;
use warp::{http::StatusCode, Rejection, Reply};

#[derive(Error, Debug)]
pub enum Error {
    #[error("wrong public key")]
    WrongPublicKey,
    #[error("invalid custom event")]
    InvalidCustomEvent,
    #[error("gcp insert error")]
    GCPInsertError,
    #[error("jwt token not valid")]
    JWTTokenError,
}

#[derive(Serialize, Debug)]
struct ErrorResponse {
    message: String,
    status: String,
}

impl warp::reject::Reject for Error {}

pub async fn handle_rejection(err: Rejection) -> std::result::Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(e) = err.find::<Error>() {
        match e {
            Error::WrongPublicKey => (StatusCode::BAD_REQUEST, e.to_string()),
            Error::InvalidCustomEvent => (StatusCode::BAD_REQUEST, e.to_string()),
            Error::GCPInsertError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            ),
            Error::JWTTokenError => (StatusCode::UNAUTHORIZED, e.to_string()),
        }
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed".to_string(),
        )
    } else {
        error!("unhandled error: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
    };

    let json = warp::reply::json(&ErrorResponse {
        status: code.to_string(),
        message,
    });

    Ok(warp::reply::with_status(json, code))
}