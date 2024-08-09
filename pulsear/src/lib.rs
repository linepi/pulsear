#![allow(dead_code)]
pub mod api;

pub mod file;
pub use file::*;

pub mod auth;
pub use auth::*;

pub mod util;
pub use util::*;

pub mod websocket;
pub use websocket::*;

pub mod sql;
pub use sql::*;

pub mod server;
pub use server::*;

pub use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer};
pub use actix_files::NamedFile;
pub use actix_web_actors::ws;
pub use bytes::Buf;
pub use mysql::params;
pub use mysql::prelude::*;
pub use mysql::TxOpts;
pub use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
pub use std::collections::HashMap;
pub use std::fmt;
pub use std::hash::Hash;
pub use std::os::unix::fs::MetadataExt;
pub use std::sync::{Arc, RwLock};
pub use std::time::*;
type Err = Box<dyn std::error::Error>;