use super::IpfsClient;
use async_trait::async_trait;

use anyhow::{anyhow, Result};
use async_compat::CompatExt;
use cid::Cid;
use hyper::{
    client::connect::dns::GaiResolver, client::HttpConnector, Body, Client, Request, StatusCode,
};
use hyper_multipart_rfc7578::client::multipart::{Body as MultipartBody, Form};
use ipfs_api_prelude::response::{IdResponse, PinLsResponse};
use tokio::io::AsyncRead;
use url::Url;

/// A high-level HTTP client for accessing IPFS
/// [Kubo RPC APIs](https://docs.ipfs.tech/reference/kubo/rpc/) and normalizing
/// their expected payloads to Noosphere-friendly formats
#[derive(Clone)]
pub struct KuboClient {
    client: Client<HttpConnector<GaiResolver>>,
    api_url: Url,
}

#[async_trait]
impl IpfsClient for KuboClient {
    async fn block_is_pinned(&self, cid: &Cid) -> Result<bool> {
        let mut api_url = self.api_url.clone();
        let cid_base64 = cid.to_string();

        api_url.set_path("/api/v0/pin/ls");
        api_url.set_query(Some(&format!("arg={}", cid_base64)));

        let request = Request::builder()
            .method("POST")
            .uri(&api_url.to_string())
            .body(Body::empty())?;
        let response = self.client.request(request).await?;

        let body_bytes = hyper::body::to_bytes(response.into_body()).await?;
        match serde_json::from_slice(body_bytes.as_ref()) {
            Ok(PinLsResponse { keys }) => Ok(keys.contains_key(&cid_base64)),
            Err(_) => Ok(false),
        }
    }

    async fn server_identity(&self) -> Result<String> {
        let mut api_url = self.api_url.clone();

        api_url.set_path("/api/v0/id");

        let request = Request::builder()
            .method("POST")
            .uri(&api_url.to_string())
            .body(Body::empty())?;
        let response = self.client.request(request).await?;

        let body_bytes = hyper::body::to_bytes(response.into_body()).await?;

        match serde_json::from_slice(body_bytes.as_ref())? {
            IdResponse { public_key, .. } => Ok(public_key),
        }
    }

    async fn syndicate_blocks<R>(&self, car: R) -> Result<()>
    where
        R: AsyncRead + Send + Sync + 'static,
    {
        let mut api_url = self.api_url.clone();
        let mut form = Form::default();

        form.add_async_reader("file", Box::pin(car).compat());

        api_url.set_path("/api/v0/dag/import");

        let request_builder = Request::builder().method("POST").uri(&api_url.to_string());
        let request = form.set_body_convert::<Body, MultipartBody>(request_builder)?;

        let response = self.client.request(request).await?;

        match response.status() {
            StatusCode::OK => Ok(()),
            other_status => Err(anyhow!("Unexpected status code: {}", other_status)),
        }
    }
}

impl KuboClient {
    pub fn new(api_url: &Url) -> Result<Self> {
        let client = hyper::Client::builder().build_http();
        Ok(KuboClient {
            client,
            api_url: api_url.clone(),
        })
    }
}

// Note that these tests require that there is a locally available IPFS Kubo
// node running with the RPC API enabled
#[cfg(all(test, feature = "test_kubo"))]
mod tests {
    use std::io::Cursor;

    use cid::Cid;
    use iroh_car::{CarHeader, CarWriter};
    use libipld_cbor::DagCborCodec;
    use noosphere_storage::block_serialize;
    use serde::{Deserialize, Serialize};
    use url::Url;

    use super::{IpfsClient, KuboClient};
    use crate::native::commands::serve::tracing::initialize_tracing;

    #[tokio::test]
    pub async fn it_can_interact_with_a_kubo_server() {
        initialize_tracing();

        #[derive(Serialize, Deserialize)]
        struct SomeData {
            value: String,
            next: Option<Cid>,
        }

        let bar = SomeData {
            value: "bar".into(),
            next: None,
        };

        let (bar_cid, bar_block) = block_serialize::<DagCborCodec, _>(bar).unwrap();

        let foo = SomeData {
            value: "foo".into(),
            next: Some(bar_cid.clone()),
        };

        let (foo_cid, foo_block) = block_serialize::<DagCborCodec, _>(foo).unwrap();

        let mut car = Vec::new();

        let car_header = CarHeader::new_v1(vec![foo_cid.clone()]);
        let mut car_writer = CarWriter::new(car_header, &mut car);

        car_writer.write(foo_cid, foo_block).await.unwrap();
        car_writer.write(bar_cid, bar_block).await.unwrap();

        let kubo_client = KuboClient::new(&Url::parse("http://127.0.0.1:5001").unwrap()).unwrap();

        kubo_client.server_identity().await.unwrap();

        kubo_client
            .syndicate_blocks(Cursor::new(car))
            .await
            .unwrap();

        assert!(kubo_client.block_is_pinned(&foo_cid).await.unwrap());
        assert!(kubo_client.block_is_pinned(&bar_cid).await.unwrap());
    }

    #[tokio::test]
    pub async fn it_gives_a_useful_result_when_a_block_is_not_pinned() {
        initialize_tracing();

        let (cid, _) = block_serialize::<DagCborCodec, _>(vec![1, 2, 3]).unwrap();

        let kubo_client = KuboClient::new(&Url::parse("http://127.0.0.1:5001").unwrap()).unwrap();
        assert!(!kubo_client.block_is_pinned(&cid).await.unwrap());
    }
}
