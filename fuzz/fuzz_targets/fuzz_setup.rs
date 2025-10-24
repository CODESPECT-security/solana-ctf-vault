use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_lang::AccountDeserialize;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_token::instruction as token_instruction;
use vault_pda::state::{ProtocolState, Vault, VaultAuthority};

// Re-export for convenience
pub use solana_program_test::ProgramTestContext;

// Custom error type for fuzzing
pub type FuzzResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Test environment with program loaded
pub struct FuzzTestEnv {
    pub program_id: Pubkey,
    pub context: ProgramTestContext,
}

/// Protocol-level accounts (protocol state and vault authority)
#[derive(Debug)]
pub struct ProtocolAccounts {
    pub protocol_state: Pubkey,
    pub vault_authority: Pubkey,
    pub owner: Pubkey,
    pub owner_keypair: Keypair,
}

impl Clone for ProtocolAccounts {
    fn clone(&self) -> Self {
        panic!("ProtocolAccounts cannot be cloned due to Keypair");
    }
}

/// Underlying token mint accounts
#[derive(Debug)]
pub struct UnderlyingMintAccounts {
    pub mint: Pubkey,
    pub mint_authority: Keypair,
    pub decimals: u8,
}

impl Clone for UnderlyingMintAccounts {
    fn clone(&self) -> Self {
        // We can't clone Keypair, so we need to handle this differently
        // For fuzzing purposes, we'll panic if someone tries to clone
        panic!("UnderlyingMintAccounts cannot be cloned due to Keypair");
    }
}

/// Vault-specific accounts
#[derive(Debug, Clone)]
pub struct VaultAccounts {
    pub vault: Pubkey,
    pub vault_token_account: Pubkey,
    pub share_mint: Pubkey,
    pub underlying_mint: Pubkey,
}

/// User token accounts for interacting with vault
#[derive(Debug)]
pub struct UserAccounts {
    pub owner: Keypair,
    pub underlying_token_account: Pubkey,
    pub share_token_account: Pubkey,
}

impl Clone for UserAccounts {
    fn clone(&self) -> Self {
        panic!("UserAccounts cannot be cloned due to Keypair");
    }
}

/// Complete setup with all accounts
pub struct CompleteSetup {
    pub protocol: ProtocolAccounts,
    pub underlying: UnderlyingMintAccounts,
    pub vault: VaultAccounts,
    pub user: UserAccounts,
}

// ============================================================================
// Core Setup Functions
// ============================================================================

/// Creates the basic program test environment with vault program loaded
pub async fn setup_program_test() -> FuzzTestEnv {
    let program_id = vault_pda::id();
    let program_test = ProgramTest::default();

    let context = program_test.start_with_context().await;

    FuzzTestEnv {
        program_id,
        context,
    }
}

