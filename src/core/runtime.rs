use serde::{Serialize, de::DeserializeOwned};
use url::Url;

use crate::{
    config::{ClientConfig, ResolvedClientConfig},
    core::request::{PreparedRequest, RequestOptions, ResolvedRequestOptions},
    error::{ErrorKind, OpenAIError},
};

/// Shared immutable runtime used by the root client and family handles.
#[derive(Clone, Debug)]
pub(crate) struct ClientRuntime {
    config: ClientConfig,
}

impl ClientRuntime {
    pub(crate) fn new(config: ClientConfig) -> Self {
        Self { config }
    }

    pub(crate) fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub(crate) fn resolved_config(&self) -> Result<ResolvedClientConfig, OpenAIError> {
        self.config.resolve()
    }

    pub(crate) fn prepare_request(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Result<PreparedRequest, OpenAIError> {
        self.prepare_request_with_body(method, path, None)
    }

    pub(crate) fn prepare_request_with_body(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        body: Option<Vec<u8>>,
    ) -> Result<PreparedRequest, OpenAIError> {
        let method = method.as_ref().trim().to_ascii_uppercase();
        if method.is_empty() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "request method cannot be blank",
            ));
        }

        let endpoint = normalize_endpoint(path.as_ref());
        if endpoint.is_empty() {
            return Err(OpenAIError::new(
                ErrorKind::Validation,
                "request path cannot be blank",
            ));
        }

        let resolved = self.resolved_config()?;

        Ok(PreparedRequest {
            method,
            url: join_url(&resolved.base_url, &endpoint)?,
            headers: resolved.headers(),
            body,
        })
    }

    pub(crate) fn prepare_json_request<B>(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        body: &B,
    ) -> Result<PreparedRequest, OpenAIError>
    where
        B: Serialize,
    {
        let body = serde_json::to_vec(body).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Validation,
                format!("failed to serialize request body: {error}"),
            )
            .with_source(error)
        })?;
        let mut request = self.prepare_request_with_body(method, path, Some(body))?;
        request.headers.insert(
            String::from("content-type"),
            String::from("application/json"),
        );
        request
            .headers
            .insert(String::from("accept"), String::from("application/json"));
        Ok(request)
    }

    pub(crate) fn resolve_request_options(
        &self,
        options: &RequestOptions,
    ) -> Result<ResolvedRequestOptions, OpenAIError> {
        let resolved = self.resolved_config()?;
        Ok(ResolvedRequestOptions {
            timeout: options.timeout.unwrap_or(resolved.timeout),
            max_retries: options.max_retries.unwrap_or(resolved.max_retries),
        })
    }

    pub(crate) fn execute_json<T>(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<crate::core::response::ApiResponse<T>, OpenAIError>
    where
        T: DeserializeOwned,
    {
        let request = self.prepare_request(method, path)?;
        let resolved_options = self.resolve_request_options(&options)?;
        crate::core::transport::execute_json(&request, &resolved_options)
    }

    pub(crate) fn execute_json_with_body<B, T>(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        body: &B,
        options: RequestOptions,
    ) -> Result<crate::core::response::ApiResponse<T>, OpenAIError>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        let request = self.prepare_json_request(method, path, body)?;
        let resolved_options = self.resolve_request_options(&options)?;
        crate::core::transport::execute_json(&request, &resolved_options)
    }

    pub(crate) fn execute_unit(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<crate::core::response::ApiResponse<()>, OpenAIError> {
        let mut request = self.prepare_request(method, path)?;
        request
            .headers
            .insert(String::from("accept"), String::from("*/*"));
        let resolved_options = self.resolve_request_options(&options)?;
        crate::core::transport::execute_unit(&request, &resolved_options)
    }

    pub(crate) fn execute_text(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<crate::core::response::ApiResponse<String>, OpenAIError> {
        let request = self.prepare_request(method, path)?;
        let resolved_options = self.resolve_request_options(&options)?;
        crate::core::transport::execute_text(&request, &resolved_options)
    }

    pub(crate) fn execute_text_with_body<B>(
        &self,
        method: impl AsRef<str>,
        path: impl AsRef<str>,
        body: &B,
        options: RequestOptions,
    ) -> Result<crate::core::response::ApiResponse<String>, OpenAIError>
    where
        B: Serialize,
    {
        let request = self.prepare_json_request(method, path, body)?;
        let resolved_options = self.resolve_request_options(&options)?;
        crate::core::transport::execute_text(&request, &resolved_options)
    }
}

fn normalize_endpoint(path: &str) -> String {
    path.trim()
        .trim_start_matches('/')
        .trim_start_matches("v1/")
        .to_string()
}

fn join_url(base_url: &str, endpoint: &str) -> Result<String, OpenAIError> {
    let mut url = Url::parse(base_url).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Configuration,
            format!("invalid OpenAI base URL `{base_url}`: {error}"),
        )
    })?;
    let (endpoint_path, endpoint_query) = endpoint
        .split_once('?')
        .map_or((endpoint, None), |(path, query)| (path, Some(query)));
    let mut path = url.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        path.push_str("/v1");
    }
    path.push('/');
    path.push_str(endpoint_path);
    url.set_path(&path);
    url.set_query(endpoint_query.filter(|query| !query.is_empty()));
    Ok(url.to_string().trim_end_matches('/').to_string())
}
