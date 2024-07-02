use borsh::{BorshDeserialize, BorshSerialize};
use jito_restaking_sanitization::assert_with_msg;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    result::{VaultCoreError, VaultCoreResult},
    AccountType,
};

#[derive(Debug, Clone, Copy, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct Vault {
    /// The account type
    account_type: AccountType,

    /// The base account of the LRT
    base: Pubkey,

    /// Mint of the LRT token
    lrt_mint: Pubkey,

    /// Mint of the token that is supported by the LRT
    supported_mint: Pubkey,

    /// Vault admin
    admin: Pubkey,

    /// Delegation admin
    delegation_admin: Pubkey,

    /// Fee account owner
    fee_owner: Pubkey,

    /// Optional mint signer
    mint_burn_authority: Pubkey,

    /// Max capacity of tokens in the vault
    capacity: u64,

    /// The index of the vault in the vault list
    vault_index: u64,

    /// The total number of LRT in circulation
    lrt_supply: u64,

    /// The total number of tokens deposited
    tokens_deposited: u64,

    /// The deposit fee in basis points
    deposit_fee_bps: u16,

    /// The withdrawal fee in basis points
    withdrawal_fee_bps: u16,

    /// Reserved space
    reserved: [u8; 1024],

    /// The bump seed for the PDA
    bump: u8,
}

impl Vault {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lrt_mint: Pubkey,
        supported_mint: Pubkey,
        admin: Pubkey,
        lrt_index: u64,
        base: Pubkey,
        deposit_fee_bps: u16,
        withdrawal_fee_bps: u16,
        bump: u8,
    ) -> Self {
        Self {
            account_type: AccountType::Vault,
            base,
            lrt_mint,
            supported_mint,
            admin,
            delegation_admin: admin,
            fee_owner: admin,
            mint_burn_authority: Pubkey::default(),
            capacity: u64::MAX,
            vault_index: lrt_index,
            lrt_supply: 0,
            tokens_deposited: 0,
            deposit_fee_bps,
            withdrawal_fee_bps,
            reserved: [0; 1024],
            bump,
        }
    }

    pub const fn lrt_mint(&self) -> Pubkey {
        self.lrt_mint
    }

    pub const fn supported_mint(&self) -> Pubkey {
        self.supported_mint
    }

    pub const fn base(&self) -> Pubkey {
        self.base
    }

    pub const fn fee_owner(&self) -> Pubkey {
        self.fee_owner
    }

    pub const fn deposit_fee_bps(&self) -> u16 {
        self.deposit_fee_bps
    }

    pub const fn withdrawal_fee_bps(&self) -> u16 {
        self.withdrawal_fee_bps
    }

    pub fn mint_burn_authority(&self) -> Option<Pubkey> {
        if self.mint_burn_authority != Pubkey::default() {
            Some(self.mint_burn_authority)
        } else {
            None
        }
    }

    pub fn set_tokens_deposited(&mut self, tokens_deposited: u64) {
        self.tokens_deposited = tokens_deposited;
    }

    /// Deposit tokens into the vault
    pub fn deposit_and_mint_with_capacity_check(&mut self, amount: u64) -> VaultCoreResult<u64> {
        // the number of tokens to mint is the pro-rata amount of the total tokens deposited and the LRT supply
        let num_tokens_to_mint = if self.tokens_deposited == 0 {
            amount
        } else {
            amount
                .checked_mul(self.lrt_supply)
                .ok_or(VaultCoreError::DepositOverflow)?
                .checked_div(self.tokens_deposited)
                .ok_or(VaultCoreError::DepositOverflow)?
        };

        // deposit tokens + check against capacity
        let total_post_deposit = self
            .tokens_deposited
            .checked_add(amount)
            .ok_or(VaultCoreError::DepositOverflow)?;
        if total_post_deposit > self.capacity {
            return Err(VaultCoreError::DepositExceedsCapacity);
        }

        let lrt_supply = self
            .lrt_supply
            .checked_add(num_tokens_to_mint)
            .ok_or(VaultCoreError::DepositOverflow)?;

        self.lrt_supply = lrt_supply;
        self.tokens_deposited = total_post_deposit;

        Ok(num_tokens_to_mint)
    }

    pub fn calculate_deposit_fee(&self, lrt_amount: u64) -> VaultCoreResult<u64> {
        let fee = lrt_amount
            .checked_mul(self.deposit_fee_bps as u64)
            .ok_or(VaultCoreError::FeeCalculationOverflow)?
            .checked_div(10_000)
            .unwrap();
        Ok(fee)
    }

    pub fn calculate_withdraw_fee(&self, lrt_amount: u64) -> VaultCoreResult<u64> {
        let fee = lrt_amount
            .checked_mul(self.withdrawal_fee_bps as u64)
            .ok_or(VaultCoreError::FeeCalculationOverflow)?
            .checked_div(10_000)
            .unwrap();
        Ok(fee)
    }

    pub const fn tokens_deposited(&self) -> u64 {
        self.tokens_deposited
    }

    pub fn set_lrt_supply(&mut self, lrt_supply: u64) {
        self.lrt_supply = lrt_supply;
    }

    pub fn increment_lrt_supply(&mut self, amount: u64) -> Option<u64> {
        self.lrt_supply = self.lrt_supply.checked_add(amount)?;
        Some(self.lrt_supply)
    }

    pub const fn lrt_supply(&self) -> u64 {
        self.lrt_supply
    }

    pub const fn bump(&self) -> u8 {
        self.bump
    }

    pub fn is_struct_valid(&self) -> bool {
        self.account_type == AccountType::Vault
    }

    pub const fn capacity(&self) -> u64 {
        self.capacity
    }

    pub fn set_capacity(&mut self, capacity: u64) {
        self.capacity = capacity;
    }

    pub const fn vault_index(&self) -> u64 {
        self.vault_index
    }

    pub const fn admin(&self) -> Pubkey {
        self.admin
    }

    pub fn set_admin(&mut self, admin: Pubkey) {
        self.admin = admin;
    }

    pub fn set_delegation_admin(&mut self, delegation_admin: Pubkey) {
        self.delegation_admin = delegation_admin;
    }

    pub const fn delegation_admin(&self) -> Pubkey {
        self.delegation_admin
    }

    pub fn seeds(base: &Pubkey) -> Vec<Vec<u8>> {
        vec![b"vault".as_ref().to_vec(), base.to_bytes().to_vec()]
    }

    pub fn find_program_address(program_id: &Pubkey, base: &Pubkey) -> (Pubkey, u8, Vec<Vec<u8>>) {
        let seeds = Self::seeds(base);
        let seeds_iter: Vec<_> = seeds.iter().map(|s| s.as_slice()).collect();
        let (pda, bump) = Pubkey::find_program_address(&seeds_iter, program_id);
        (pda, bump, seeds)
    }

    pub fn deserialize_checked(
        program_id: &Pubkey,
        account: &AccountInfo,
    ) -> Result<Self, ProgramError> {
        assert_with_msg(
            !account.data_is_empty(),
            ProgramError::UninitializedAccount,
            "LRT account is not initialized",
        )?;
        assert_with_msg(
            account.owner == program_id,
            ProgramError::IllegalOwner,
            "LRT account not owned by the correct program",
        )?;

        // The AvsState shall be properly deserialized and valid struct
        let state = Self::deserialize(&mut account.data.borrow_mut().as_ref())?;
        assert_with_msg(
            state.is_struct_valid(),
            ProgramError::InvalidAccountData,
            "LRT account is invalid",
        )?;

        // The AvsState shall be at the correct PDA as defined by the seeds and bump
        let mut seeds = Self::seeds(&state.base);
        seeds.push(vec![state.bump]);
        let seeds_iter: Vec<_> = seeds.iter().map(|s| s.as_ref()).collect();
        let expected_pubkey = Pubkey::create_program_address(&seeds_iter, program_id)?;

        assert_with_msg(
            expected_pubkey == *account.key,
            ProgramError::InvalidAccountData,
            "Vault account is not at the correct PDA",
        )?;

        Ok(state)
    }
}

