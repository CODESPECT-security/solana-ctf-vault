#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use fuzz_helpers::*;
use solana_sdk::{
    instruction::Instruction,
    signature::Signer,
    transaction::Transaction,
};

/// Fuzzable input for deposit instruction
#[derive(Debug, Clone, Arbitrary)]
struct DepositFuzzInput {
    /// Amount to deposit (fuzzed)
    amount: u64,
    /// Initial user balance (for setup)
    initial_balance: u64,
    /// Token decimals (for setup)
    decimals: u8,
    /// Amount of yield/profit to add to vault before deposit (simulates yield accumulation)
    /// This tests the scenario where vault value grows between deposits
    yield_amount: u64,
    /// Whether to do an initial deposit first (to test subsequent deposit scenarios)
    do_initial_deposit: bool,
    /// Initial deposit amount (if do_initial_deposit is true)
    initial_deposit_amount: u64,
}

/// Execute a single fuzz iteration for the deposit instruction
async fn fuzz_deposit_once(input: DepositFuzzInput) -> Result<(), Box<dyn std::error::Error>> {
    // Constrain inputs to reasonable ranges to avoid trivial failures
    let amount = if input.amount == 0 {
        1 // Avoid zero amounts that are rejected by validation
    } else {
        input.amount
    };

    // Calculate total balance needed for user
    let mut total_needed = amount;
    if input.do_initial_deposit {
        let initial_deposit = if input.initial_deposit_amount == 0 {
            1
        } else {
            input.initial_deposit_amount
        };
        total_needed = total_needed.saturating_add(initial_deposit);
    }

    let initial_balance = input.initial_balance.saturating_add(total_needed);
    let decimals = input.decimals % 19; // Token decimals are typically 0-18
    let yield_amount = input.yield_amount % 1_000_000_000; // Cap yield to reasonable amount

    // Setup complete environment
    let (mut env, setup) = match setup_complete_environment(initial_balance, decimals).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Setup failed: {}", e);
            return Ok(()); // Skip this iteration if setup fails
        }
    };

    // SCENARIO 1: Simulate initial deposit if requested (to test subsequent deposits)
    if input.do_initial_deposit {
        let initial_deposit = if input.initial_deposit_amount == 0 {
            1
        } else {
            input.initial_deposit_amount.min(initial_balance / 2) // Don't use all balance
        };

        let accounts = vault_pda::accounts::Deposit {
            vault: setup.vault.vault,
            underlying_mint: setup.underlying.mint,
            vault_token_account: setup.vault.vault_token_account,
            share_mint: setup.vault.share_mint,
            vault_authority: setup.protocol.vault_authority,
            depositor_underlying_account: setup.user.underlying_token_account,
            depositor_share_account: setup.user.share_token_account,
            depositor: setup.user.owner.pubkey(),
            token_program: spl_token::id(),
        };

        let data = vault_pda::instruction::Deposit {
            amount: initial_deposit,
        }
        .data();

        let ix = Instruction {
            program_id: env.program_id,
            accounts: accounts.to_account_metas(None),
            data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&setup.user.owner.pubkey()),
            &[&setup.user.owner],
            env.context.last_blockhash,
        );

        // Execute initial deposit - if it fails, skip this iteration
        if env.context.banks_client.process_transaction(tx).await.is_err() {
            return Ok(()); // Skip if initial deposit fails
        }
    }

    // SCENARIO 2: Simulate yield accumulation (vault value increases)
    // This simulates profit/yield/rewards being added to the vault
    if yield_amount > 0 {
        // Mint yield tokens directly to the vault token account
        // This simulates external profit being added (e.g., from lending, farming, etc.)
        match mint_tokens_to_user(
            &mut env.context,
            &setup.underlying.mint,
            &setup.underlying.mint_authority,
            &setup.vault.vault_token_account,
            yield_amount,
        )
        .await
        {
            Ok(_) => {
                // Yield added successfully
            }
            Err(_) => {
                // If yield minting fails, continue without it
                // (this might happen with very large numbers)
            }
        }
    }

    // Get current vault state before deposit
    let vault_balance_before = get_token_balance(
        &mut env.context,
        &setup.vault.vault_token_account,
    ).await?;

    let share_supply_before = get_mint_supply(
        &mut env.context,
        &setup.vault.share_mint,
    ).await?;

    let user_balance_before = get_token_balance(
        &mut env.context,
        &setup.user.underlying_token_account,
    ).await?;

    let user_shares_before = get_token_balance(
        &mut env.context,
        &setup.user.share_token_account,
    ).await?;

    // Build deposit instruction
    let accounts = vault_pda::accounts::Deposit {
        vault: setup.vault.vault,
        underlying_mint: setup.underlying.mint,
        vault_token_account: setup.vault.vault_token_account,
        share_mint: setup.vault.share_mint,
        vault_authority: setup.protocol.vault_authority,
        depositor_underlying_account: setup.user.underlying_token_account,
        depositor_share_account: setup.user.share_token_account,
        depositor: setup.user.owner.pubkey(),
        token_program: spl_token::id(),
    };

    let data = vault_pda::instruction::Deposit { amount }.data();

    let ix = Instruction {
        program_id: env.program_id,
        accounts: accounts.to_account_metas(None),
        data,
    };

    // Execute deposit instruction
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&setup.user.owner.pubkey()),
        &[&setup.user.owner],
        env.context.last_blockhash,
    );

    let result = env.context.banks_client.process_transaction(tx).await;

    // Analyze results
    match result {
        Ok(_) => {
            // Transaction succeeded - verify invariants
            let vault_balance_after = get_token_balance(
                &mut env.context,
                &setup.vault.vault_token_account,
            ).await?;

            let share_supply_after = get_mint_supply(
                &mut env.context,
                &setup.vault.share_mint,
            ).await?;

            let user_balance_after = get_token_balance(
                &mut env.context,
                &setup.user.underlying_token_account,
            ).await?;

            let user_shares_after = get_token_balance(
                &mut env.context,
                &setup.user.share_token_account,
            ).await?;

            let shares_minted = user_shares_after - user_shares_before;

            // ========================================
            // MATHEMATICAL PROPERTY CHECKS
            // ========================================

            // PROPERTY 1: CONSERVATION OF TOKENS
            // Total tokens in system must be conserved (no creation/destruction)
            assert_eq!(
                vault_balance_before + user_balance_before,
                vault_balance_after + user_balance_after,
                "CRITICAL: Token conservation violated! Tokens created or destroyed. Before: vault={} user={}, After: vault={} user={}",
                vault_balance_before,
                user_balance_before,
                vault_balance_after,
                user_balance_after
            );

            // PROPERTY 2: BASIC BALANCE CHECKS
            // Vault should have received exactly the amount deposited
            assert_eq!(
                vault_balance_after,
                vault_balance_before + amount,
                "Vault balance should increase by exact deposit amount"
            );

            // User should have lost exactly the amount deposited
            assert_eq!(
                user_balance_after,
                user_balance_before - amount,
                "User balance should decrease by exact deposit amount"
            );

            // ========================================
            // SECURITY PROPERTY CHECKS
            // ========================================

            // SECURITY PROPERTY 1: SHARE VALUE PRESERVATION
            // The value per share should NEVER decrease after a deposit
            // This prevents share dilution attacks
            if share_supply_before > 0 {
                // Calculate value per share with high precision (using 1e9 multiplier)
                let precision = 1_000_000_000u128;
                let value_per_share_before =
                    (vault_balance_before as u128 * precision) / share_supply_before as u128;
                let value_per_share_after =
                    (vault_balance_after as u128 * precision) / share_supply_after as u128;

                assert!(
                    value_per_share_after >= value_per_share_before,
                    "CRITICAL VULNERABILITY: Share dilution attack! Value per share decreased from {} to {} (precision=1e9). \
                    This means existing shareholders lost value! \
                    Before: vault={} shares={}, After: vault={} shares={}, deposited={}",
                    value_per_share_before,
                    value_per_share_after,
                    vault_balance_before,
                    share_supply_before,
                    vault_balance_after,
                    share_supply_after,
                    amount
                );
            }

            // SECURITY PROPERTY 2: FAIRNESS - USER EXCHANGE RATE
            // User should receive fair value in shares (no more than they deserve)
            // Rounding should favor the vault/existing shareholders, not the depositor
            if share_supply_before > 0 {
                // Calculate maximum acceptable shares (with 0.1% tolerance for rounding)
                let expected_shares_precise = (amount as u128)
                    .saturating_mul(share_supply_before as u128)
                    .saturating_div(vault_balance_before as u128);

                // Allow up to 0.1% extra due to rounding, but no more
                let tolerance = expected_shares_precise / 1000; // 0.1%
                let max_acceptable_shares = expected_shares_precise + tolerance;

                assert!(
                    shares_minted as u128 <= max_acceptable_shares,
                    "VULNERABILITY: User received too many shares! Possible rounding exploit. \
                    Expected: {} shares, Got: {} shares, Max acceptable: {} (with 0.1% tolerance). \
                    Deposit: {}, Vault before: {}, Shares before: {}",
                    expected_shares_precise,
                    shares_minted,
                    max_acceptable_shares,
                    amount,
                    vault_balance_before,
                    share_supply_before
                );
            }

            // SECURITY PROPERTY 3: MONOTONICITY
            // Depositing non-zero amount should always result in non-zero shares
            assert!(
                shares_minted > 0,
                "User deposited {} tokens but received 0 shares - value extraction vulnerability!",
                amount
            );

            // SECURITY PROPERTY 4: REASONABLE BOUNDS
            // Shares minted should never exceed a reasonable multiple of amount deposited
            // For first deposit: shares = amount (ratio 1:1)
            // For subsequent: shares should be proportional
            if share_supply_before == 0 {
                assert_eq!(
                    shares_minted,
                    amount,
                    "First deposit should mint shares 1:1 with amount"
                );
            } else {
                // Shares should not be more than 2x the amount (sanity check)
                // In normal operation, shares ≈ amount * (share_supply / vault_balance)
                assert!(
                    shares_minted <= amount * 2,
                    "SUSPICIOUS: Minted {} shares for {} tokens deposit - seems excessive. \
                    Vault: {}, Share supply: {}",
                    shares_minted,
                    amount,
                    vault_balance_before,
                    share_supply_before
                );
            }

            // ========================================
            // CORRECTNESS CHECKS
            // ========================================

            // CORRECTNESS 1: Share supply should increase by exactly shares minted
            assert_eq!(
                share_supply_after,
                share_supply_before + shares_minted,
                "Share supply should increase by exactly the shares minted"
            );

            // CORRECTNESS 2: User share balance should increase by exactly shares minted
            assert_eq!(
                user_shares_after,
                user_shares_before + shares_minted,
                "User share balance should increase by exactly the shares minted"
            );

            // CORRECTNESS 3: Verify calculation matches expected formula
            if share_supply_before > 0 {
                let expected_shares = (amount as u128)
                    .saturating_mul(share_supply_before as u128)
                    .saturating_div(vault_balance_before as u128);

                // Allow for ±1 rounding difference
                let diff = if shares_minted as u128 > expected_shares {
                    shares_minted as u128 - expected_shares
                } else {
                    expected_shares - shares_minted as u128
                };

                assert!(
                    diff <= 1,
                    "Share calculation incorrect. Expected: {} (±1), Got: {}, Diff: {}",
                    expected_shares,
                    shares_minted,
                    diff
                );
            }

            // Calculate value per share for logging
            let value_per_share = if share_supply_after > 0 {
                (vault_balance_after as f64) / (share_supply_after as f64)
            } else {
                0.0
            };

            // Determine scenario type for logging
            let scenario = if share_supply_before == 0 {
                "FIRST_DEPOSIT"
            } else if yield_amount > 0 {
                "YIELD_GROWTH"
            } else if input.do_initial_deposit {
                "SUBSEQUENT"
            } else {
                "BASIC"
            };

            println!(
                "✓ PASS [{:13}] - deposit={}, shares={}, vault: {}→{} (+yield: {}), \
                value/share: {:.6}, all invariants ✓",
                scenario,
                amount,
                shares_minted,
                vault_balance_before,
                vault_balance_after,
                yield_amount,
                value_per_share
            );
        }
        Err(e) => {
            // Transaction failed - this might be expected for some inputs
            println!(
                "✗ Deposit failed: amount={}, error={:?}",
                amount,
                e
            );

            // Some failures are expected and acceptable:
            // - Insufficient balance
            // - Amount too small resulting in 0 shares
            // - Arithmetic overflow

            // However, we should panic on unexpected errors like:
            // - Program panic
            // - Unexpected account validation failures

            let error_string = format!("{:?}", e);

            // List of acceptable error patterns
            let acceptable_errors = [
                "InsufficientFunds",
                "InvalidAmount",
                "InsufficientShares",
                "MathOverflow",
            ];

            let is_acceptable = acceptable_errors.iter().any(|&pattern| {
                error_string.contains(pattern)
            });

            if !is_acceptable {
                panic!(
                    "Unexpected error during deposit: {:?}\nInput: {:?}",
                    e, input
                );
            }
        }
    }

    Ok(())
}

fuzz_target!(|input: DepositFuzzInput| {
    // Run the async fuzz test
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        if let Err(e) = fuzz_deposit_once(input).await {
            eprintln!("Fuzz iteration failed: {}", e);
        }
    });
});
