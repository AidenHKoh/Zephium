//! The session bearer's home in the OS vault: one item per install, shared by
//! every profile.

use crate::AccountToken;

/// Content-free vault failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultFault {
    Missing,
    Inaccessible,
}

/// Secret storage boundary. Tests supply memory vaults, never the OS one.
pub trait AccountVault: Send + Sync {
    fn load(&self) -> Result<AccountToken, VaultFault>;
    fn store(&self, token: &AccountToken) -> Result<(), VaultFault>;
    fn clear(&self) -> Result<(), VaultFault>;
    /// Presence from attributes alone, without reading the secret.
    fn present(&self) -> Result<bool, VaultFault>;
}

/// The Keychain on macOS and Credential Manager on Windows.
pub struct SystemVault;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod platform {
    use super::{AccountToken, AccountVault, SystemVault, VaultFault};
    use zephium_credentials::VaultError;

    const SERVICE: &str = "app.zephium.account";

    fn fault(error: VaultError) -> VaultFault {
        match error {
            VaultError::Missing => VaultFault::Missing,
            _ => VaultFault::Inaccessible,
        }
    }

    impl AccountVault for SystemVault {
        fn load(&self) -> Result<AccountToken, VaultFault> {
            let _turn = zephium_credentials::turn();
            let bytes = match zephium_credentials::read(SERVICE) {
                Err(VaultError::Inaccessible) => {
                    // Concurrent login-keychain reads fail transiently.
                    std::thread::sleep(std::time::Duration::from_millis(40));
                    zephium_credentials::read(SERVICE)
                }
                read => read,
            }
            .map_err(fault)?;
            let text = std::str::from_utf8(&bytes).map_err(|_| VaultFault::Inaccessible)?;
            AccountToken::new(text.to_owned()).map_err(|_| VaultFault::Inaccessible)
        }

        fn store(&self, token: &AccountToken) -> Result<(), VaultFault> {
            let _turn = zephium_credentials::turn();
            zephium_credentials::write(SERVICE, token.expose().as_bytes()).map_err(fault)
        }

        fn clear(&self) -> Result<(), VaultFault> {
            let _turn = zephium_credentials::turn();
            zephium_credentials::delete(SERVICE).map_err(fault)
        }

        fn present(&self) -> Result<bool, VaultFault> {
            let _turn = zephium_credentials::turn();
            zephium_credentials::present(SERVICE).map_err(fault)
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl AccountVault for SystemVault {
    fn load(&self) -> Result<AccountToken, VaultFault> {
        Err(VaultFault::Missing)
    }
    fn store(&self, _: &AccountToken) -> Result<(), VaultFault> {
        Err(VaultFault::Inaccessible)
    }
    fn clear(&self) -> Result<(), VaultFault> {
        Ok(())
    }
    fn present(&self) -> Result<bool, VaultFault> {
        Ok(false)
    }
}
