use reqwest::Client;

use crate::{DiscoveryIndex, OpenApiError};

/// Extract the `hash` query parameter from a server-relative URL.
/// e.g. "/openapi/v3/api/v1?hash=ABC123" → Some("ABC123")
pub fn extract_hash(server_relative_url: &str) -> Option<String> {
    let query = server_relative_url.split('?').nth(1)?;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("hash=")
            && !value.is_empty()
        {
            return Some(value.to_string());
        }
    }
    None
}

pub async fn fetch_discovery(
    client: &Client,
    base_url: &str,
) -> Result<DiscoveryIndex, OpenApiError> {
    let url = format!("{base_url}/openapi/v3");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| OpenApiError::Http(format!("GET {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(OpenApiError::Http(format!(
            "GET {url}: status {}",
            resp.status()
        )));
    }
    resp.json::<DiscoveryIndex>()
        .await
        .map_err(|e| OpenApiError::Parse(format!("parse discovery from {url}: {e}")))
}

pub async fn fetch_spec(
    client: &Client,
    base_url: &str,
    server_relative_url: &str,
) -> Result<serde_json::Value, OpenApiError> {
    let url = format!("{base_url}{server_relative_url}");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| OpenApiError::Http(format!("GET {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(OpenApiError::Http(format!(
            "GET {url}: status {}",
            resp.status()
        )));
    }
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| OpenApiError::Parse(format!("parse spec from {url}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_hash_basic() {
        assert_eq!(
            extract_hash("/openapi/v3/api/v1?hash=ABC123"),
            Some("ABC123".to_string())
        );
    }

    #[test]
    fn extract_hash_multiple_params() {
        assert_eq!(
            extract_hash("/openapi/v3/api/v1?foo=bar&hash=XYZ&baz=qux"),
            Some("XYZ".to_string())
        );
    }

    #[test]
    fn extract_hash_no_hash() {
        assert_eq!(extract_hash("/openapi/v3/api/v1"), None);
        assert_eq!(extract_hash("/openapi/v3/api/v1?foo=bar"), None);
    }

    #[test]
    fn extract_hash_empty_value() {
        assert_eq!(extract_hash("/openapi/v3/api/v1?hash="), None);
    }
}
