import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { VaultPda } from "../target/types/vault_pda";
import {
  createMint,
  getMinimumBalanceForRentExemptMint,
  TOKEN_PROGRAM_ID,
  MINT_SIZE,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import { expect } from "chai";

describe("vault-pda", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.vaultPda as Program<VaultPda>;
  const payer = provider.wallet as anchor.Wallet;

  // PDAs
  let protocolStatePda: anchor.web3.PublicKey;
  let vaultAuthorityPda: anchor.web3.PublicKey;
  let vaultPda: anchor.web3.PublicKey;
  let shareMintPda: anchor.web3.PublicKey;
  let vaultTokenAccountPda: anchor.web3.PublicKey;

  // Mints
  let underlyingMint: anchor.web3.PublicKey;

  // Token accounts
  let depositorUnderlyingAccount: anchor.web3.PublicKey;
  let depositorShareAccount: anchor.web3.PublicKey;

  it("Initializes the protocol", async () => {
    // Derive PDAs
    [protocolStatePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("protocol_state")],
      program.programId
    );

    [vaultAuthorityPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault_authority")],
      program.programId
    );

    console.log("Protocol State PDA:", protocolStatePda.toString());
    console.log("Vault Authority PDA:", vaultAuthorityPda.toString());

    // Call initialize instruction
    const tx = await program.methods
      .initialize()
      .accounts({
        protocolState: protocolStatePda,
        vaultAuthority: vaultAuthorityPda,
        owner: payer.publicKey,
        payer: payer.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Initialize transaction signature:", tx);

    // Fetch and verify protocol state
    const protocolStateAccount = await program.account.protocolState.fetch(
      protocolStatePda
    );
    expect(protocolStateAccount.owner.toString()).to.equal(
      payer.publicKey.toString()
    );
    console.log("Protocol owner:", protocolStateAccount.owner.toString());

    // Fetch and verify vault authority
    const vaultAuthorityAccount = await program.account.vaultAuthority.fetch(
      vaultAuthorityPda
    );
    expect(vaultAuthorityAccount.bump).to.be.greaterThan(0);
    console.log("Vault authority bump:", vaultAuthorityAccount.bump);
  });

  it("Creates an underlying token mint", async () => {
    // Create a new mint for the underlying asset
    underlyingMint = await createMint(
      provider.connection,
      payer.payer,
      payer.publicKey, // mint authority
      null, // freeze authority
      6 // decimals (USDC-like)
    );

    console.log("Underlying mint created:", underlyingMint.toString());

    // Verify the mint was created
    const mintInfo = await provider.connection.getAccountInfo(underlyingMint);
    expect(mintInfo).to.not.be.null;
  });

  it("Initializes a vault for the underlying token", async () => {
    // Derive vault PDA
    [vaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), underlyingMint.toBuffer()],
      program.programId
    );

    // Derive share mint PDA
    [shareMintPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("share_mint"), vaultPda.toBuffer()],
      program.programId
    );

    // Derive vault token account PDA
    [vaultTokenAccountPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault_token_account"), vaultPda.toBuffer()],
      program.programId
    );

    console.log("Vault PDA:", vaultPda.toString());
    console.log("Share Mint PDA:", shareMintPda.toString());
    console.log("Vault Token Account PDA:", vaultTokenAccountPda.toString());

    // Call initialize_vault instruction
    const tx = await program.methods
      .initializeVault()
      .accounts({
        vault: vaultPda,
        underlyingMint: underlyingMint,
        vaultTokenAccount: vaultTokenAccountPda,
        shareMint: shareMintPda,
        vaultAuthority: vaultAuthorityPda,
        payer: payer.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    console.log("Initialize vault transaction signature:", tx);

    // Fetch and verify vault account
    const vaultAccount = await program.account.vault.fetch(vaultPda);
    expect(vaultAccount.shareMint.toString()).to.equal(shareMintPda.toString());
    expect(vaultAccount.underlyingMint.toString()).to.equal(
      underlyingMint.toString()
    );
    expect(vaultAccount.bump).to.be.greaterThan(0);

    console.log("Vault created successfully!");
    console.log("  Share Mint:", vaultAccount.shareMint.toString());
    console.log("  Underlying Mint:", vaultAccount.underlyingMint.toString());
    console.log("  Bump:", vaultAccount.bump);

    // Verify share mint was created with correct authority
    const shareMintInfo = await provider.connection.getAccountInfo(shareMintPda);
    expect(shareMintInfo).to.not.be.null;
    console.log("Share mint created successfully!");
  });

  it("Makes first deposit (1:1 share calculation)", async () => {
    const depositAmount = 1000_000; // 1 token (6 decimals)

    // Create depositor's token accounts
    const depositorUnderlyingAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer.payer,
      underlyingMint,
      payer.publicKey
    );
    depositorUnderlyingAccount = depositorUnderlyingAta.address;

    const depositorShareAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer.payer,
      shareMintPda,
      payer.publicKey
    );
    depositorShareAccount = depositorShareAta.address;

    // Mint some underlying tokens to the depositor
    await mintTo(
      provider.connection,
      payer.payer,
      underlyingMint,
      depositorUnderlyingAccount,
      payer.publicKey,
      depositAmount * 10 // Mint 10x what we'll deposit
    );

    console.log("Depositor underlying account:", depositorUnderlyingAccount.toString());
    console.log("Depositor share account:", depositorShareAccount.toString());

    // Call deposit instruction
    const tx = await program.methods
      .deposit(new anchor.BN(depositAmount))
      .accounts({
        vault: vaultPda,
        underlyingMint: underlyingMint,
        vaultTokenAccount: vaultTokenAccountPda,
        shareMint: shareMintPda,
        vaultAuthority: vaultAuthorityPda,
        depositorUnderlyingAccount: depositorUnderlyingAccount,
        depositorShareAccount: depositorShareAccount,
        depositor: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    console.log("Deposit transaction signature:", tx);

    // Verify balances
    const vaultTokenAccountInfo = await getAccount(
      provider.connection,
      vaultTokenAccountPda
    );
    expect(vaultTokenAccountInfo.amount.toString()).to.equal(depositAmount.toString());

    const depositorShareAccountInfo = await getAccount(
      provider.connection,
      depositorShareAccount
    );
    // First deposit: shares minted 1:1
    expect(depositorShareAccountInfo.amount.toString()).to.equal(depositAmount.toString());

    console.log("First deposit successful!");
    console.log("  Vault balance:", vaultTokenAccountInfo.amount.toString());
    console.log("  Shares minted:", depositorShareAccountInfo.amount.toString());
  });

  it("Makes second deposit (proportional share calculation)", async () => {
    const depositAmount = 500_000; // 0.5 tokens (6 decimals)

    // Get vault balance before deposit
    const vaultTokenAccountBefore = await getAccount(
      provider.connection,
      vaultTokenAccountPda
    );
    const vaultBalanceBefore = Number(vaultTokenAccountBefore.amount);

    const depositorShareAccountBefore = await getAccount(
      provider.connection,
      depositorShareAccount
    );
    const sharesBefore = Number(depositorShareAccountBefore.amount);

    // Call deposit instruction
    const tx = await program.methods
      .deposit(new anchor.BN(depositAmount))
      .accounts({
        vault: vaultPda,
        underlyingMint: underlyingMint,
        vaultTokenAccount: vaultTokenAccountPda,
        shareMint: shareMintPda,
        vaultAuthority: vaultAuthorityPda,
        depositorUnderlyingAccount: depositorUnderlyingAccount,
        depositorShareAccount: depositorShareAccount,
        depositor: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    console.log("Second deposit transaction signature:", tx);

    // Verify balances
    const vaultTokenAccountAfter = await getAccount(
      provider.connection,
      vaultTokenAccountPda
    );
    const vaultBalanceAfter = Number(vaultTokenAccountAfter.amount);
    expect(vaultBalanceAfter).to.equal(vaultBalanceBefore + depositAmount);

    const depositorShareAccountAfter = await getAccount(
      provider.connection,
      depositorShareAccount
    );
    const sharesAfter = Number(depositorShareAccountAfter.amount);

    // Calculate expected shares: (depositAmount * sharesBefore) / vaultBalanceBefore
    const expectedShares = Math.floor((depositAmount * sharesBefore) / vaultBalanceBefore);
    const actualSharesMinted = sharesAfter - sharesBefore;

    expect(actualSharesMinted).to.equal(expectedShares);

    console.log("Second deposit successful!");
    console.log("  Deposited:", depositAmount);
    console.log("  Vault balance before:", vaultBalanceBefore);
    console.log("  Total shares before:", sharesBefore);
    console.log("  Expected shares minted:", expectedShares);
    console.log("  Actual shares minted:", actualSharesMinted);
    console.log("  New vault balance:", vaultBalanceAfter);
    console.log("  New total shares:", sharesAfter);
  });

  it("Redeems partial shares", async () => {
    const sharesToRedeem = 500_000; // Redeem 500k shares

    // Get balances before redeem
    const vaultTokenAccountBefore = await getAccount(
      provider.connection,
      vaultTokenAccountPda
    );
    const vaultBalanceBefore = Number(vaultTokenAccountBefore.amount);

    const depositorUnderlyingAccountBefore = await getAccount(
      provider.connection,
      depositorUnderlyingAccount
    );
    const underlyingBefore = Number(depositorUnderlyingAccountBefore.amount);

    const depositorShareAccountBefore = await getAccount(
      provider.connection,
      depositorShareAccount
    );
    const sharesBefore = Number(depositorShareAccountBefore.amount);

    // Call redeem instruction
    const tx = await program.methods
      .redeem(new anchor.BN(sharesToRedeem))
      .accounts({
        vault: vaultPda,
        underlyingMint: underlyingMint,
        vaultTokenAccount: vaultTokenAccountPda,
        shareMint: shareMintPda,
        vaultAuthority: vaultAuthorityPda,
        redeemerUnderlyingAccount: depositorUnderlyingAccount,
        redeemerShareAccount: depositorShareAccount,
        redeemer: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    console.log("Redeem transaction signature:", tx);

    // Verify balances after redeem
    const vaultTokenAccountAfter = await getAccount(
      provider.connection,
      vaultTokenAccountPda
    );
    const vaultBalanceAfter = Number(vaultTokenAccountAfter.amount);

    const depositorUnderlyingAccountAfter = await getAccount(
      provider.connection,
      depositorUnderlyingAccount
    );
    const underlyingAfter = Number(depositorUnderlyingAccountAfter.amount);

    const depositorShareAccountAfter = await getAccount(
      provider.connection,
      depositorShareAccount
    );
    const sharesAfter = Number(depositorShareAccountAfter.amount);

    // Calculate expected underlying returned: (sharesToRedeem * vaultBalance) / totalShares
    const expectedUnderlying = Math.floor((sharesToRedeem * vaultBalanceBefore) / sharesBefore);
    const actualUnderlyingReturned = underlyingAfter - underlyingBefore;

    expect(sharesAfter).to.equal(sharesBefore - sharesToRedeem);
    expect(actualUnderlyingReturned).to.equal(expectedUnderlying);
    expect(vaultBalanceAfter).to.equal(vaultBalanceBefore - expectedUnderlying);

    console.log("Partial redeem successful!");
    console.log("  Shares redeemed:", sharesToRedeem);
    console.log("  Vault balance before:", vaultBalanceBefore);
    console.log("  Total shares before:", sharesBefore);
    console.log("  Expected underlying returned:", expectedUnderlying);
    console.log("  Actual underlying returned:", actualUnderlyingReturned);
    console.log("  New vault balance:", vaultBalanceAfter);
    console.log("  Remaining shares:", sharesAfter);
  });

  it("Redeems all remaining shares", async () => {
    // Get current share balance
    const depositorShareAccountBefore = await getAccount(
      provider.connection,
      depositorShareAccount
    );
    const allShares = Number(depositorShareAccountBefore.amount);

    const vaultTokenAccountBefore = await getAccount(
      provider.connection,
      vaultTokenAccountPda
    );
    const vaultBalanceBefore = Number(vaultTokenAccountBefore.amount);

    const depositorUnderlyingAccountBefore = await getAccount(
      provider.connection,
      depositorUnderlyingAccount
    );
    const underlyingBefore = Number(depositorUnderlyingAccountBefore.amount);

    // Redeem all shares
    const tx = await program.methods
      .redeem(new anchor.BN(allShares))
      .accounts({
        vault: vaultPda,
        underlyingMint: underlyingMint,
        vaultTokenAccount: vaultTokenAccountPda,
        shareMint: shareMintPda,
        vaultAuthority: vaultAuthorityPda,
        redeemerUnderlyingAccount: depositorUnderlyingAccount,
        redeemerShareAccount: depositorShareAccount,
        redeemer: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    console.log("Redeem all shares transaction signature:", tx);

    // Verify balances
    const depositorShareAccountAfter = await getAccount(
      provider.connection,
      depositorShareAccount
    );
    const sharesAfter = Number(depositorShareAccountAfter.amount);

    const vaultTokenAccountAfter = await getAccount(
      provider.connection,
      vaultTokenAccountPda
    );
    const vaultBalanceAfter = Number(vaultTokenAccountAfter.amount);

    const depositorUnderlyingAccountAfter = await getAccount(
      provider.connection,
      depositorUnderlyingAccount
    );
    const underlyingAfter = Number(depositorUnderlyingAccountAfter.amount);

    // When redeeming all shares, should get all vault balance
    expect(sharesAfter).to.equal(0);
    expect(vaultBalanceAfter).to.equal(0);
    expect(underlyingAfter).to.equal(underlyingBefore + vaultBalanceBefore);

    console.log("Full redeem successful!");
    console.log("  Shares redeemed:", allShares);
    console.log("  Underlying returned:", vaultBalanceBefore);
    console.log("  Final vault balance:", vaultBalanceAfter);
    console.log("  Final shares:", sharesAfter);
  });

  it("PoC Placeholder", async () => {
    console.log("\n=== PoC Placeholder Test ===");
  });

  it("Second PoC placeholder", async () => {
    console.log("\n=== Second PoC Placeholder Test ===");
  });
});
