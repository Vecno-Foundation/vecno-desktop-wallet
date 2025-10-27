use crate::state::{AppState, ErrorResponse, NodeInfo};
use tauri::{command, State};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use vecno_wrpc_client::prelude::{WrpcEncoding};
use log::{error, info};

#[command]
pub async fn is_node_connected(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
    let guard = state.resolver.lock().await;
    let resolver = guard.as_ref().ok_or_else(|| {
        let msg = "Resolver not initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;
    let network_id = NetworkId::new(NetworkType::Mainnet);
    info!("Attempting to connect to resolver with network ID: {:?}", network_id);
    match resolver.get_url(WrpcEncoding::Borsh, network_id).await {
        Ok(url) => {
            info!("Successfully resolved node URL: {}", url);
            Ok(true)
        }
        Err(e) => {
            error!("Node connection failed: {}. Check Resolvers.toml for valid endpoints", e);
            Err(ErrorResponse { error: format!("Node connection failed: {}. Ensure Resolver is reachable or run a local node.", e) })
        }
    }
}

#[command]
pub async fn get_node_info(state: State<'_, AppState>) -> Result<NodeInfo, ErrorResponse> {
    let guard = state.resolver.lock().await;
    let resolver = guard.as_ref().ok_or_else(|| {
        let msg = "Resolver not initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;
    let network_id = NetworkId::new(NetworkType::Mainnet);
    match resolver.get_url(WrpcEncoding::Borsh, network_id).await {
        Ok(url) => {
            info!("Retrieved node URL: {}", url);
            Ok(NodeInfo { url })
        }
        Err(e) => {
            error!("Failed to retrieve node URL: {}. Check Resolvers.toml for valid endpoints.", e);
            Err(ErrorResponse { error: format!("Failed to retrieve node info: {}. Ensure seed.vecnoscan.org is reachable.", e) })
        }
    }
}