/// Initializes the protocol (calls initialize instruction)
pub async fn setup_protocol(
    context: &mut ProgramTestContext,
    program_id: &Pubkey,
) -> FuzzResult<ProtocolAccounts> {
    let owner_keypair = Keypair::new();
    let owner = owner_keypair.pubkey();

    // Derive PDAs
    let (protocol_state, _) = derive_protocol_state_pda(program_id);
    let (vault_authority, _) = derive_vault_authority_pda(program_id);

    // Fund the owner account
    let rent = context.banks_client.get_rent().await?;
    let lamports = rent.minimum_balance(0) + 1_000_000_000; // 1 SOL

    let ix = solana_sdk::system_instruction::transfer(
        &context.payer.pubkey(),
        &owner,
        lamports,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await?;

    // Build initialize instruction
    let accounts = vault_pda::accounts::Initialize {
        protocol_state,
        vault_authority,
        owner,
        payer: owner,
        system_program: solana_sdk::system_program::ID,
    };

    let data = vault_pda::instruction::Initialize {}.data();

    let ix = Instruction {
        program_id: *program_id,
        accounts: accounts.to_account_metas(None),
        data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&owner),
        &[&owner_keypair],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await?;

    Ok(ProtocolAccounts {
        protocol_state,
        vault_authority,
        owner,
        owner_keypair,
    })
}

/// Creates a new SPL token mint to serve as underlying asset
pub async fn setup_underlying_mint(
    context: &mut ProgramTestContext,
    decimals: u8,
) -> FuzzResult<UnderlyingMintAccounts> {
    let mint_authority = Keypair::new();
    let mint_keypair = Keypair::new();
    let mint = mint_keypair.pubkey();

    let rent = context.banks_client.get_rent().await?;
    let mint_len = 82; // Size of Mint account in SPL Token program
    let mint_rent = rent.minimum_balance(mint_len);

    // Create mint account
    let create_account_ix = solana_sdk::system_instruction::create_account(
        &context.payer.pubkey(),
        &mint,
        mint_rent,
        mint_len as u64,
        &spl_token::id(),
    );

    // Initialize mint
    let init_mint_ix = token_instruction::initialize_mint(
        &spl_token::id(),
        &mint,
        &mint_authority.pubkey(),
        None,
        decimals,
    )?;

    let tx = Transaction::new_signed_with_payer(
        &[create_account_ix, init_mint_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer, &mint_keypair],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await?;

    Ok(UnderlyingMintAccounts {
        mint,
        mint_authority,
        decimals,
    })
}

/// Initializes a vault for a given underlying mint
pub async fn setup_vault(
    context: &mut ProgramTestContext,
    program_id: &Pubkey,
    vault_authority: &Pubkey,
    underlying_mint: &Pubkey,
    payer: &Keypair,
) -> FuzzResult<VaultAccounts> {
    // Derive PDAs
    let (vault, _) = derive_vault_pda(program_id, underlying_mint);
    let (share_mint, _) = derive_share_mint_pda(program_id, &vault);
    let (vault_token_account, _) = derive_vault_token_account_pda(program_id, &vault);

    // Build initialize_vault instruction
    let accounts = vault_pda::accounts::InitializeVault {
        vault,
        underlying_mint: *underlying_mint,
        vault_token_account,
        share_mint,
        vault_authority: *vault_authority,
        payer: payer.pubkey(),
        system_program: solana_sdk::system_program::ID,
        token_program: spl_token::id(),
    };

    let data = vault_pda::instruction::InitializeVault {}.data();

    let ix = Instruction {
        program_id: *program_id,
        accounts: accounts.to_account_metas(None),
        data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await?;

    Ok(VaultAccounts {
        vault,
        vault_token_account,
        share_mint,
        underlying_mint: *underlying_mint,
    })
}

/// Creates token accounts for a user (for deposits/redeems)
pub async fn setup_user_accounts(
    context: &mut ProgramTestContext,
    underlying_mint: &Pubkey,
    share_mint: &Pubkey,
) -> FuzzResult<UserAccounts> {
    let owner = Keypair::new();

    // Fund the owner account
    let rent = context.banks_client.get_rent().await?;
    let lamports = rent.minimum_balance(0) + 1_000_000_000; // 1 SOL

    let ix = solana_sdk::system_instruction::transfer(
        &context.payer.pubkey(),
        &owner.pubkey(),
        lamports,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await?;

    let account_len = 165; // Size of Token account in SPL Token program

    // Create underlying token account
    let underlying_token_account = Keypair::new();
    let create_underlying_ix = solana_sdk::system_instruction::create_account(
        &context.payer.pubkey(),
        &underlying_token_account.pubkey(),
        rent.minimum_balance(account_len),
        account_len as u64,
        &spl_token::id(),
    );

    let init_underlying_ix = token_instruction::initialize_account(
        &spl_token::id(),
        &underlying_token_account.pubkey(),
        underlying_mint,
        &owner.pubkey(),
    )?;

    // Create share token account
    let share_token_account = Keypair::new();
    let create_share_ix = solana_sdk::system_instruction::create_account(
        &context.payer.pubkey(),
        &share_token_account.pubkey(),
        rent.minimum_balance(account_len),
        account_len as u64,
        &spl_token::id(),
    );

    let init_share_ix = token_instruction::initialize_account(
        &spl_token::id(),
        &share_token_account.pubkey(),
        share_mint,
        &owner.pubkey(),
    )?;

    let tx = Transaction::new_signed_with_payer(
        &[
            create_underlying_ix,
            init_underlying_ix,
            create_share_ix,
            init_share_ix,
        ],
        Some(&context.payer.pubkey()),
        &[
            &context.payer,
            &underlying_token_account,
            &share_token_account,
        ],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await?;

    Ok(UserAccounts {
        owner,
        underlying_token_account: underlying_token_account.pubkey(),
        share_token_account: share_token_account.pubkey(),
    })
}

/// Mints tokens to a user's underlying token account
pub async fn mint_tokens_to_user(
    context: &mut ProgramTestContext,
    mint: &Pubkey,
    mint_authority: &Keypair,
    destination: &Pubkey,
    amount: u64,
) -> FuzzResult<()> {
    let mint_to_ix = token_instruction::mint_to(
        &spl_token::id(),
        mint,
        destination,
        &mint_authority.pubkey(),
        &[],
        amount,
    )?;

    let tx = Transaction::new_signed_with_payer(
        &[mint_to_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer, mint_authority],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await?;

    Ok(())
}

/// Sets up everything: protocol + underlying mint + vault + user with tokens
pub async fn setup_complete_environment(
    initial_user_balance: u64,
    decimals: u8,
) -> FuzzResult<(FuzzTestEnv, CompleteSetup)> {
    let mut env = setup_program_test().await;

    // Setup protocol
    let protocol = setup_protocol(&mut env.context, &env.program_id).await?;

    // Setup underlying mint
    let underlying = setup_underlying_mint(&mut env.context, decimals).await?;

    // Setup vault
    let vault = setup_vault(
        &mut env.context,
        &env.program_id,
        &protocol.vault_authority,
        &underlying.mint,
        &protocol.owner_keypair,
    )
    .await?;

    // Setup user accounts
    let user = setup_user_accounts(
        &mut env.context,
        &underlying.mint,
        &vault.share_mint,
    )
    .await?;

    // Mint initial tokens to user
    if initial_user_balance > 0 {
        mint_tokens_to_user(
            &mut env.context,
            &underlying.mint,
            &underlying.mint_authority,
            &user.underlying_token_account,
            initial_user_balance,
        )
        .await?;
    }

    let setup = CompleteSetup {
        protocol,
        underlying,
        vault,
        user,
    };

    Ok((env, setup))
}

// ============================================================================
// PDA Derivation Helpers
// ============================================================================

/// Derive protocol state PDA
pub fn derive_protocol_state_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"protocol_state"], program_id)
}

/// Derive vault authority PDA
pub fn derive_vault_authority_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault_authority"], program_id)
}

/// Derive vault PDA
pub fn derive_vault_pda(program_id: &Pubkey, underlying_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", underlying_mint.as_ref()], program_id)
}

/// Derive share mint PDA
pub fn derive_share_mint_pda(program_id: &Pubkey, vault: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"share_mint", vault.as_ref()], program_id)
}

