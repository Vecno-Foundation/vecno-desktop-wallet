use crate::state::{AppState, ErrorResponse, NodeInfo};
use tauri::{command, State};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use vecno_wrpc_client::prelude::WrpcEncoding;
use log::{error, info};

#[command]
pub async fn is_node_connected(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
    {
        let cache_guard = state.node_cache.lock().await;
        if cache_guard.url.is_some() {
            info!("Node already known to be connected (cached).");
            return Ok(true);
        }
    }

    let guard = state.resolver.lock().await;
    let resolver = guard.as_ref().ok_or_else(|| {
        let msg = "Resolver not initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;

    let network_id = NetworkId::new(NetworkType::Mainnet);
    info!("Attempting to resolve node URL (cache miss) for network ID: {:?}", network_id);

    match resolver.get_url(WrpcEncoding::Borsh, network_id).await {
        Ok(url) => {
            info!("Successfully resolved node URL: {}", url);
            {
                let mut cache_guard = state.node_cache.lock().await;
                cache_guard.url = Some(url.clone());
            }
            Ok(true)
        }
        Err(e) => {
            {
                let mut cache_guard = state.node_cache.lock().await;
                cache_guard.url = None;
            }
            error!(
                "Node connection failed: {}. Check Resolvers.toml for valid endpoints",
                e
            );
            Err(ErrorResponse {
                error: "Failed to connect to Vecno node. Check your internet connection or try again later.".to_string(),
            })
        }
    }
}

#[command]
pub async fn get_node_info(state: State<'_, AppState>) -> Result<NodeInfo, ErrorResponse> {
    {
        let cache_guard = state.node_cache.lock().await;
        if let Some(url) = &cache_guard.url {
            info!("Returning cached node URL: {}", url);
            return Ok(NodeInfo { url: url.clone() });
        }
    }

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
            {
                let mut cache_guard = state.node_cache.lock().await;
                cache_guard.url = Some(url.clone());
            }
            Ok(NodeInfo { url })
        }
        Err(e) => {
            {
                let mut cache_guard = state.node_cache.lock().await;
                cache_guard.url = None;
            }
            error!(
                "Failed to retrieve node URL: {}. Check Resolvers.toml for valid endpoints.",
                e
            );
            Err(ErrorResponse {
                error: "Failed to connect to Vecno node. Check your internet connection or try again later.".to_string(),
            })
        }
    }
}