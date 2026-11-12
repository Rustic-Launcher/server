//! Outgoing HTTP: only from the async instance (it blocks), only with the `network` capability
//! and only to the domains it lists.
//!
//! ```ignore
//! ctx.async_task("fetch-motd", |_: ()| {
//!     let body = Request::get("https://example.org/motd.txt").send()?.text()?;
//!     Scheduler::post_to_main("motd", &body)?;
//!     Ok(())
//! });
//! ```

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::__rt;
use crate::bindings::rustic::api::http as wit;
use crate::error::{Error, Result};
use crate::host::call;

/// An HTTP request under construction.
#[derive(Debug, Clone, PartialEq)]
#[must_use = "call .send()"]
pub struct Request {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    timeout_ms: u32,
}

impl Request {
    /// A request with any method.
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Request {
        Request {
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
            timeout_ms: 10_000,
        }
    }

    /// `GET url`.
    pub fn get(url: impl Into<String>) -> Request {
        Request::new("GET", url)
    }

    /// `POST url`.
    pub fn post(url: impl Into<String>) -> Request {
        Request::new("POST", url)
    }

    /// `PUT url`.
    pub fn put(url: impl Into<String>) -> Request {
        Request::new("PUT", url)
    }

    /// `DELETE url`.
    pub fn delete(url: impl Into<String>) -> Request {
        Request::new("DELETE", url)
    }

    /// Adds a header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Request {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Sets the body.
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Request {
        self.body = body.into();
        self
    }

    /// Sets a JSON body and `Content-Type: application/json`.
    pub fn json<T: Serialize + ?Sized>(self, value: &T) -> Result<Request> {
        let bytes = serde_json::to_vec(value)?;
        Ok(self.header("Content-Type", "application/json").body(bytes))
    }

    /// Timeout in milliseconds (default 10 s).
    pub fn timeout_ms(mut self, ms: u32) -> Request {
        self.timeout_ms = ms;
        self
    }

    /// Sends it and waits for the response. Fails with [`Error::AsyncOnly`] on the main instance.
    pub fn send(self) -> Result<Response> {
        if __rt::is_main() {
            return Err(Error::AsyncOnly("http.fetch"));
        }
        let req = wit::Request {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body,
            timeout_ms: self.timeout_ms,
        };
        let r = call!("http.fetch", wit::fetch(&req))?;
        Ok(Response {
            status: r.status,
            headers: r.headers,
            body: r.body,
        })
    }
}

/// An HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// Status code.
    pub status: u16,
    /// Headers in order.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

impl Response {
    /// Whether the status is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// A header (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The body as UTF-8 text.
    pub fn text(&self) -> Result<String> {
        String::from_utf8(self.body.clone())
            .map_err(|e| Error::Serde(format!("response body is not UTF-8: {e}")))
    }

    /// The body parsed as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_slice(&self.body)?)
    }

    /// An error unless the status is 2xx.
    pub fn error_for_status(self) -> Result<Response> {
        if self.is_success() {
            Ok(self)
        } else {
            Err(Error::Other(format!("HTTP status {}", self.status)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_and_response() {
        let r = Request::post("https://x.y")
            .json(&serde_json::json!({"a": 1}))
            .unwrap()
            .timeout_ms(500);
        assert_eq!(r.method, "POST");
        assert_eq!(r.body, br#"{"a":1}"#);
        assert_eq!(r.headers[0].1, "application/json");
        assert_eq!(r.timeout_ms, 500);

        let resp = Response {
            status: 200,
            headers: vec![("Content-Type".into(), "text/plain".into())],
            body: b"[1,2]".to_vec(),
        };
        assert!(resp.is_success());
        assert_eq!(resp.header("content-type"), Some("text/plain"));
        assert_eq!(resp.json::<Vec<u8>>().unwrap(), vec![1, 2]);
        assert_eq!(resp.text().unwrap(), "[1,2]");
        let bad = Response {
            status: 404,
            headers: vec![],
            body: vec![],
        };
        assert!(bad.error_for_status().is_err());
    }
}
