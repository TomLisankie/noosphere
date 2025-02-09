use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use tokio::io::AsyncRead;

/// A generic interface for interacting with an IPFS-like backend where it may
/// be desirable to syndicate sphere data to. Although the interface was
/// designed after a small subset of the capabilities of IPFS Kubo, it is
/// intended to be general enough to apply to other IPFS implementations.
#[async_trait]
pub trait IpfsClient {
    /// Returns true if the block (referenced by [Cid]) is pinned by the IPFS
    /// server
    async fn block_is_pinned(&self, cid: &Cid) -> Result<bool>;

    /// Returns a string that represents the identity (for example, a
    /// base64-encoded public key) of a node. This node is used to track
    /// syndication progress over time, so it should ideally be stable for a
    /// given server as the client interacts with it over time
    async fn server_identity(&self) -> Result<String>;

    /// Given some CAR bytes, syndicate that CAR to the IPFS server. Callers
    /// expect the roots in the CAR to be explicitly pinned, and for their
    /// descendents to be pinned by association.
    async fn syndicate_blocks<R>(&self, car: R) -> Result<()>
    where
        R: AsyncRead + Send + Sync + 'static;
}
