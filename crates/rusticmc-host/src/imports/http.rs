//! `http.fetch`: async instance only, capability `network` with the manifest's domain
//! allowlist checked against the URL host, bounded timeout and response size, no redirects
//! (a 3xx is returned to the plugin, so a redirect cannot leave the allowlist). ADR 0022.

use crate::bindings::rustic::api::{http as wh, types as wt};
use crate::config::HttpConfig;
use crate::state::HostState;
use std::time::{Duration, Instant};
use ureq::RequestExt;
use ureq::http::{Method, Uri};

/// The agent shared by every plugin: no redirects, HTTP errors are responses.
pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .user_agent(format!("Rustic/{}", env!("CARGO_PKG_VERSION")))
            .build(),
    )
}

/// Check a request against the capability and limits; returns the parsed URI and timeout.
pub(crate) fn validate(
    req: &wh::Request,
    allows: impl Fn(&str) -> bool,
    cfg: &HttpConfig,
) -> Result<(Method, Uri, Duration), wt::HostError> {
    let uri: Uri = req
        .url
        .parse()
        .map_err(|e| wt::HostError::Invalid(format!("bad URL `{}`: {e}", req.url)))?;
    match uri.scheme_str() {
        Some("https") => {}
        Some("http") if cfg.allow_plain_http => {}
        Some(s) => {
            return Err(wt::HostError::Invalid(format!(
                "URL scheme `{s}` is not allowed"
            )));
        }
        None => return Err(wt::HostError::Invalid("the URL needs a scheme".into())),
    }
    let host = uri
        .host()
        .ok_or_else(|| wt::HostError::Invalid("the URL has no host".into()))?;
    if req.url.contains('@') && uri.authority().is_some_and(|a| a.as_str().contains('@')) {
        return Err(wt::HostError::Invalid(
            "credentials in URLs are not allowed".into(),
        ));
    }
    if !allows(host) {
        return Err(wt::HostError::CapabilityDenied(format!(
            "network: `{host}` is not in the plugin's [capabilities] network list"
        )));
    }
    let method = Method::from_bytes(req.method.trim().to_ascii_uppercase().as_bytes())
        .map_err(|_| wt::HostError::Invalid(format!("bad HTTP method `{}`", req.method)))?;
    if req.body.len() > cfg.max_request_bytes {
        return Err(wt::HostError::Invalid(format!(
            "request body over {} bytes",
            cfg.max_request_bytes
        )));
    }
    let timeout = if req.timeout_ms == 0 {
        cfg.default_timeout
    } else {
        Duration::from_millis(u64::from(req.timeout_ms))
    }
    .min(cfg.max_timeout);
    Ok((method, uri, timeout))
}

fn perform(
    agent: &ureq::Agent,
    method: Method,
    uri: Uri,
    headers: &[(String, String)],
    body: Vec<u8>,
    timeout: Duration,
    max_bytes: u64,
) -> Result<wh::Response, wt::HostError> {
    let mut b = ureq::http::Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        b = b.header(k.as_str(), v.as_str());
    }
    let err = |e: ureq::Error| match e {
        ureq::Error::Timeout(t) => wt::HostError::Other(format!("timeout ({t})")),
        ureq::Error::BodyExceedsLimit(n) => {
            wt::HostError::Other(format!("response body over {n} bytes"))
        }
        other => wt::HostError::Other(other.to_string()),
    };
    let bad = |e: ureq::http::Error| wt::HostError::Invalid(format!("bad request: {e}"));
    let mut resp = if body.is_empty() {
        b.body(())
            .map_err(bad)?
            .with_agent(agent)
            .configure()
            .timeout_global(Some(timeout))
            .run()
    } else {
        b.body(body)
            .map_err(bad)?
            .with_agent(agent)
            .configure()
            .timeout_global(Some(timeout))
            .run()
    }
    .map_err(err)?;
    let status = resp.status().as_u16();
    let headers = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_owned(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    let body = resp
        .body_mut()
        .with_config()
        .limit(max_bytes)
        .read_to_vec()
        .map_err(err)?;
    Ok(wh::Response {
        status,
        headers,
        body,
    })
}

impl wh::Host for HostState {
    fn fetch(&mut self, req: wh::Request) -> Result<wh::Response, wt::HostError> {
        if self.is_main {
            return Err(wt::HostError::AsyncOnly);
        }
        let caps = &self.meta.capabilities;
        let cfg = &self.host.config.http;
        let (method, uri, timeout) = validate(&req, |h| caps.network_allows(h), cfg)?;
        let t0 = Instant::now();
        let r = perform(
            &self.host.http,
            method,
            uri,
            &req.headers,
            req.body,
            timeout,
            cfg.max_response_bytes,
        );
        // Time blocked on the network does not count against the call's deadline.
        self.clock.blocked += t0.elapsed();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(url: &str) -> wh::Request {
        wh::Request {
            method: "get".into(),
            url: url.into(),
            headers: vec![],
            body: vec![],
            timeout_ms: 0,
        }
    }

    fn allow(h: &str) -> bool {
        h == "api.example.org"
    }

    #[test]
    fn allowlist_and_url_checks() {
        let cfg = HttpConfig::default();
        let (m, u, t) = validate(&req("https://api.example.org/x?y=1"), allow, &cfg).unwrap();
        assert_eq!(m, Method::GET);
        assert_eq!(u.host(), Some("api.example.org"));
        assert_eq!(t, cfg.default_timeout);
        assert!(matches!(
            validate(&req("https://evil.example.org/"), allow, &cfg),
            Err(wt::HostError::CapabilityDenied(_))
        ));
        assert!(matches!(
            validate(&req("https://user:pw@api.example.org/"), allow, &cfg),
            Err(wt::HostError::Invalid(_))
        ));
        assert!(matches!(
            validate(&req("file:///etc/passwd"), allow, &cfg),
            Err(wt::HostError::Invalid(_))
        ));
        assert!(matches!(
            validate(&req("not a url"), allow, &cfg),
            Err(wt::HostError::Invalid(_))
        ));
        let mut long = req("https://api.example.org/");
        long.timeout_ms = 10_000_000;
        assert_eq!(validate(&long, allow, &cfg).unwrap().2, cfg.max_timeout);
        let mut big = req("https://api.example.org/");
        big.body = vec![0; cfg.max_request_bytes + 1];
        assert!(validate(&big, allow, &cfg).is_err());
        let strict = HttpConfig {
            allow_plain_http: false,
            ..HttpConfig::default()
        };
        assert!(validate(&req("http://api.example.org/"), allow, &strict).is_err());
    }
}
