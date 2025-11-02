// src-tauri/src/send_transactions.rs
use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use vecno_addresses::Address;
use vecno_wallet_core::prelude::*;
use vecno_wrpc_client::prelude::RpcApi;
use vecno_wallet_core::tx::generator::{Generator, GeneratorSettings};
use vecno_wallet_core::tx::{PaymentDestination, PaymentOutputs, PaymentOutput, Fees};
use vecno_wallet_core::utxo::{
    scan::DEFAULT_WINDOW_SIZE, Scan, ScanExtent, balance::AtomicBalance, UtxoContext,
    UtxoEntryReference,
};
use vecno_wallet_core::utxo::UtxoContextBinding;
use vecno_wallet_core::derivation::AddressManager;
use std::sync::Arc;
use workflow_core::prelude::Abortable;
use vecno_wallet_core::tx::generator::signer::Signer;

async fn get_mature_utxos(ctx: &UtxoContext) -> Result<Vec<UtxoEntryReference>, ErrorResponse> {
    let entries = ctx
        .get_utxos(None, None)
        .await
        .map_err(|e| ErrorResponse { error: format!("get_utxos failed: {e}") })?;

    Ok(entries.into_iter().map(UtxoEntryReference::from).collect())
}

async fn fetch_current_daa_score(rpc: &dyn RpcApi) -> Result<u64, ErrorResponse> {
    let info = rpc
        .get_server_info()
        .await
        .map_err(|e| ErrorResponse { error: format!("RPC get_server_info failed: {e}") })?;

    Ok(info.virtual_daa_score)
}

#[command]
pub async fn send_transaction(
    to_address: String,
    amount: u64,
    state: State<'_, AppState>,
) -> Result<String, ErrorResponse> {
    let wallet_guard = state.wallet.lock().await;
    let wallet = wallet_guard
        .as_ref()
        .ok_or(ErrorResponse { error: "Wallet is not open".into() })?
        .clone();

    if !wallet.is_open() {
        return Err(ErrorResponse { error: "Wallet is not open".into() });
    }

    let account_trait: Arc<dyn Account> = wallet
        .account()
        .map_err(ErrorResponse::from)?;
    let account: Arc<dyn Account> = account_trait.clone();

    let wallet_secret_guard = state.wallet_secret.lock().await;
    let wallet_secret = wallet_secret_guard
        .as_ref()
        .ok_or(ErrorResponse { error: "Wallet secret not loaded".into() })?;

    let prv_key_data_id = account
        .prv_key_data_id()?
        .clone();

    let prv_key_data = wallet
        .get_prv_key_data(wallet_secret, &prv_key_data_id)
        .await
        .map_err(|e| ErrorResponse { error: format!("Failed to load PrvKeyData: {e}") })?
        .ok_or(ErrorResponse { error: "PrvKeyData not found".into() })?;

    drop(wallet_secret_guard);

    let processor = wallet.utxo_processor().clone();
    let binding = UtxoContextBinding::AccountId(*account.id());
    let utxo_context = Arc::new(UtxoContext::new(&processor, binding));

    let derivation = account
        .clone()
        .as_derivation_capable()
        .map_err(|e| ErrorResponse { error: format!("Account is not derivation-capable: {e}") })?;

    let receive_manager: Arc<AddressManager> = derivation.derivation().receive_address_manager();
    let change_manager: Arc<AddressManager> = derivation.derivation().change_address_manager();

    // Ensure addresses are valid
    receive_manager
        .current_address()
        .map_err(|e| ErrorResponse { error: format!("Receive address error: {e}") })?;
    change_manager
        .current_address()
        .map_err(|e| ErrorResponse { error: format!("Change address error: {e}") })?;

    let rpc = wallet.rpc_api();
    let current_daa_score = fetch_current_daa_score(rpc.as_ref()).await?;

    // Scan receive and change addresses
    let receive_scan = Scan::new_with_address_manager(
        receive_manager.clone(),
        &Arc::new(AtomicBalance::default()),
        current_daa_score,
        Some(DEFAULT_WINDOW_SIZE),
        Some(ScanExtent::EmptyWindow),
    );
    let change_scan = Scan::new_with_address_manager(
        change_manager.clone(),
        &Arc::new(AtomicBalance::default()),
        current_daa_score,
        Some(DEFAULT_WINDOW_SIZE),
        Some(ScanExtent::EmptyWindow),
    );

    tokio::try_join!(
        receive_scan.scan(&utxo_context),
        change_scan.scan(&utxo_context)
    )
    .map_err(|e| ErrorResponse { error: format!("Scan failed: {e}") })?;

    // Collect mature UTXOs
    let utxo_entries = get_mature_utxos(&utxo_context).await?;
    let total_available: u64 = utxo_entries.iter().map(|u| u.amount()).sum();

    log::info!(
        "send_transaction: Using {} UTXOs totaling {} VENI (need: {})",
        utxo_entries.len(),
        total_available,
        amount
    );

    if total_available < amount {
        return Err(ErrorResponse {
            error: format!(
                "Insufficient funds: need {} VENI, have {}",
                amount, total_available
            ),
        });
    }

    // Convert to iterator for generator
    let utxo_iterator = utxo_entries.into_iter().map(UtxoEntryReference::from);

    // Create signer
    let signer = Arc::new(Signer::new(
        account.clone(),
        prv_key_data,
        None::<Secret>,
    ));

    // Parse target address
    let target_address = Address::try_from(to_address.as_str())
        .map_err(|e| ErrorResponse { error: format!("Invalid address: {e}") })?;

    // Get change address
    let change_address = account
        .change_address()
        .map_err(|e| ErrorResponse { error: format!("Change address error: {e}") })?;

    // Generator settings
    let settings = GeneratorSettings {
        network_id: wallet.network_id()?,
        multiplexer: None,
        utxo_iterator: Box::new(utxo_iterator),
        source_utxo_context: None,
        priority_utxo_entries: None,
        sig_op_count: account.sig_op_count(),
        minimum_signatures: account.minimum_signatures(),
        change_address: change_address.clone(),
        fee_rate: None,
        final_transaction_priority_fee: Fees::SenderPays(0),
        final_transaction_destination: PaymentDestination::PaymentOutputs(PaymentOutputs {
            outputs: vec![PaymentOutput::new(target_address.clone(), amount)],
        }),
        final_transaction_payload: None,
        destination_utxo_context: None,
    };

    let abortable = Abortable::default();
    let generator = Generator::try_new(settings, Some(signer), Some(&abortable))
        .map_err(|e| ErrorResponse { error: format!("Generator creation failed: {e}") })?;

    let mut tx_ids = Vec::new();

    for (i, pending_tx_result) in generator.iter().enumerate() {
        let pending_tx = pending_tx_result
            .map_err(|e| ErrorResponse { error: format!("Generator error at tx #{}: {e}", i + 1) })?;

        pending_tx
            .try_sign()
            .map_err(|e| ErrorResponse { error: format!("Signing failed for tx #{}: {e}", i + 1) })?;

        let rpc_id = pending_tx
            .try_submit(&rpc)
            .await
            .map_err(|e| ErrorResponse { error: format!("Submit failed for tx #{}: {e}", i + 1) })?;

        tx_ids.push(rpc_id.to_string());
    }

    // Only return the LAST transaction ID
    let last_tx_id = tx_ids.last().cloned().unwrap_or_default();
    log::info!("Successfully submitted {} transaction(s). Last TXID: {}", tx_ids.len(), last_tx_id);

    Ok(last_tx_id)
}