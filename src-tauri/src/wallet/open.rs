use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use vecno_wallet_core::prelude::*;
use vecno_wallet_core::storage::interface::OpenArgs;
use vecno_wrpc_client::prelude::{Resolver, WrpcEncoding, ConnectOptions, ConnectStrategy};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use futures_lite::stream::StreamExt;
use log::{error, info};
use std::path::Path;
use std::sync::Arc;
use vecno_wallet_core::settings::application_folder;
use vecno_wallet_core::storage::PrvKeyDataId;
use vecno_wallet_core::storage::keydata::PrvKeyDataVariant;

#[command]
pub async fn open_wallet(
    filename: String,
    secret: String,
    state: State<'_, AppState>,
) -> Result<String, ErrorResponse> {
    // -------------------------------------------------
    // 1. Basic validation
    // -------------------------------------------------
    if filename.is_empty() {
        error!("Wallet filename is empty");
        return Err(ErrorResponse {
            error: "Wallet filename is required".to_string(),
        });
    }
    if secret.is_empty() {
        error!("Wallet password is empty");
        return Err(ErrorResponse {
            error: "Wallet password is required".to_string(),
        });
    }

    // -------------------------------------------------
    // 2. Resolve storage path
    // -------------------------------------------------
    let _wallet_dir = application_folder().map_err(|e| {
        error!("Failed to get application folder: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let storage_path = Path::new(&filename);
    if !storage_path.exists() {
        error!("Wallet file does not exist: {}", filename);
        return Err(ErrorResponse {
            error: "Wallet file does not exist".to_string(),
        });
    }

    let filename_stem = storage_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            error!("Invalid wallet filename: {}", filename);
            ErrorResponse {
                error: "Invalid wallet filename".to_string(),
            }
        })?
        .to_string();

    // -------------------------------------------------
    // 3. Open local store
    // -------------------------------------------------
    let store = Wallet::local_store().map_err(|e| {
        error!("Local store creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let wallet_secret = Secret::new(secret.as_bytes().to_vec());
    info!("Opening wallet storage at {}", storage_path.display());

    let open_args = OpenArgs {
        filename: Some(filename_stem),
    };

    match store.open(&wallet_secret, open_args).await {
        Ok(_) => info!("Wallet storage opened successfully"),
        Err(e) => {
            error!("Failed to open storage: {}", e);
            if e.to_string().to_lowercase().contains("decrypt")
                || e.to_string().to_lowercase().contains("crypto")
                || e.to_string().to_lowercase().contains("invalid key")
            {
                return Err(ErrorResponse {
                    error: "Incorrect password provided".to_string(),
                });
            }
            return Err(ErrorResponse {
                error: format!("Failed to open storage: {}", e),
            });
        }
    }

    // -------------------------------------------------
    // 4. Build Wallet + wRPC connection
    // -------------------------------------------------
    let network_id = NetworkId::new(NetworkType::Mainnet);
    let resolver = Resolver::default();

    let wallet = Arc::new(
        Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id)).map_err(|e| {
            error!("Wallet creation failed: {}", e);
            ErrorResponse {
                error: format!("Wallet creation failed: {}", e),
            }
        })?,
    );

    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        let url = resolver
            .get_url(WrpcEncoding::Borsh, network_id)
            .await
            .map_err(|e| {
                error!("Failed to get resolver URL: {}", e);
                ErrorResponse {
                    error: format!(
                        "Failed to resolve node URL: {}. Ensure seed.vecnoscan.org is reachable.",
                        e
                    ),
                }
            })?;
        let options = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };
        wrpc_client.connect(Some(options)).await.map_err(|e| {
            error!("Failed to connect to node: {}", e);
            ErrorResponse {
                error: format!(
                    "Failed to connect to node: {}. Ensure seed.vecnoscan.org is reachable or run a local node.",
                    e
                ),
            }
        })?;
    } else {
        return Err(ErrorResponse {
            error: "No wRPC client available.".to_string(),
        });
    }

    if !wallet.is_open() {
        return Err(ErrorResponse {
            error: "Failed to open wallet: initialization error".to_string(),
        });
    }

    // -------------------------------------------------
    // 5. Load PrvKeyData → extract mnemonic (if any)
    // -------------------------------------------------
    let mut key_data_id: Option<PrvKeyDataId> = None;
    let mut keys = wallet.store().as_prv_key_data_store()?.iter().await?;
    while let Some(key_info) = keys.try_next().await? {
        key_data_id = Some(key_info.id);
        break; // we only need the first one
    }

    let mnemonic_opt: Option<String> = if let Some(prv_key_data_id) = key_data_id {
        // Load the (still encrypted) PrvKeyData
        let encrypted_key_data = store
            .as_prv_key_data_store()?
            .load_key_data(&wallet_secret, &prv_key_data_id)
            .await
            .map_err(|e| {
                if e.to_string().to_lowercase().contains("decrypt") {
                    ErrorResponse {
                        error: "Incorrect password provided".to_string(),
                    }
                } else {
                    ErrorResponse {
                        error: format!("Failed to load private key data: {}", e),
                    }
                }
            })?;

        // Decrypt the payload (Encryptable<T> → T)
        let payload = encrypted_key_data
            .unwrap().payload
            .decrypt(Some(&wallet_secret))
            .map_err(|e| {
                error!("Failed to decrypt private key data: {}", e);
                ErrorResponse {
                    error: "Failed to decrypt wallet data. Incorrect password?".to_string(),
                }
            })?;

        // Public accessor → Zeroizing<PrvKeyDataVariant>
        let variant = payload.as_variant();

        match *variant {
            PrvKeyDataVariant::Mnemonic(ref mnemonic_str) => {
                // Remove possible null-padding and whitespace
                Some(mnemonic_str.trim_end_matches('\0').trim().to_owned())
            }
            _ => None,
        }
    } else {
        return Err(ErrorResponse {
            error: "No private key data found".to_string(),
        });
    };

    // -------------------------------------------------
    // 6. Load & select the first account
    // -------------------------------------------------
    let mut account_id: Option<AccountId> = None;
    let mut accounts = wallet.store().as_account_store()?.iter(None).await?;
    while let Some((account_storage, _)) = accounts.try_next().await? {
        account_id = Some(*account_storage.id());
        break;
    }

    if let Some(account_id) = account_id {
        let guard_obj = wallet.guard();
        let guard = guard_obj.lock().await;
        let account = wallet
            .get_account_by_id(&account_id, &guard)
            .await?
            .ok_or_else(|| ErrorResponse {
                error: format!("Account with ID {:?} not found", account_id),
            })?;
        wallet.select(Some(&account)).await?;
    } else {
        return Err(ErrorResponse {
            error: "No accounts found in wallet".to_string(),
        });
    }

    let account = wallet.account()?;
    account.start().await.map_err(|e| {
        ErrorResponse {
            error: format!("Failed to start account: {}. Ensure node is reachable.", e),
        }
    })?;

    // -------------------------------------------------
    // 7. Store everything into AppState
    // -------------------------------------------------
    {
        let mut wallet_state = state.wallet.lock().await;
        let mut resolver_state = state.resolver.lock().await;
        let mut secret_state = state.wallet_secret.lock().await;
        let mut mnemonic_state = state.mnemonic.lock().await;

        *wallet_state = Some(wallet.clone());
        *resolver_state = Some(resolver);
        *secret_state = Some(wallet_secret);
        *mnemonic_state = mnemonic_opt;
    }

    Ok(format!(
        "Success: Wallet opened from {}",
        storage_path.display()
    ))
}