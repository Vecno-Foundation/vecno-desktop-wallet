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
use vecno_wrpc_client::prelude::{Resolver, WrpcEncoding, ConnectOptions, ConnectStrategy};
use futures_lite::stream::StreamExt;
use std::path::Path;
use workflow_core::abortable::Abortable;
use std::sync::Arc as StdArc;

struct AppState {
    wallet: Mutex<Option<Arc<Wallet>>>,
    resolver: Mutex<Option<Resolver>>,
    wallet_secret: Mutex<Option<Secret>>,
}

#[derive(Serialize, Debug, Deserialize)]
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

#[derive(Serialize, Deserialize)]
struct NodeInfo {
    url: String,
}

#[command]
async fn is_wallet_open(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| {
        let msg = "No wallet initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;
    let is_open = wallet.is_open();
    info!("is_wallet_open: wallet exists: {}, is_open: {}", guard.is_some(), is_open);
    Ok(is_open)
}

#[command]
async fn is_node_connected(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
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
            error!("Node connection failed: {}. Check Resolvers.toml for valid endpoints (e.g., ws://localhost:8110, wss://wallet.vecnoscan.org).", e);
            Err(ErrorResponse { error: format!("Node connection failed: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) })
        }
    }
}

#[command]
async fn get_node_info(state: State<'_, AppState>) -> Result<NodeInfo, ErrorResponse> {
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

    let network_id = NetworkId::new(NetworkType::Mainnet);
    let resolver = Resolver::default();
    info!("Creating wallet with NetworkId: Mainnet");
    let wallet = Arc::new(Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id)).map_err(|e| {
        error!("Wallet creation failed: {}", e);
        ErrorResponse { error: format!("Wallet creation failed: {}", e) }
    })?);

    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        let url = resolver.get_url(WrpcEncoding::Borsh, network_id).await.map_err(|e| {
            error!("Failed to get resolver URL: {}", e);
            ErrorResponse { error: format!("Failed to resolve node URL: {}. Ensure seed.vecnoscan.org is reachable.", e) }
        })?;
        info!("Connecting to node: {}", url);
        let options = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };
        wrpc_client.connect(Some(options)).await.map_err(|e| {
            error!("Failed to connect to node: {}. Check Resolvers.toml for valid endpoints.", e);
            ErrorResponse { error: format!("Failed to connect to node: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) }
        })?;
    } else {
        error!("No wRPC client available for wallet");
        return Err(ErrorResponse { error: "No wRPC client available. Ensure wallet is properly initialized.".to_string() });
    }

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

    let mut account_id: Option<AccountId> = None;
    info!("Iterating over accounts to select one");
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

    if let Err(e) = account.start().await {
        error!("Account start failed: {}. Ensure Vecno node is running.", e);
        return Err(ErrorResponse { error: format!("Failed to start account: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) });
    }

    let mut wallet_state = state.wallet.lock().await;
    let mut resolver_state = state.resolver.lock().await;
    let mut secret_state = state.wallet_secret.lock().await;
    *wallet_state = Some(wallet.clone());
    *resolver_state = Some(resolver);
    *secret_state = Some(wallet_secret);
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
async fn import_wallets(mnemonic: String, secret: String, filename: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
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

    let resolver = Resolver::default();
    info!("Initializing resolver for wallet import");
    let wallet = Arc::new(Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id)).map_err(|e| {
        error!("Wallet creation failed: {}", e);
        ErrorResponse { error: format!("Wallet creation failed: {}", e) }
    })?);

    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        let url = resolver.get_url(WrpcEncoding::Borsh, network_id).await.map_err(|e| {
            error!("Failed to get resolver URL: {}", e);
            ErrorResponse { error: format!("Failed to resolve node URL: {}. Ensure seed.vecnoscan.org is reachable.", e) }
        })?;
        info!("Connecting to node: {}", url);
        let options = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };
        wrpc_client.connect(Some(options)).await.map_err(|e| {
            error!("Failed to connect to node: {}. Check Resolvers.toml for valid endpoints.", e);
            ErrorResponse { error: format!("Failed to connect to node: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) }
        })?;
    } else {
        error!("No wRPC client available for wallet");
        return Err(ErrorResponse { error: "No wRPC client available. Ensure wallet is properly initialized.".to_string() });
    }

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

    if let Err(e) = _account.start().await {
        error!("Account start failed: {}. Ensure Vecno node is running.", e);
        return Err(ErrorResponse { error: format!("Failed to start account: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) });
    }

    let mut wallet_state = state.wallet.lock().await;
    let mut resolver_state = state.resolver.lock().await;
    let mut secret_state = state.wallet_secret.lock().await;
    *wallet_state = Some(wallet.clone());
    *resolver_state = Some(resolver);
    *secret_state = Some(wallet_secret);
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

    let resolver = Resolver::default();
    info!("Initializing resolver for wallet creation");
    let wallet = Arc::new(Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id)).map_err(|e| {
        error!("Wallet creation failed: {}", e);
        ErrorResponse { error: format!("Wallet creation failed: {}", e) }
    })?);

    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        let url = resolver.get_url(WrpcEncoding::Borsh, network_id).await.map_err(|e| {
            error!("Failed to get resolver URL: {}", e);
            ErrorResponse { error: format!("Failed to resolve node URL: {}. Ensure seed.vecnoscan.org is reachable.", e) }
        })?;
        info!("Connecting to node: {}", url);
        let options = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };
        wrpc_client.connect(Some(options)).await.map_err(|e| {
            error!("Failed to connect to node: {}. Check Resolvers.toml for valid endpoints.", e);
            ErrorResponse { error: format!("Failed to connect to node: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) }
        })?;
    } else {
        error!("No wRPC client available for wallet");
        return Err(ErrorResponse { error: "No wRPC client available. Ensure wallet is properly initialized.".to_string() });
    }

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

    if let Err(e) = _account.start().await {
        error!("Account start failed: {}. Ensure Vecno node is running.", e);
        return Err(ErrorResponse { error: format!("Failed to start account: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) });
    }

    let mut wallet_state = state.wallet.lock().await;
    let mut resolver_state = state.resolver.lock().await;
    let mut secret_state = state.wallet_secret.lock().await;
    *wallet_state = Some(wallet.clone());
    *resolver_state = Some(resolver);
    *secret_state = Some(wallet_secret);
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

    info!("Successfully retrieved addresses: {:?}", addresses);
    Ok(addresses)
}

#[command]
async fn get_balance(address: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let secret_guard = state.wallet_secret.lock().await;
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

    let wallet_secret = secret_guard.as_ref().cloned().ok_or_else(|| {
        let msg = "Wallet secret not set";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;

    let account = wallet.account().map_err(|e| {
        error!("Account retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    if address != account.receive_address().map_err(|e| {
        error!("Receive address retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.to_string() &&
       address != account.change_address().map_err(|e| {
        error!("Change address retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.to_string() {
        error!("Invalid address for this account: {}", address);
        return Err(ErrorResponse { error: "Invalid address for this account".to_string() });
    }

    let derivation_account = account.clone()
        .as_derivation_capable()
        .map_err(|e| {
            error!("Account is not derivation capable: {}", e);
            ErrorResponse { error: "Account does not support address derivation".to_string() }
        })?;

    info!("Starting derivation scan for address: {}", address);
    let abortable = Abortable::new();
    let balance = StdArc::new(Mutex::new(0u64));
    let balance_clone = balance.clone();

    // Check if wallet is connected before scanning
    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        let network_id = NetworkId::new(NetworkType::Mainnet);
        if let Some(resolver) = wrpc_client.resolver() {
            let url = resolver.get_url(WrpcEncoding::Borsh, network_id).await.map_err(|e| {
                error!("Failed to get resolver URL: {}", e);
                ErrorResponse { error: format!("Failed to resolve node URL: {}. Ensure seed.vecnoscan.org is reachable.", e) }
            })?;
            info!("Connecting to node for balance scan: {}", url);
            let options = ConnectOptions {
                block_async_connect: true,
                strategy: ConnectStrategy::Fallback,
                url: Some(url),
                ..Default::default()
            };
            wrpc_client.connect(Some(options)).await.map_err(|e| {
                error!("Failed to connect to node: {}. Check Resolvers.toml for valid endpoints.", e);
                ErrorResponse { error: format!("Failed to connect to node: {}. Ensure seed.vecnoscan.org is reachable or run a local node (e.g., ws://localhost:8110).", e) }
            })?;
        } else {
            error!("No resolver configured for wallet");
            return Err(ErrorResponse { error: "No resolver configured. Check Resolvers.toml for valid seed nodes.".to_string() });
        }
    } else {
        error!("No wRPC client available for wallet");
        return Err(ErrorResponse { error: "No wRPC client available. Ensure wallet is properly initialized.".to_string() });
    }

    // Perform derivation scan, explicitly ignoring the Result
    let _ = derivation_account
        .derivation_scan(
            wallet_secret,
            None, // payment_secret
            0,    // start
            1000, // Reduced count for faster testing
            128,  // window
            false, // sweep
            None, // fee_rate
            &abortable,
            true, // verbose
            Some(StdArc::new(move |processed: usize, _, found_balance, txid| {
                let value = balance_clone.clone();
                tauri::async_runtime::spawn(async move {
                    let mut balance_guard = value.lock().await;
                    *balance_guard = found_balance;
                    if let Some(txid) = txid {
                        info!("Scan detected {} VE at index {}; transfer txid: {}", found_balance, processed, txid);
                    } else if processed > 0 {
                        info!("Scanned {} derivations, found {} VE", processed, found_balance);
                    } else {
                        info!("Scanning for account UTXOs...");
                    }
                });
            })),
        )
        .await;

    // Wait briefly to ensure the callback has time to update the balance
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Retrieve the balance
    let balance_value = *balance.lock().await;
    info!("Balance scan completed for address {}: {} VE", address, balance_value);
    Ok(balance_value.to_string())
}

#[command]
async fn send_transaction(to_address: String, amount: u64, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let secret_guard = state.wallet_secret.lock().await;
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

    let wallet_secret = secret_guard.as_ref().cloned().ok_or_else(|| {
        let msg = "Wallet secret not set";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;

    let store = wallet.store().clone();
    let network_id = NetworkId::new(NetworkType::Mainnet);
    let resolver = Resolver::default();
    info!("Attempting to initialize resolver for transaction");
    let new_wallet = Arc::new(Wallet::try_new(store.clone(), Some(resolver), Some(network_id)).map_err(|e| {
        error!("Wallet creation failed: {}", e);
        ErrorResponse { error: format!("Wallet creation failed: {}", e) }
    })?);

    if let Some(wrpc_client) = new_wallet.try_wrpc_client().as_ref() {
        if let Some(resolver) = wrpc_client.resolver() {
            info!("Resolver initialized, querying seed.vecnoscan.org for wallet nodes");
            let url = resolver.get_url(WrpcEncoding::Borsh, network_id).await.map_err(|e| {
                error!("Failed to get resolver URL: {}", e);
                ErrorResponse { error: format!("Failed to resolve node URL: {}. Ensure seed.vecnoscan.org is reachable.", e) }
            })?;
            info!("Connecting to node: {}", url);
            let options = ConnectOptions {
                block_async_connect: true,
                strategy: ConnectStrategy::Fallback,
                url: Some(url),
                ..Default::default()
            };
            wrpc_client.connect(Some(options)).await.map_err(|e| {
                error!("Failed to connect to node: {}. Check Resolvers.toml for valid endpoints.", e);
                ErrorResponse { error: format!("Failed to connect to node: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) }
            })?;
        } else {
            error!("No resolver configured for wallet");
            return Err(ErrorResponse { error: "No resolver configured. Check Resolvers.toml for valid seed nodes.".to_string() });
        }
    } else {
        error!("No wRPC client available for wallet");
        return Err(ErrorResponse { error: "No wRPC client available. Ensure wallet is properly initialized.".to_string() });
    }

    let mut key_data_id: Option<PrvKeyDataId> = None;
    info!("Iterating over private key data for new wallet");
    let mut keys = new_wallet.store().as_prv_key_data_store().map_err(|e| {
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

    let mut account_id: Option<AccountId> = None;
    info!("Iterating over accounts to select one for new wallet");
    let mut accounts = new_wallet.store().as_account_store().map_err(|e| {
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
        let guard_obj = new_wallet.guard();
        let guard = guard_obj.lock().await;
        let account = new_wallet.get_account_by_id(&account_id, &guard).await.map_err(|e| {
            error!("Failed to load account: {}", e);
            ErrorResponse { error: format!("Failed to load account: {}", e) }
        })?;
        if let Some(account) = account {
            info!("Selecting account with ID: {:?}", account_id);
            new_wallet.select(Some(&account)).await.map_err(|e| {
                error!("Account selection failed: {}", e);
                ErrorResponse { error: format!("Failed to select account: {}", e) }
            })?;
            if let Err(e) = account.start().await {
                error!("Account start failed: {}. Ensure Vecno node is running.", e);
                return Err(ErrorResponse { error: format!("Failed to start account: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) });
            }
        } else {
            error!("Account with ID {:?} not found", account_id);
            return Err(ErrorResponse { error: format!("Account with ID {:?} not found", account_id) });
        }
    } else {
        error!("No accounts found in wallet");
        return Err(ErrorResponse { error: "No accounts found in wallet".to_string() });
    }

    let to_addr = Address::try_from(to_address.as_str()).map_err(|e| {
        error!("Address parsing failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let account = new_wallet.account().map_err(|e| {
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
            wallet_secret,
            None,
            &Abortable::new(),
            None,
        )
        .await
        .map_err(|e| {
            error!("Transaction send failed: {}. Check if RPC nodes in Resolvers.toml are running.", e);
            ErrorResponse {
                error: format!("Failed to send transaction: {}. Ensure seed.vecnoscan.org is reachable or run a local node (e.g., ws://localhost:8110).", e)
            }
        })?;

    info!("Successfully sent transaction: {:?}", tx_id);
    Ok(format!("Transaction sent: {:?}", tx_id))
}

#[command]
async fn list_transactions(_state: State<'_, AppState>) -> Result<Vec<TransactionId>, ErrorResponse> {
    Ok(vec![])
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    if let Err(e) = ensure_application_folder().await {
        eprintln!("Failed to create application folder: {}", e);
    }

    let _network_id = NetworkId::new(NetworkType::Mainnet);
    let resolver = Resolver::default();
    info!("Main: Initialized resolver with Resolvers.toml");

    tauri::Builder::default()
        .manage(AppState {
            wallet: Mutex::new(None),
            resolver: Mutex::new(Some(resolver)),
            wallet_secret: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            is_wallet_open,
            is_node_connected,
            get_node_info,
            create_wallet,
            import_wallets,
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