pub struct SanitizedVault<'a, 'info> {
    account: &'a AccountInfo<'info>,
    vault: Vault,
}

impl<'a, 'info> SanitizedVault<'a, 'info> {
    /// Sanitizes the AvsAccount so it can be used in a safe context
    pub fn sanitize(
        program_id: &Pubkey,
        account: &'a AccountInfo<'info>,
        expect_writable: bool,
    ) -> Result<SanitizedVault<'a, 'info>, ProgramError> {
        if expect_writable {
            assert_with_msg(
                account.is_writable,
                ProgramError::InvalidAccountData,
                "Invalid writable flag for vault",
            )?;
        }
        let vault = Vault::deserialize_checked(program_id, account)?;

        Ok(SanitizedVault { account, vault })
    }

    pub const fn account(&self) -> &AccountInfo<'info> {
        self.account
    }

    pub const fn vault(&self) -> &Vault {
        &self.vault
    }

    pub fn vault_mut(&mut self) -> &mut Vault {
        &mut self.vault
    }

    pub fn save(&self) -> ProgramResult {
        borsh::to_writer(&mut self.account.data.borrow_mut()[..], &self.vault)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use solana_program::pubkey::Pubkey;

    use crate::vault::{Vault, VaultCoreError};

    #[test]
    fn test_deposit_ratio_simple_ok() {
        let mut vault = Vault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
            Pubkey::new_unique(),
            0,
            0,
            0,
        );
        let num_minted = vault.deposit_and_mint_with_capacity_check(100).unwrap();
        assert_eq!(num_minted, 100);

        assert_eq!(vault.tokens_deposited(), 100);
        assert_eq!(vault.lrt_supply(), 100);
    }

    #[test]
    fn test_deposit_ratio_after_slashed_ok() {
        let mut vault = Vault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
            Pubkey::new_unique(),
            0,
            0,
            0,
        );
        vault.set_tokens_deposited(90);
        vault.set_lrt_supply(100);

        let num_minted = vault.deposit_and_mint_with_capacity_check(100).unwrap();
        assert_eq!(num_minted, 111);

        assert_eq!(vault.tokens_deposited(), 190);
        assert_eq!(vault.lrt_supply(), 211);
    }

    #[test]
    fn test_deposit_capacity_exceeded_fails() {
        let mut vault = Vault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
            Pubkey::new_unique(),
            0,
            0,
            0,
        );
        vault.set_capacity(100);

        assert_eq!(
            vault.deposit_and_mint_with_capacity_check(101),
            Err(VaultCoreError::DepositExceedsCapacity)
        );
    }

    #[test]
    fn test_deposit_capacity_ok() {
        let mut vault = Vault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
            Pubkey::new_unique(),
            0,
            0,
            0,
        );
        vault.set_capacity(100);

        vault.deposit_and_mint_with_capacity_check(50).unwrap();
        vault.deposit_and_mint_with_capacity_check(50).unwrap();
        assert_eq!(
            vault.deposit_and_mint_with_capacity_check(1),
            Err(VaultCoreError::DepositExceedsCapacity)
        );
    }
}
