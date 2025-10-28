use crate::state::{AppState, ErrorResponse, WalletAddress, WalletFile};
use tauri::{command, State};
use vecno_wallet_core::prelude::*;
use vecno_wallet_core::storage::local::{Storage, WalletStorage, Payload};
use vecno_wallet_core::storage::interface::{CreateArgs, OpenArgs};
use vecno_wallet_core::wallet::args::{AccountCreateArgsBip32, PrvKeyDataCreateArgs};
use vecno_wallet_core::storage::keydata::PrvKeyDataVariantKind;
use bip39::{Mnemonic, Language};
use rand::Rng;
use log::{error, info};
use std::fs;
use vecno_wallet_core::settings::{application_folder};
use vecno_wallet_core::storage::PrvKeyDataId;
use vecno_wrpc_client::prelude::{Resolver, WrpcEncoding, ConnectOptions, ConnectStrategy};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use futures_lite::stream::StreamExt;
use std::path::Path;
use std::sync::Arc;

#[command]
pub async fn list_wallets() -> Result<Vec<WalletFile>, ErrorResponse> {
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
pub async fn open_wallet(filename: String, secret: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
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

    // Attempt to open the wallet storage and explicitly check for decryption errors
    match store.open(&wallet_secret, open_args).await {
        Ok(_) => {
            info!("Wallet storage opened successfully");
        }
        Err(e) => {
            error!("Failed to open storage: {}", e);
            // Check if the error is related to decryption (indicating wrong password)
            if e.to_string().to_lowercase().contains("decrypt") || 
               e.to_string().to_lowercase().contains("crypto") || 
               e.to_string().to_lowercase().contains("invalid key") {
                info!("Returning incorrect password error for storage open");
                return Err(ErrorResponse { error: "Incorrect password provided".to_string() });
            }
            return Err(ErrorResponse { error: format!("Failed to open storage: {}", e) });
        }
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
        match store
            .as_prv_key_data_store()
            .map_err(|e| {
                error!("Failed to access private key data store: {}", e);
                ErrorResponse { error: e.to_string() }
            })?
            .load_key_data(&wallet_secret, &prv_key_data_id)
            .await
        {
            Ok(_) => {
                info!("Private key data loaded successfully");
            }
            Err(e) => {
                error!("Failed to load private key data: {}", e);
                // Check if the error is related to decryption (indicating wrong password)
                if e.to_string().to_lowercase().contains("decrypt") || 
                   e.to_string().to_lowercase().contains("crypto") || 
                   e.to_string().to_lowercase().contains("invalid key") {
                    info!("Returning incorrect password error for key data");
                    return Err(ErrorResponse { error: "Incorrect password provided".to_string() });
                }
                return Err(ErrorResponse { error: format!("Failed to load private key data: {}", e) });
            }
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
pub async fn generate_mnemonic() -> Result<String, ErrorResponse> {
    let entropy = rand::thread_rng().gen::<[u8; 32]>();
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).map_err(|e| {
        error!("Mnemonic generation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    Ok(mnemonic.to_string())
}

#[command]
pub async fn import_wallets(mnemonic: String, secret: String, filename: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
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
pub async fn create_wallet(secret: String, filename: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
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
pub async fn get_address(state: State<'_, AppState>) -> Result<Vec<WalletAddress>, ErrorResponse> {
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
pub async fn is_wallet_open(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
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