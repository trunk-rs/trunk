use super::get_package_error::GetPackageError;
use super::node_package_information::NodePackageInformation;
use crate::version::VERSION;
use anyhow::Result;
use reqwest::{Client, StatusCode};
use url::Url;

pub struct NodePackageClient {
    client: Client,
    server: Url,
}

impl NodePackageClient {
    pub fn new(url: &str) -> Result<Self> {
        let client = Client::builder()
            .user_agent(format!("{}/{VERSION}", env!("CARGO_PKG_NAME")))
            .build()?;

        let server = Url::parse(url).map_err(|error| {
            std::io::Error::other(format!("the npm registry URL is well-formed: {error}"))
        })?;

        Ok(Self { client, server })
    }

    pub fn api_url<I>(&self, segments: I) -> Url
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut res = self.server.clone();

        if let Ok(mut p) = res.path_segments_mut() {
            p.extend(segments);
        }

        res
    }
}

impl Default for NodePackageClient {
    #![allow(clippy::unwrap_used)]
    fn default() -> Self {
        Self::new("https://registry.npmjs.org/").unwrap()
    }
}

impl NodePackageClient {
    pub async fn get_package(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Result<NodePackageInformation, GetPackageError> {
        let url = self.api_url([package_name, package_version]);

        let res = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| GetPackageError::Send(format!("{e:?}")))?;

        match res.status() {
            StatusCode::OK => {
                let body: NodePackageInformation = res
                    .json()
                    .await
                    .map_err(|e| GetPackageError::Receive(format!("{e:?}")))?;
                Ok(body)
            }
            StatusCode::NOT_FOUND => Err(GetPackageError::NotFound),
            code => Err(GetPackageError::Receive(format!(
                "unexpected status code {code}"
            ))),
        }
    }
}
