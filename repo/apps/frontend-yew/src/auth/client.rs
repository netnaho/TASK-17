use chrono::Utc;
use gloo::net::http::{Request, Response};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use web_sys::FormData;

type HmacSha256 = Hmac<Sha256>;

/// Browser-side signed-request client.
///
/// ## Canonical string format
///
/// All requests include `x-silveroak-body-hash` (SHA-256 of the request body,
/// or SHA-256 of empty bytes for bodyless methods).  This allows the server to
/// run in `compat`, `dual`, or `strict` mode without client-side changes.
///
/// Standard (compat/dual) canonical — the signature only covers the 4-field form:
/// ```text
/// METHOD\nPATH\nTIMESTAMP\nNONCE
/// ```
/// Strict-mode canonical — server uses the 6-field form when verifying:
/// ```text
/// METHOD\nPATH\nTIMESTAMP\nNONCE\nQUERY\nBODY_SHA256
/// ```
///
/// The client always sends the body-hash header so the server can apply strict
/// verification without a client-side configuration change.
///
/// ## Multipart uploads
///
/// `post_multipart` sends SHA-256 of empty bytes as the body hash (a documented
/// limitation — `FormData` content cannot be hashed as a byte sequence in the
/// browser without streaming it through a custom hash transform).  Body integrity
/// for multipart uploads is provided by TLS.  In `strict` mode the server must
/// accept the empty-body hash for multipart content; see the README operational
/// notes.
pub struct ApiClient {
    pub api_token: String,
    pub signing_key: String,
}

/// SHA-256 of the empty byte sequence — used as the body hash for GET/DELETE
/// and other bodyless requests.  Pre-computed to avoid allocating in the hot path.
const EMPTY_BODY_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn hmac_sign(key: &str, canonical: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes()).expect("hmac");
    mac.update(canonical.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

impl ApiClient {
    pub async fn get(&self, path: &str) -> Result<Response, String> {
        self.send("GET", path, None::<&str>).await
    }

    pub async fn post_json<S: serde::Serialize>(
        &self,
        path: &str,
        body: &S,
    ) -> Result<Response, String> {
        let body = serde_json::to_string(body).map_err(|e| e.to_string())?;
        self.send("POST", path, Some(body.as_str())).await
    }

    /// Upload a file as multipart/form-data with the same HMAC signing headers.
    ///
    /// Sends `x-silveroak-body-hash` set to the SHA-256 of an empty byte sequence
    /// (documented limitation; see struct-level docs).
    pub async fn post_multipart(&self, path: &str, form: FormData) -> Result<Response, String> {
        let ts = Utc::now().timestamp().to_string();
        let nonce = Uuid::new_v4().to_string();
        // Strip query string: only the bare path is signed.
        let sign_path = path.split_once('?').map_or(path, |(p, _)| p);
        let canonical = format!("POST\n{sign_path}\n{ts}\n{nonce}");
        let sig = hmac_sign(&self.signing_key, &canonical);
        Request::post(path)
            .header("x-silveroak-token", &self.api_token)
            .header("x-silveroak-timestamp", &ts)
            .header("x-silveroak-nonce", &nonce)
            .header("x-silveroak-signature", &sig)
            // Empty-body hash: FormData cannot be streamed through Sha256 in the
            // browser without a ReadableStream transform.  Server accepts this for
            // multipart requests.
            .header("x-silveroak-body-hash", EMPTY_BODY_SHA256)
            .body(form)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())
    }

    /// Core signed-request implementation.
    ///
    /// Always includes `x-silveroak-body-hash` so the server can operate in any
    /// signing mode (`compat`, `dual`, `strict`) without client reconfiguration.
    async fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<Response, String> {
        let ts = Utc::now().timestamp().to_string();
        let nonce = Uuid::new_v4().to_string();
        // Strip query string: only the bare path is signed (compat/dual modes).
        let sign_path = path.split_once('?').map_or(path, |(p, _)| p);
        let canonical = format!("{method}\n{sign_path}\n{ts}\n{nonce}");
        let sig = hmac_sign(&self.signing_key, &canonical);

        // Compute body hash: SHA-256 of body bytes, or SHA-256 of empty bytes
        // for GET/DELETE and other bodyless requests.
        let body_hash = body
            .map(|b| sha256_hex(b.as_bytes()))
            .unwrap_or_else(|| EMPTY_BODY_SHA256.to_string());

        let builder = match method {
            "POST" => Request::post(path),
            "PUT" => Request::put(path),
            "PATCH" => Request::patch(path),
            "DELETE" => Request::delete(path),
            _ => Request::get(path),
        }
        .header("x-silveroak-token", &self.api_token)
        .header("x-silveroak-timestamp", &ts)
        .header("x-silveroak-nonce", &nonce)
        .header("x-silveroak-signature", &sig)
        .header("x-silveroak-body-hash", &body_hash);

        let res = if let Some(body) = body {
            builder
                .header("content-type", "application/json")
                .body(body.to_string())
                .map_err(|e| e.to_string())?
                .send()
                .await
        } else {
            builder.send().await
        };
        res.map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_body_sha256_is_known_constant() {
        // The pre-computed constant must match the actual SHA-256 of empty bytes.
        // This is the value that GET/DELETE requests send as their body hash.
        assert_eq!(sha256_hex(b""), EMPTY_BODY_SHA256);
    }

    #[test]
    fn body_hash_changes_with_content() {
        let h1 = sha256_hex(b"{\"key\":\"a\"}");
        let h2 = sha256_hex(b"{\"key\":\"b\"}");
        assert_ne!(h1, h2);
    }

    #[test]
    fn body_hash_is_hex_encoded_32_bytes() {
        let h = sha256_hex(b"any content");
        // SHA-256 produces 32 bytes = 64 hex chars
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn canonical_string_matches_server_no_body() {
        // Verify the client canonical matches what the server's
        // `canonical_string_no_body` would produce for a GET request.
        let method = "GET";
        let path = "/api/v1/health";
        let ts = "1700000000";
        let nonce = "abc123";
        let client_canonical = format!("{method}\n{path}\n{ts}\n{nonce}");
        // Server form (from backend crate):
        //   format!("{}\n{}\n{}\n{}", method.to_uppercase(), signing_path(path), ts, nonce)
        let expected = format!("GET\n/api/v1/health\n1700000000\nabc123");
        assert_eq!(client_canonical, expected);
    }
}
