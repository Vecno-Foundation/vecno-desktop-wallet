use tauri::{command, State};
use vecno_wallet_core::prelude::*;
use vecno_consensus_core::network::{NetworkId, NetworkType};
use vecno_wallet_core::storage::local::{Storage, WalletStorage, Payload};
use vecno_wallet_core::storage::interface::{CreateArgs, OpenArgs};
use vecno_wallet_core::wallet::args::{AccountCreateArgsBip32, PrvKeyDataCreateArgs};
use vecno_wallet_core::storage::keydata::PrvKeyDataVariantKind;
use bip39::{Mnemonic, Language};
use rand::Rng;
use tauri::async_runtime::Mutex;
use async_std::sync::Arc;
use log::{error, info};
use serde::{Serialize, Deserialize};
use std::fs;
use vecno_wallet_core::settings::{application_folder, ensure_application_folder};
use vecno_wallet_core::storage::PrvKeyDataId;
use vecno_wrpc_client::prelude::{VecnoRpcClient, ConnectOptions};
use workflow_rpc::encoding::Encoding;
use std::time::Duration;
use futures_lite::stream::StreamExt;
use std::path::Path;

struct AppState {
    wallet: Mutex<Option<Arc<Wallet>>>,
    rpc_client: Mutex<Option<Arc<VecnoRpcClient>>>,
}

#[derive(Serialize, Deserialize)]
struct WalletAddress {
    account_name: String,
    account_index: u32,
    receive_address: String,
    change_address: String,
}

