//! Transaction type enumeration

use crate::error::{DashError, Result};
use std::fmt;

/// Dash transaction types
///
/// Matches the transaction types defined in Dash Core's primitives/transaction.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum DashTxType {
    /// Normal transaction
    Normal = 0,
    /// Masternode provider registration
    ProviderRegister = 1,
    /// Masternode provider update service
    ProviderUpdateService = 2,
    /// Masternode provider update registrar
    ProviderUpdateRegistrar = 3,
    /// Masternode provider update revoke
    ProviderUpdateRevoke = 4,
    /// Coinbase special transaction
    Coinbase = 5,
    /// Quorum commitment
    QuorumCommitment = 6,
    /// Masternode hard fork signal
    MnhfSignal = 7,
    /// Asset lock transaction
    AssetLock = 8,
    /// Asset unlock transaction
    AssetUnlock = 9,
}

impl TryFrom<u16> for DashTxType {
    type Error = DashError;

    fn try_from(value: u16) -> Result<Self> {
        match value {
            0 => Ok(DashTxType::Normal),
            1 => Ok(DashTxType::ProviderRegister),
            2 => Ok(DashTxType::ProviderUpdateService),
            3 => Ok(DashTxType::ProviderUpdateRegistrar),
            4 => Ok(DashTxType::ProviderUpdateRevoke),
            5 => Ok(DashTxType::Coinbase),
            6 => Ok(DashTxType::QuorumCommitment),
            7 => Ok(DashTxType::MnhfSignal),
            8 => Ok(DashTxType::AssetLock),
            9 => Ok(DashTxType::AssetUnlock),
            _ => Err(DashError::UnknownTxType(value)),
        }
    }
}

impl fmt::Display for DashTxType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DashTxType::Normal => write!(f, "Normal"),
            DashTxType::ProviderRegister => write!(f, "ProviderRegister"),
            DashTxType::ProviderUpdateService => write!(f, "ProviderUpdateService"),
            DashTxType::ProviderUpdateRegistrar => write!(f, "ProviderUpdateRegistrar"),
            DashTxType::ProviderUpdateRevoke => write!(f, "ProviderUpdateRevoke"),
            DashTxType::Coinbase => write!(f, "Coinbase"),
            DashTxType::QuorumCommitment => write!(f, "QuorumCommitment"),
            DashTxType::MnhfSignal => write!(f, "MnhfSignal"),
            DashTxType::AssetLock => write!(f, "AssetLock"),
            DashTxType::AssetUnlock => write!(f, "AssetUnlock"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dash_tx_type_from_u16() {
        assert_eq!(DashTxType::try_from(0).unwrap(), DashTxType::Normal);
        assert_eq!(
            DashTxType::try_from(1).unwrap(),
            DashTxType::ProviderRegister
        );
        assert_eq!(DashTxType::try_from(5).unwrap(), DashTxType::Coinbase);
        assert_eq!(DashTxType::try_from(9).unwrap(), DashTxType::AssetUnlock);

        assert!(DashTxType::try_from(999).is_err());
        assert!(DashTxType::try_from(10).is_err());
    }

    #[test]
    fn test_dash_tx_type_to_u16() {
        assert_eq!(DashTxType::Normal as u16, 0);
        assert_eq!(DashTxType::Coinbase as u16, 5);
        assert_eq!(DashTxType::AssetLock as u16, 8);
    }

    #[test]
    fn test_dash_tx_type_display() {
        assert_eq!(format!("{}", DashTxType::Normal), "Normal");
        assert_eq!(format!("{}", DashTxType::Coinbase), "Coinbase");
        assert_eq!(
            format!("{}", DashTxType::ProviderRegister),
            "ProviderRegister"
        );
    }

    #[test]
    fn test_all_variants() {
        // Ensure all variants can round-trip
        for i in 0..=9 {
            let tx_type = DashTxType::try_from(i).unwrap();
            assert_eq!(tx_type as u16, i);
        }
    }
}