/// Derive vault token account PDA
pub fn derive_vault_token_account_pda(program_id: &Pubkey, vault: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault_token_account", vault.as_ref()], program_id)
}

// ============================================================================
// Account State Verification Helpers
// ============================================================================

/// Fetch and return vault state
pub async fn get_vault_state(
    context: &mut ProgramTestContext,
    vault: &Pubkey,
) -> FuzzResult<Vault> {
    let account = context
        .banks_client
        .get_account(*vault)
        .await?
        .ok_or("Vault account not found")?;

    let vault_data = Vault::try_deserialize(&mut account.data.as_ref())?;
    Ok(vault_data)
}

/// Get token account balance
pub async fn get_token_balance(
    context: &mut ProgramTestContext,
    account: &Pubkey,
) -> FuzzResult<u64> {
    let account_data = context
        .banks_client
        .get_account(*account)
        .await?
        .ok_or("Token account not found")?;

    // Manually parse amount from token account data
    // Token account structure: amount is at offset 64 (u64)
    if account_data.data.len() < 72 {
        return Err("Invalid token account data".into());
    }

    let amount = u64::from_le_bytes(
        account_data.data[64..72]
            .try_into()
            .map_err(|_| "Failed to parse amount")?
    );

    Ok(amount)
}

/// Get mint supply
pub async fn get_mint_supply(
    context: &mut ProgramTestContext,
    mint: &Pubkey,
) -> FuzzResult<u64> {
    let account = context
        .banks_client
        .get_account(*mint)
        .await?
        .ok_or("Mint account not found")?;

    // Manually parse supply from mint account data
    // Mint account structure: supply is at offset 36 (u64)
    if account.data.len() < 44 {
        return Err("Invalid mint account data".into());
    }

    let supply = u64::from_le_bytes(
        account.data[36..44]
            .try_into()
            .map_err(|_| "Failed to parse supply")?
    );

    Ok(supply)
}

/// Get protocol state
pub async fn get_protocol_state(
    context: &mut ProgramTestContext,
    protocol_state: &Pubkey,
) -> FuzzResult<ProtocolState> {
    let account = context
        .banks_client
        .get_account(*protocol_state)
        .await?
        .ok_or("Protocol state account not found")?;

    let state = ProtocolState::try_deserialize(&mut account.data.as_ref())?;
    Ok(state)
}

/// Get vault authority
pub async fn get_vault_authority(
    context: &mut ProgramTestContext,
    vault_authority: &Pubkey,
) -> FuzzResult<VaultAuthority> {
    let account = context
        .banks_client
        .get_account(*vault_authority)
        .await?
        .ok_or("Vault authority account not found")?;

    let authority = VaultAuthority::try_deserialize(&mut account.data.as_ref())?;
    Ok(authority)
}