#[derive(Serialize, Deserialize)]
struct WalletFile {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[command]
async fn is_wallet_open(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| {
        let msg = "No wallet initialized";
        info!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;
    Ok(wallet.is_open())
}

#[command]
async fn is_node_connected(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
    let guard = state.rpc_client.lock().await;
    let rpc_client = guard.as_ref().ok_or_else(|| {
        let msg = "RPC client not initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;
    Ok(rpc_client.is_connected())
}

#[command]
async fn list_wallets() -> Result<Vec<WalletFile>, ErrorResponse> {
    let wallet_dir = application_folder().map_err(|e| {
        error!("Failed to get application folder: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let mut wallets = Vec::new();
    match fs::read_dir(&wallet_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if file_name.ends_with(".wallet") {
                        let path = entry.path().to_string_lossy().into_owned();
                        let name = file_name.strip_suffix(".wallet").unwrap_or(&file_name).to_string();
                        wallets.push(WalletFile { name, path });
                    }
                }
            }
            info!("Found {} wallets", wallets.len());
            Ok(wallets)
        }
        Err(e) => {
            error!("Failed to read wallet directory: {}", e);
            Err(ErrorResponse { error: format!("Failed to read wallet directory: {}", e) })
        }
    }
}

#[command]
async fn open_wallet(filename: String, secret: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    if filename.is_empty() {
        error!("Wallet filename is empty");
        return Err(ErrorResponse { error: "Wallet filename is required".to_string() });
    }
    if secret.is_empty() {
        error!("Wallet password is empty");
        return Err(ErrorResponse { error: "Wallet password is required".to_string() });
    }
    let _wallet_dir = application_folder().map_err(|e| {
        error!("Failed to get application folder: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let storage_path = Path::new(&filename);
    if !storage_path.exists() {
        error!("Wallet file does not exist: {}", filename);
        return Err(ErrorResponse { error: "Wallet file does not exist".to_string() });
    }

    // Extract the filename stem (without .wallet extension)
    let filename_stem = storage_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            error!("Invalid wallet filename: {}", filename);
            ErrorResponse { error: "Invalid wallet filename".to_string() }
        })?
        .to_string();

    let store = Wallet::local_store().map_err(|e| {
        error!("Local store creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let wallet_secret = Secret::new(secret.as_bytes().to_vec());
    info!("Opening wallet storage at {}", storage_path.display());
    let open_args = OpenArgs {
        filename: Some(filename_stem),
    };
    if let Err(e) = store.open(&wallet_secret, open_args).await {
        error!("Failed to open storage: {}", e);
        if e.to_string().to_lowercase().contains("decrypt") || e.to_string().to_lowercase().contains("crypto") {
            info!("Returning incorrect password error for storage open");
            return Err(ErrorResponse { error: "Incorrect password".to_string() });
        }
        return Err(ErrorResponse { error: format!("Failed to open storage: {}", e) });
    }

    info!("Creating wallet with NetworkId: Mainnet");
    let wallet = Arc::new(Wallet::try_new(store.clone(), None, Some(NetworkId::new(NetworkType::Mainnet))).map_err(|e| {
        error!("Wallet creation failed: {}", e);
        ErrorResponse { error: format!("Wallet creation failed: {}", e) }
    })?);

    info!("Wallet open status: {}", wallet.is_open());
    if !wallet.is_open() {
        error!("Wallet is not open after initialization");
        return Err(ErrorResponse { error: "Failed to open wallet: initialization error".to_string() });
    }

    let mut key_data_id: Option<PrvKeyDataId> = None;
    info!("Iterating over private key data");
    let mut keys = wallet.store().as_prv_key_data_store().map_err(|e| {
        error!("Failed to access private key data store: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.iter().await.map_err(|e| {
        error!("Failed to iterate over keys: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    while let Some(key_info) = keys.try_next().await.map_err(|e| {
        error!("Key iteration failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })? {
        key_data_id = Some(key_info.id);
        info!("Found private key data ID: {:?}", key_data_id);
        break;
    }

    if let Some(prv_key_data_id) = key_data_id {
        info!("Loading private key data for ID: {:?}", prv_key_data_id);
        if let Err(e) = store
            .as_prv_key_data_store()
            .map_err(|e| {
                error!("Failed to access private key data store: {}", e);
                ErrorResponse { error: e.to_string() }
            })?
            .load_key_data(&wallet_secret, &prv_key_data_id)
            .await
        {
            error!("Failed to load private key data: {}", e);
            if e.to_string().to_lowercase().contains("decrypt") || e.to_string().to_lowercase().contains("crypto") {
                info!("Returning incorrect password error for key data");
                return Err(ErrorResponse { error: "Incorrect password".to_string() });
            }
            return Err(ErrorResponse { error: "No private key data found".to_string() });
        }
    } else {
        error!("No private key data found in wallet");
        return Err(ErrorResponse { error: "No private key data found".to_string() });
    }

    // Iterate over accounts to select one
    info!("Iterating over accounts to select one");
    let mut account_id: Option<AccountId> = None;
    let mut accounts = wallet.store().as_account_store().map_err(|e| {
        error!("Failed to access account store: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.iter(None).await.map_err(|e| {
        error!("Failed to iterate over accounts: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    while let Some((account_storage, _metadata)) = accounts.try_next().await.map_err(|e| {
        error!("Account iteration failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })? {
        account_id = Some(*account_storage.id());
        info!("Found account with ID: {:?}", account_id);
        break;
    }

    if let Some(account_id) = account_id {
        info!("Loading account with ID: {:?}", account_id);
        let guard_obj = wallet.guard();
        let guard = guard_obj.lock().await;
        let account = wallet.get_account_by_id(&account_id, &guard).await.map_err(|e| {
            error!("Failed to load account: {}", e);
            ErrorResponse { error: format!("Failed to load account: {}", e) }
        })?;
        if let Some(account) = account {
            info!("Selecting account with ID: {:?}", account_id);
            wallet.select(Some(&account)).await.map_err(|e| {
                error!("Account selection failed: {}", e);
                ErrorResponse { error: format!("Failed to select account: {}", e) }
            })?;
        } else {
            error!("Account with ID {:?} not found", account_id);
            return Err(ErrorResponse { error: format!("Account with ID {:?} not found", account_id) });
        }
    } else {
        error!("No accounts found in wallet");
        return Err(ErrorResponse { error: "No accounts found in wallet".to_string() });
    }

    let account = wallet.account().map_err(|e| {
        error!("Account retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let rpc_client = state.rpc_client.lock().await;
    if rpc_client.is_none() {
        error!("RPC client not initialized");
        return Err(ErrorResponse { error: "RPC client not initialized".to_string() });
    }

    if let Err(e) = account.start().await {
        error!("Account start failed: {}", e);
        return Err(ErrorResponse { error: format!("Failed to start account: {}. Ensure Vecno node is running.", e) });
    }

    let mut wallet_state = state.wallet.lock().await;
    *wallet_state = Some(wallet.clone());
    info!("Wallet successfully opened from {}", storage_path.display());
    Ok(format!("Success: Wallet opened from {}", storage_path.display()))
}

#[command]
async fn generate_mnemonic() -> Result<String, ErrorResponse> {
    let entropy = rand::thread_rng().gen::<[u8; 32]>();
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).map_err(|e| {
        error!("Mnemonic generation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    Ok(mnemonic.to_string())
}

#[command]
async fn import_wallet(mnemonic: String, secret: String, filename: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    if secret.is_empty() {
        return Err(ErrorResponse { error: "Wallet password is required".to_string() });
    }
    if mnemonic.is_empty() {
        return Err(ErrorResponse { error: "Mnemonic is required".to_string() });
    }
    if filename.is_empty() {
        return Err(ErrorResponse { error: "Wallet filename is required".to_string() });
    }

    let mnemonic = Mnemonic::parse(&mnemonic).map_err(|e| {
        error!("Invalid mnemonic: {}", e);
        ErrorResponse { error: format!("Invalid mnemonic: Must be 24 words") }
    })?;
    if mnemonic.word_count() != 24 {
        return Err(ErrorResponse { error: "Mnemonic must be 24 words".to_string() });
    }

    let network_id = NetworkId::new(NetworkType::Mainnet);
    let wallet_dir = application_folder().map_err(|e| {
        error!("Failed to get application folder: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let storage_path = wallet_dir.join(&filename);

    let storage = Storage::try_new(storage_path.to_str().ok_or_else(|| ErrorResponse { error: "Invalid path".to_string() })?).map_err(|e| {
        error!("Storage initialization failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let store = Wallet::local_store().map_err(|e| {
        error!("Local store creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let wallet_secret = Secret::new(secret.as_bytes().to_vec());
    let create_args = CreateArgs {
        title: Some("Imported Wallet".to_string()),
        filename: Some(storage_path.to_str().ok_or_else(|| ErrorResponse { error: "Invalid path".to_string() })?.to_string()),
        encryption_kind: EncryptionKind::XChaCha20Poly1305,
        user_hint: None,
        overwrite_wallet: true,
    };
    store.create(&wallet_secret, create_args).await.map_err(|e| {
        error!("Store creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let wallet = Arc::new(Wallet::try_new(store.clone(), None, Some(network_id)).map_err(|e| {
        error!("Wallet creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?);

    if !wallet.is_open() {
        error!("Wallet is not open after initialization");
        return Err(ErrorResponse { error: "Failed to open wallet: initialization error".to_string() });
    }

    let prv_key_data = PrvKeyDataCreateArgs {
        name: None,
        payment_secret: None,
        secret: Secret::new(mnemonic.to_string().into_bytes()),
        kind: PrvKeyDataVariantKind::Mnemonic,
    };

    let key_id = wallet
        .create_prv_key_data(&wallet_secret, prv_key_data)
        .await
        .map_err(|e| {
            error!("Private key data creation failed: {}", e);
            ErrorResponse { error: e.to_string() }
        })?;

    let account_args = AccountCreateArgsBip32 {
        account_name: Some("default-account".to_string()),
        account_index: None,
    };

    let _account = wallet
        .create_account_bip32(&wallet_secret, key_id, None, account_args)
        .await
        .map_err(|e| {
            error!("Account creation failed: {}", e);
            ErrorResponse { error: e.to_string() }
        })?;

    let payload = Payload::new(vec![], vec![], vec![]);
    let wallet_storage = WalletStorage::try_new(
        Some("Imported Wallet".to_string()),
        None,
        &wallet_secret,
        EncryptionKind::XChaCha20Poly1305,
        payload,
        vec![],
    ).map_err(|e| {
        error!("Wallet storage creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    wallet_storage.try_store(&storage).await.map_err(|e| {
        error!("Wallet storage failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    wallet.select(Some(&_account)).await.map_err(|e| {
        error!("Wallet selection failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let rpc_client = state.rpc_client.lock().await;
    if rpc_client.is_none() {
        error!("RPC client not initialized");
        return Err(ErrorResponse { error: "RPC client not initialized".to_string() });
    }

    if let Err(e) = _account.start().await {
        error!("Account start failed: {}", e);
        return Err(ErrorResponse { error: format!("Failed to start account: {}. Ensure Vecno node is running.", e) });
    }

    let mut wallet_state = state.wallet.lock().await;
    *wallet_state = Some(wallet.clone());
    info!("Wallet successfully imported at {}", storage_path.display());
    Ok(format!("Success: Wallet imported at {}", storage_path.display()))
}

#[command]
async fn create_wallet(secret: String, filename: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    if secret.is_empty() {
        return Err(ErrorResponse { error: "Wallet password is required".to_string() });
    }
    if filename.is_empty() {
        return Err(ErrorResponse { error: "Wallet filename is required".to_string() });
    }
    let network_id = NetworkId::new(NetworkType::Mainnet);
    let wallet_dir = application_folder().map_err(|e| {
        error!("Failed to get application folder: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let storage_path = wallet_dir.join(&filename);

    let storage = Storage::try_new(storage_path.to_str().ok_or_else(|| ErrorResponse { error: "Invalid path".to_string() })?).map_err(|e| {
        error!("Storage initialization failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let store = Wallet::local_store().map_err(|e| {
        error!("Local store creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let wallet_secret = Secret::new(secret.as_bytes().to_vec());
    let create_args = CreateArgs {
        title: Some("My Wallet".to_string()),
        filename: Some(storage_path.to_str().ok_or_else(|| ErrorResponse { error: "Invalid path".to_string() })?.to_string()),
        encryption_kind: EncryptionKind::XChaCha20Poly1305,
        user_hint: None,
        overwrite_wallet: true,
    };
    store.create(&wallet_secret, create_args).await.map_err(|e| {
        error!("Store creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let wallet = Arc::new(Wallet::try_new(store.clone(), None, Some(network_id)).map_err(|e| {
        error!("Wallet creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?);

    if !wallet.is_open() {
        error!("Wallet is not open after initialization");
        return Err(ErrorResponse { error: "Failed to open wallet: initialization error".to_string() });
    }

    let entropy = rand::thread_rng().gen::<[u8; 32]>();
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).map_err(|e| {
        error!("Mnemonic generation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.to_string();

    let prv_key_data = PrvKeyDataCreateArgs {
        name: None,
        payment_secret: None,
        secret: Secret::new(mnemonic.clone().into_bytes()),
        kind: PrvKeyDataVariantKind::Mnemonic,
    };

    let key_id = wallet
        .create_prv_key_data(&wallet_secret, prv_key_data)
        .await
        .map_err(|e| {
            error!("Private key data creation failed: {}", e);
            ErrorResponse { error: e.to_string() }
        })?;

    let account_args = AccountCreateArgsBip32 {
        account_name: Some("default-account".to_string()),
        account_index: None,
    };

    let _account = wallet
        .create_account_bip32(&wallet_secret, key_id, None, account_args)
        .await
        .map_err(|e| {
            error!("Account creation failed: {}", e);
            ErrorResponse { error: e.to_string() }
        })?;

    let payload = Payload::new(vec![], vec![], vec![]);
    let wallet_storage = WalletStorage::try_new(
        Some("My Wallet".to_string()),
        None,
        &wallet_secret,
        EncryptionKind::XChaCha20Poly1305,
        payload,
        vec![],
    ).map_err(|e| {
        error!("Wallet storage creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    wallet_storage.try_store(&storage).await.map_err(|e| {
        error!("Wallet storage failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    wallet.select(Some(&_account)).await.map_err(|e| {
        error!("Wallet selection failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let rpc_client = state.rpc_client.lock().await;
    if rpc_client.is_none() {
        error!("RPC client not initialized");
        return Err(ErrorResponse { error: "RPC client not initialized".to_string() });
    }

    if let Err(e) = _account.start().await {
        error!("Account start failed: {}", e);
        return Err(ErrorResponse { error: format!("Failed to start account: {}. Ensure Vecno node is running.", e) });
    }

    let mut wallet_state = state.wallet.lock().await;
    *wallet_state = Some(wallet.clone());
    info!("Wallet successfully created at {}", storage_path.display());
    Ok(format!("Success: Wallet created at {} with mnemonic: {}", storage_path.display(), mnemonic))
}

#[command]
async fn get_address(state: State<'_, AppState>) -> Result<Vec<WalletAddress>, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| {
        let msg = "No wallet initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;

    if !wallet.is_open() {
        let msg = "Wallet is not open";
        error!("{}", msg);
        return Err(ErrorResponse { error: msg.to_string() });
    }

    let mut addresses = Vec::new();
    let account = wallet.account().map_err(|e| {
        error!("Account retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let receive_address = account.receive_address().map_err(|e| {
        error!("Receive address retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.to_string();
    let change_address = account.change_address().map_err(|e| {
        error!("Change address retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.to_string();

    addresses.push(WalletAddress {
        account_name: "default-account".to_string(),
        account_index: 0,
        receive_address,
        change_address,
    });

    info!("Successfully retrieved addresses");
    Ok(addresses)
}

#[command]
async fn get_balance(address: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| {
        let msg = "No wallet initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;

    if !wallet.is_open() {
        let msg = "Wallet is not open";
        error!("{}", msg);
        return Err(ErrorResponse { error: msg.to_string() });
    }

    let account = wallet.account().map_err(|e| {
        error!("Account retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    if address != account.receive_address().map_err(|e| ErrorResponse { error: e.to_string() })?.to_string() &&
       address != account.change_address().map_err(|e| ErrorResponse { error: e.to_string() })?.to_string() {
        return Err(ErrorResponse { error: "Invalid address for this account".to_string() });
    }
    let balance = account
        .balance()
        .ok_or_else(|| {
            let msg = "No balance available";
            error!("{}", msg);
            ErrorResponse { error: msg.to_string() }
        })?
        .mature;
    info!("Successfully retrieved balance for address: {}", address);
    Ok(balance.to_string())
}

#[command]
async fn send_transaction(to_address: String, amount: u64, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| {
        let msg = "No wallet initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;

    if !wallet.is_open() {
        let msg = "Wallet is not open";
        error!("{}", msg);
        return Err(ErrorResponse { error: msg.to_string() });
    }

    let to_addr = Address::try_from(to_address.as_str()).map_err(|e| {
        error!("Address parsing failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let account = wallet.account().map_err(|e| {
        error!("Account retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let destination = PaymentDestination::PaymentOutputs(PaymentOutputs::from((to_addr, amount)));
    let fees = Fees::SenderPays(amount);

    let tx_id = account
        .send(
            destination,
            None,
            fees,
            None,
            Secret::new("secret".as_bytes().to_vec()),
            None,
            &workflow_core::abortable::Abortable::default(),
            None,
        )
        .await
        .map_err(|e| {
            error!("Transaction send failed: {}", e);
            ErrorResponse { error: e.to_string() }
        })?;

    info!("Successfully sent transaction: {:?}", tx_id);
    Ok(format!("Transaction sent: {:?}", tx_id))
}

#[command]
async fn list_transactions(_state: State<'_, AppState>) -> Result<Vec<TransactionId>, ErrorResponse> {
    // Placeholder: Implement actual transaction listing logic
    Ok(vec![])
}

#[tokio::main]
async fn main() {
    env_logger::init(); // Initialize logging
    if let Err(e) = ensure_application_folder().await {
        eprintln!("Failed to create application folder: {}", e);
    }

    let rpc_client = Arc::new(VecnoRpcClient::new(
        Encoding::Borsh,
        Some("wss://wallet2.vecnoscan.org"),
        None,
        Some(NetworkId::new(NetworkType::Mainnet)),
        None,
    ).expect("Failed to create RPC client"));

    if let Err(e) = rpc_client.connect(Some(ConnectOptions {
        url: Some("wss://wallet2.vecnoscan.org".to_string()),
        connect_timeout: Some(Duration::from_millis(10000)),
        retry_interval: Some(Duration::from_secs(3)),
        ..Default::default()
    })).await {
        eprintln!("Failed to connect to RPC: {}", e);
    } else {
        info!("Successfully connected to RPC at ws://wallet2.vecnoscan.org");
    }

    tauri::Builder::default()
        .manage(AppState {
            wallet: Mutex::new(None),
            rpc_client: Mutex::new(Some(rpc_client)),
        })
        .invoke_handler(tauri::generate_handler![
            is_wallet_open,
            is_node_connected,
            create_wallet,
            import_wallet,
            generate_mnemonic,
            get_address,
            get_balance,
            send_transaction,
            list_wallets,
            open_wallet,
            list_transactions
        ])
        .run(tauri::generate_context!())
        .expect("Error running Vecno Wallet App");
}