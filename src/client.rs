use crate::auth::{auth_header_name, basic_auth_header};
use crate::config::Config;
use crate::error::{AppError, ErrorCode};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Space {
    pub id: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    pub status: String,
    pub title: String,
    pub space_id: Option<String>,
    pub parent_id: Option<String>,
    pub version: Option<PageVersion>,
    pub body: Option<Value>,
    #[serde(rename = "_links")]
    pub links: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageVersion {
    pub number: u64,
}

#[derive(Debug, Clone)]
pub struct CreatePageRequest {
    pub space_id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub storage_html: String,
}

#[derive(Debug, Clone)]
pub struct UpdatePageRequest {
    pub page_id: String,
    pub title: String,
    pub next_version: u64,
    pub storage_html: String,
}

pub struct ConfluenceClient {
    client: Client,
    base_url: Url,
    email: String,
    api_token: String,
}

impl ConfluenceClient {
    pub fn new(config: Config) -> Result<Self, AppError> {
        let config = config.validate()?;
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            auth_header_name(),
            basic_auth_header(&config.email, &config.api_token)?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|source| {
                AppError::new(
                    ErrorCode::ConfigInvalid,
                    format!("Failed to build HTTP client: {source}"),
                )
            })?;

        let base_url = Url::parse(config.site_url.trim_end_matches('/')).map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Failed to parse Confluence site URL: {source}"),
            )
        })?;

        Ok(Self {
            client,
            base_url,
            email: config.email,
            api_token: config.api_token,
        })
    }

    pub async fn list_spaces(&self) -> Result<Vec<Space>, AppError> {
        let result: MultiResult<Space> = self
            .request_json(Method::GET, "/api/v2/spaces", &[("limit", "25")])
            .await?;
        Ok(result.results)
    }

    pub async fn resolve_space_id(&self, key: &str) -> Result<String, AppError> {
        let result: MultiResult<Space> = self
            .request_json(
                Method::GET,
                "/api/v2/spaces",
                &[("keys", key), ("limit", "1")],
            )
            .await?;

        result
            .results
            .into_iter()
            .find(|space| space.key == key)
            .map(|space| space.id)
            .ok_or_else(|| {
                AppError::new(ErrorCode::NotFound, format!("Space {key} was not found."))
            })
    }

    pub async fn search(&self, cql: &str) -> Result<Value, AppError> {
        self.request_json(
            Method::GET,
            "/rest/api/search",
            &[("cql", cql), ("limit", "25")],
        )
        .await
    }

    pub async fn get_page(&self, page_id: &str) -> Result<Page, AppError> {
        let path = format!("/api/v2/pages/{page_id}");
        self.request_json(Method::GET, &path, &[("body-format", "storage")])
            .await
    }

    pub async fn create_page(&self, request: CreatePageRequest) -> Result<Value, AppError> {
        let payload = CreatePagePayload {
            space_id: request.space_id,
            status: "current",
            title: request.title,
            parent_id: request.parent_id,
            body: StorageBody {
                representation: "storage",
                value: request.storage_html,
            },
            subtype: "live",
        };
        self.send_json(Method::POST, "/api/v2/pages", &payload)
            .await
    }

    pub async fn update_page(&self, request: UpdatePageRequest) -> Result<Value, AppError> {
        let path = format!("/api/v2/pages/{}", request.page_id);
        let payload = UpdatePagePayload {
            id: request.page_id,
            status: "current",
            title: request.title,
            body: StorageBody {
                representation: "storage",
                value: request.storage_html,
            },
            version: UpdateVersion {
                number: request.next_version,
                message: "Updated by confluence-cli",
            },
        };
        self.send_json(Method::PUT, &path, &payload).await
    }

    async fn request_json<T>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, AppError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = self.endpoint(path)?;
        let response = self
            .client
            .request(method.clone(), url)
            .query(query)
            .send()
            .await
            .map_err(transport_error)?;

        self.parse_response(method, path, response).await
    }

    async fn send_json<T>(&self, method: Method, path: &str, payload: &T) -> Result<Value, AppError>
    where
        T: Serialize + ?Sized,
    {
        let url = self.endpoint(path)?;
        let response = self
            .client
            .request(method.clone(), url)
            .header(CONTENT_TYPE, "application/json")
            .json(payload)
            .send()
            .await
            .map_err(transport_error)?;

        self.parse_response(method, path, response).await
    }

    async fn parse_response<T>(
        &self,
        method: Method,
        path: &str,
        response: reqwest::Response,
    ) -> Result<T, AppError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let status = response.status();
        if status.is_success() {
            return response.json::<T>().await.map_err(|source| {
                AppError::new(
                    ErrorCode::ConfluenceValidationFailed,
                    format!("Failed to parse Confluence response JSON: {source}"),
                )
            });
        }

        let summary = response
            .text()
            .await
            .map(|body| self.sanitize_response_body(&body))
            .unwrap_or_else(|_| String::new());
        Err(self.status_error(method, path, status, summary))
    }

    fn endpoint(&self, path: &str) -> Result<Url, AppError> {
        let mut url = self.base_url.clone();
        let base_path = url.path().trim_end_matches('/');
        let request_path = path.trim_start_matches('/');
        let joined = if base_path.is_empty() {
            format!("/{request_path}")
        } else {
            format!("{base_path}/{request_path}")
        };
        url.set_path(&joined);
        url.set_query(None);
        Ok(url)
    }

    fn status_error(
        &self,
        method: Method,
        path: &str,
        status: StatusCode,
        summary: String,
    ) -> AppError {
        let code = match status {
            StatusCode::UNAUTHORIZED => ErrorCode::AuthFailed,
            StatusCode::FORBIDDEN => ErrorCode::PermissionDenied,
            StatusCode::NOT_FOUND => ErrorCode::NotFound,
            StatusCode::CONFLICT if method == Method::PUT && path.starts_with("/api/v2/pages/") => {
                ErrorCode::ConfluenceVersionConflict
            }
            StatusCode::TOO_MANY_REQUESTS => ErrorCode::RateLimited,
            status if status.is_server_error() => ErrorCode::NetworkError,
            status if status.is_client_error() => ErrorCode::ConfluenceValidationFailed,
            _ => ErrorCode::NetworkError,
        };

        AppError::new(code, format!("Confluence returned HTTP {status}."))
            .with_retryable(matches!(
                code,
                ErrorCode::NetworkError | ErrorCode::RateLimited
            ))
            .with_details(json!({
                "status": status.as_u16(),
                "response_body": truncate_summary(&summary),
            }))
    }

    fn sanitize_response_body(&self, body: &str) -> String {
        body.replace(&self.api_token, "[redacted]")
            .replace(&self.email, "[redacted]")
    }
}

#[derive(Debug, Deserialize)]
struct MultiResult<T> {
    results: Vec<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePagePayload {
    space_id: String,
    status: &'static str,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<String>,
    body: StorageBody,
    subtype: &'static str,
}

#[derive(Debug, Serialize)]
struct StorageBody {
    representation: &'static str,
    value: String,
}

#[derive(Debug, Serialize)]
struct UpdatePagePayload {
    id: String,
    status: &'static str,
    title: String,
    body: StorageBody,
    version: UpdateVersion,
}

#[derive(Debug, Serialize)]
struct UpdateVersion {
    number: u64,
    message: &'static str,
}

fn transport_error(source: reqwest::Error) -> AppError {
    AppError::new(
        ErrorCode::NetworkError,
        format!("Failed to reach Confluence: {source}"),
    )
    .with_retryable(true)
}

fn truncate_summary(summary: &str) -> String {
    const MAX_CHARS: usize = 512;
    let mut chars = summary.chars();
    let truncated: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}
