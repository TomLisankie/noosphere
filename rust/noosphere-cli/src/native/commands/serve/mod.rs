pub mod authority;
pub mod extractor;
pub mod gateway;
pub mod ipfs;
pub mod route;
pub mod tracing;

use anyhow::Result;

use std::net::{IpAddr, TcpListener};

use url::Url;

use crate::native::workspace::Workspace;

use self::gateway::GatewayScope;

pub async fn serve(
    interface: IpAddr,
    port: u16,
    ipfs_api: Url,
    cors_origin: Option<Url>,
    workspace: &Workspace,
) -> Result<()> {
    let listener = TcpListener::bind(&(interface, port))?;

    let counterpart = workspace.counterpart_identity().await?;

    let identity = workspace.sphere_identity().await?;

    let gateway_scope = GatewayScope {
        identity,
        counterpart,
    };

    let sphere_context = workspace.sphere_context().await?;

    gateway::start_gateway(
        listener,
        gateway_scope,
        sphere_context,
        ipfs_api,
        cors_origin,
    )
    .await
}
