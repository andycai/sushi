use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

type HttpHandlerFuture = Pin<Box<dyn Future<Output = Result<HttpResponse, String>> + Send>>;
type HttpHandlerCallback = Arc<dyn Fn(HttpRequest) -> HttpHandlerFuture + Send + Sync>;

static NEXT_HTTP_HANDLER_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub dispatch_path: String,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        dispatch_path: impl Into<String>,
        body: Option<Vec<u8>>,
    ) -> Self {
        Self {
            method: method.into().to_uppercase(),
            path: path.into(),
            dispatch_path: dispatch_path.into(),
            headers: Vec::new(),
            body,
        }
    }

    pub fn with_headers(mut self, headers: Vec<(String, Vec<u8>)>) -> Self {
        self.headers = headers;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn from_plugin_body(response_body: String) -> Self {
        if let Some((file_name, content_type, body)) = parse_download_envelope(&response_body) {
            let safe_name = sanitize_content_disposition_name(&file_name);
            return Self::new(200, body)
                .with_header("content-type", content_type)
                .with_header(
                    "content-disposition",
                    format!("attachment; filename=\"{safe_name}\""),
                );
        }

        if let Some((status, body)) = parse_status_envelope(&response_body) {
            return Self::new(status, body).with_header("content-type", "application/json");
        }

        let content_type = infer_response_content_type(&response_body);
        Self::new(200, response_body).with_header("content-type", content_type)
    }
}

#[derive(Clone)]
pub struct HttpHandler {
    id: u64,
    callback: HttpHandlerCallback,
}

impl HttpHandler {
    pub fn new<F, Fut>(handler: F) -> Self
    where
        F: Fn(HttpRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HttpResponse, String>> + Send + 'static,
    {
        Self {
            id: NEXT_HTTP_HANDLER_ID.fetch_add(1, Ordering::Relaxed),
            callback: Arc::new(move |request| Box::pin(handler(request))),
        }
    }

    pub async fn call(&self, request: HttpRequest) -> Result<HttpResponse, String> {
        (self.callback)(request).await
    }
}

impl std::fmt::Debug for HttpHandler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpHandler")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for HttpHandler {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for HttpHandler {}

#[derive(serde::Deserialize)]
struct DownloadEnvelope {
    #[serde(default)]
    __app_web_download: bool,
    #[serde(default)]
    __sushi_file_download: bool,
    file_name: String,
    #[serde(default)]
    content_type: String,
    #[serde(default)]
    mime: String,
    body_hex: String,
}

fn parse_download_envelope(body: &str) -> Option<(String, String, Vec<u8>)> {
    let parsed: DownloadEnvelope = serde_json::from_str(body).ok()?;
    if !parsed.__app_web_download && !parsed.__sushi_file_download {
        return None;
    }
    let content_type = if parsed.content_type.is_empty() {
        parsed.mime
    } else {
        parsed.content_type
    };
    if content_type.is_empty() {
        return None;
    }
    let decoded = decode_hex_bytes(&parsed.body_hex)?;
    Some((parsed.file_name, content_type, decoded))
}

fn decode_hex_bytes(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(2) {
        return None;
    }
    let mut output = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let chunk = std::str::from_utf8(&bytes[index..index + 2]).ok()?;
        output.push(u8::from_str_radix(chunk, 16).ok()?);
        index += 2;
    }
    Some(output)
}

fn sanitize_content_disposition_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| match character {
            '"' | '\\' | '\r' | '\n' => '_',
            _ => character,
        })
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "download.bin".to_string()
    } else {
        sanitized
    }
}

fn infer_response_content_type(body: &str) -> &'static str {
    let trimmed = body.trim_start();
    if trimmed.starts_with('<') {
        "text/html; charset=utf-8"
    } else if serde_json::from_str::<Value>(body).is_ok() {
        "application/json"
    } else {
        "text/plain; charset=utf-8"
    }
}

fn parse_status_envelope(body: &str) -> Option<(u16, String)> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    let object = parsed.as_object()?;
    let sentinel = object
        .get("__app_web_json")
        .or_else(|| object.get("__sushi_web_json"))?
        .as_bool()?;
    if !sentinel {
        return None;
    }
    let status = u16::try_from(object.get("status")?.as_u64()?).ok()?;
    if !(100..=599).contains(&status) {
        return None;
    }
    let encoded = serde_json::to_string(object.get("body")?).ok()?;
    Some((status, encoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rust_handler_receives_normalized_request_and_returns_response() {
        let handler = HttpHandler::new(|request| async move {
            assert_eq!(request.method, "POST");
            assert_eq!(request.path, "/api/notes");
            assert_eq!(request.dispatch_path, "/api/notes?draft=true");
            assert_eq!(request.body, Some(vec![0xff, 0x00, b'a']));
            Ok(HttpResponse::new(201, b"created".to_vec())
                .with_header("content-type", "text/plain"))
        });

        let response = handler
            .call(HttpRequest::new(
                "POST",
                "/api/notes",
                "/api/notes?draft=true",
                Some(vec![0xff, 0x00, b'a']),
            ))
            .await
            .expect("handler succeeds");

        assert_eq!(response.status, 201);
        assert_eq!(response.body, b"created");
        assert_eq!(response.headers[0].0, "content-type");
    }

    #[test]
    fn plugin_envelopes_normalize_to_transport_response() {
        let status = HttpResponse::from_plugin_body(
            r#"{"__sushi_web_json":true,"status":202,"body":{"ok":true}}"#.to_string(),
        );
        assert_eq!(status.status, 202);
        assert_eq!(status.body, br#"{"ok":true}"#);

        let download = HttpResponse::from_plugin_body(
            r#"{"__sushi_file_download":true,"file_name":"report.bin","mime":"application/octet-stream","body_hex":"0001ff"}"#.to_string(),
        );
        assert_eq!(download.status, 200);
        assert_eq!(download.body, vec![0, 1, 255]);
        assert_eq!(
            download.headers[1],
            (
                "content-disposition".to_string(),
                "attachment; filename=\"report.bin\"".to_string()
            )
        );
    }
}
