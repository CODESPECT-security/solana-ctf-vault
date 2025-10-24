# Fuzzing Infrastructure for Vault PDA

This directory contains the honggfuzz fuzzing setup for the Vault PDA Solana program.

## Status

🚧 **Under Construction** - Fuzzing infrastructure is being set up.

## Planned Fuzz Targets

### Individual Instruction Targets

1. **fuzz_initialize** - Test protocol initialization
   - Edge cases in protocol state setup
   - Account validation testing

2. **fuzz_initialize_vault** - Test vault creation
   - Various underlying mint configurations
   - PDA derivation edge cases

3. **fuzz_deposit** - Test deposit functionality
   - Share calculation overflow/underflow
   - Zero and maximum amount handling
   - First deposit vs subsequent deposits

4. **fuzz_redeem** - Test redemption functionality
   - Share burning edge cases
   - Proportional redemption calculations
   - Empty vault handling

5. **fuzz_transfer_ownership** - Test ownership transfer
   - Authorization validation
   - Account ownership checks

### Stateful Fuzzing

6. **fuzz_all_instructions** - Combined instruction fuzzing
   - Random sequences of operations
   - State transition testing
   - Multi-step vulnerability discovery

## Directory Structure

```
fuzz/
├── Cargo.toml           # Fuzzing workspace configuration
├── fuzz_targets/        # Fuzz harness implementations (to be added)
├── corpus/              # Seed inputs (generated during fuzzing)
├── hfuzz_workspace/     # Honggfuzz working directory (generated)
└── README.md           # This file
```

## Setup Instructions

### Prerequisites

**Option 1: Native (Linux/x86_64 macOS)**
```bash
cargo install honggfuzz
```

**Option 2: Docker (macOS ARM64/Apple Silicon)** ⭐ RECOMMENDED
```bash
# Docker must be installed and running
# No other prerequisites needed!
```

> **Note for macOS ARM64 users:** honggfuzz has linking issues on Apple Silicon. Use Docker instead.

### Running Fuzz Tests

#### Currently Implemented Targets

**fuzz_deposit** - Fuzzes the deposit instruction

**🐋 Using Docker (Recommended for macOS ARM64):**
```bash
# Run with 1000 iterations (from project root)
cd fuzz
FUZZ_ITERATIONS=1000 ./run-docker.sh

# Or with default iterations
./run-docker.sh
```

**💻 Native (Linux/x86 macOS):**
```bash
# Run deposit fuzzing (from project root)
cargo hfuzz run fuzz_deposit

# Run with custom timeout (10 seconds)
HFUZZ_RUN_ARGS="--run_time 10" cargo hfuzz run fuzz_deposit

# Run with specific number of iterations
HFUZZ_RUN_ARGS="--iterations 1000" cargo hfuzz run fuzz_deposit
```

**🍎 macOS ARM64 (Apple Silicon) - Using libFuzzer:**
```bash
# Build the fuzzer (from fuzz/ directory)
cd fuzz
cargo build --release --bin fuzz_deposit_libfuzzer --features libfuzzer_fuzz

# Run with 1000 iterations
../target/release/fuzz_deposit_libfuzzer -runs=1000 -verbosity=1

# Run for 60 seconds
../target/release/fuzz_deposit_libfuzzer -max_total_time=60 -verbosity=1

# Run with larger inputs (default is 4096 bytes)
../target/release/fuzz_deposit_libfuzzer -max_len=8192 -runs=1000

# View all libFuzzer options
../target/release/fuzz_deposit_libfuzzer -help=1
```

> **Note for macOS ARM64 users:** libFuzzer works natively on Apple Silicon without Docker. However, it doesn't provide coverage-guided fuzzing without sanitizer instrumentation. For better coverage metrics, use the Docker option with honggfuzz.

#### Targets To Be Implemented

```bash
# Run a specific fuzz target (once implemented)
cargo hfuzz run fuzz_initialize
cargo hfuzz run fuzz_initialize_vault
cargo hfuzz run fuzz_redeem
cargo hfuzz run fuzz_transfer_ownership
cargo hfuzz run fuzz_all_instructions

# Run all targets sequentially
cargo hfuzz run fuzz_initialize && \
cargo hfuzz run fuzz_initialize_vault && \
cargo hfuzz run fuzz_deposit && \
cargo hfuzz run fuzz_redeem && \
cargo hfuzz run fuzz_transfer_ownership && \
cargo hfuzz run fuzz_all_instructions
```

### Viewing Results

```bash
# Check crashes
ls hfuzz_workspace/fuzz_initialize/crashes/

# Replay a crash
cargo hfuzz run-debug fuzz_initialize hfuzz_workspace/fuzz_initialize/crashes/CRASH_FILE

# View coverage
cargo hfuzz run fuzz_initialize --coverage
```

### Building Fuzz Targets

```bash
# Build all fuzz targets (from the fuzz/ directory)
cargo build --release

# Build a specific target
cargo build --release --bin fuzz_initialize
```

## Dependencies Added

The following dependencies have been added to `Cargo.toml`:

**Fuzzing Framework:**
- `honggfuzz = { version = "0.5", optional = true }` - Coverage-guided fuzzer (for Linux/x86)
- `libfuzzer-sys = { version = "0.4", optional = true }` - LLVM libFuzzer bindings (for macOS ARM64)
- `arbitrary = { version = "1.3", features = ["derive"] }` - Generate arbitrary test data

**Features:**
- `honggfuzz_fuzz` - Enable honggfuzz support (Linux/x86/Docker)
- `libfuzzer_fuzz` - Enable libFuzzer support (macOS ARM64 native)

**Solana Testing:**
- `solana-program-test = "1.18"` - Solana program test framework
- `solana-sdk = "1.18"` - Solana SDK for account/transaction handling
- `anchor-lang = "0.31.1"` - Anchor framework (matching program version)
- `anchor-spl = "0.31.1"` - Anchor SPL token utilities
- `spl-token = "4.0"` - SPL Token program interface

**Local Program:**
- `vault-pda = { path = "../programs/vault-pda", features = ["no-entrypoint"] }` - The vault program to fuzz

## Implementation Status

- [x] Setup fuzz infrastructure
- [x] Create `fuzz_setup.rs` helper module with common functions
- [x] Implement `fuzz_deposit` (honggfuzz version) - Tests deposit instruction with:
  - Arbitrary deposit amounts
  - Various token decimals
  - Invariant checks for balance transfers
  - Share calculation verification
  - First deposit vs subsequent deposit logic
  - Yield accumulation scenarios
- [x] Implement `fuzz_deposit_libfuzzer` (libFuzzer version for macOS ARM64)
  - Complete port of all invariant checks
  - Native support for Apple Silicon
  - Successfully tested with 1000 iterations
- [x] Create helper scripts for running fuzzers (Docker support)
- [x] Add dual fuzzer support (honggfuzz + libFuzzer)
- [ ] Implement `fuzz_initialize`
- [ ] Implement `fuzz_initialize_vault`
- [ ] Implement `fuzz_redeem`
- [ ] Implement `fuzz_transfer_ownership`
- [ ] Implement `fuzz_all_instructions`
- [ ] Add corpus seeds for better initial coverage
- [ ] Document findings and vulnerabilities

## What fuzz_deposit Tests

The deposit fuzzer performs comprehensive property-based testing with three layers of checks:

### Fuzzed Inputs
- `amount`: Deposit amount (u64) - any value from 0 to MAX
- `initial_balance`: User's starting balance (u64) - any value
- `decimals`: Token decimals (0-18) - tests various precision levels
- `yield_amount`: Yield/profit added to vault before deposit (0 to 1B) - simulates yield growth
- `do_initial_deposit`: Whether to make an initial deposit first (bool)
- `initial_deposit_amount`: Amount for initial deposit if enabled (u64)

### Test Scenarios

The fuzzer automatically tests multiple real-world scenarios:

**1. FIRST_DEPOSIT** - Empty vault, first depositor
```
Vault: 0 tokens, 0 shares → User deposits X → Gets X shares (1:1)
```

**2. SUBSEQUENT** - Vault has existing shares, new depositor
```
Initial: User deposits 1000 → Gets 1000 shares
Main: User deposits X → Gets proportional shares
```

**3. YIELD_GROWTH** - Vault earned profit between deposits ⚠️ CRITICAL
```
1. Initial deposit: 1000 tokens → 1000 shares
2. Vault earns yield: +100 tokens minted directly to vault
3. Vault now: 1100 tokens, 1000 shares (value/share = 1.1)
4. New deposit: 1000 tokens → Should get 909 shares (not 1000!)
```

**4. BASIC** - Simple deposit without prior setup
```
Clean slate deposit
```

### Why Yield Growth Testing Matters

This tests a **critical attack vector**: the "First Depositor Inflation Attack"

**Attack scenario:**
```rust
1. Attacker deposits 1 token → gets 1 share
2. Attacker directly transfers 1,000,000 tokens to vault (not via deposit!)
3. Vault: 1,000,001 tokens, 1 share (value/share = 1,000,001)
4. Victim deposits 1,000,000 tokens
   → shares = (1,000,000 × 1) / 1,000,001 = 0 shares (rounds down!)
5. Victim's tokens are trapped, attacker controls 100% of vault
```

Our fuzzer **automatically generates this scenario** through:
- `do_initial_deposit=true, initial_deposit_amount=1`
- `yield_amount=1000000` (simulates attacker's direct transfer)
- `amount=1000000` (victim's deposit)

The **MONOTONICITY** invariant catches this:
```rust
assert!(shares_minted > 0, "Deposited X but got 0 shares - attack!");
```

### Mathematical Property Checks

**1. Conservation of Tokens**
```
vault_before + user_before = vault_after + user_after
```
Ensures no tokens are created or destroyed (fundamental physics-like property).

**2. Basic Balance Checks**
- Vault balance increases by exact deposit amount
- User balance decreases by exact deposit amount

### Security Property Checks (Attack Prevention)

**1. Share Value Preservation** ⚠️ CRITICAL
```
value_per_share_after ≥ value_per_share_before
```
Prevents **share dilution attacks** where deposits could devalue existing shareholders.

**2. Fairness - User Exchange Rate**
```
shares_minted ≤ expected_shares + 0.1%
```
Ensures rounding favors the vault/existing shareholders, not the depositor. Prevents **rounding exploits**.

**3. Monotonicity**
```
amount > 0 → shares_minted > 0
```
Depositing tokens must always result in receiving shares. Prevents **value extraction** bugs.

**4. Reasonable Bounds**
```
shares_minted ≤ amount × 2
```
Sanity check that share amounts are reasonable (prevents obvious calculation bugs).

### Correctness Checks (Implementation Verification)

**1. Share Supply Accounting**
- Share supply increases by exactly shares minted
- User share balance increases by exactly shares minted

**2. Formula Verification**
- First deposit: `shares = amount` (1:1 minting)
- Subsequent deposits: `shares = (amount × total_shares) / total_assets`
- Allows ±1 rounding error

### Error Handling

**Expected Errors (Handled Gracefully):**
- `InsufficientFunds` - User doesn't have enough tokens
- `InvalidAmount` - Amount is zero or invalid
- `InsufficientShares` - Calculation results in 0 shares
- `MathOverflow` - Arithmetic overflow in share calculation

**Unexpected Errors (Fuzzer Panics & Reports):**
- Program panics
- Assertion failures (invariant violations)
- Unexpected account validation failures
- Any error not in the expected list above

### What Gets Caught

This approach catches:
- ✅ **Arithmetic bugs** - overflow, underflow, division by zero
- ✅ **Share dilution attacks** - deposits that harm existing users
- ✅ **Rounding exploits** - manipulating truncation for profit
- ✅ **Value extraction bugs** - deposit tokens without receiving fair shares
- ✅ **Conservation violations** - tokens disappearing or appearing
- ✅ **Edge cases** - first deposit, max values, zero amounts
- ✅ **Precision issues** - different decimal configurations

## Next Steps

- [ ] Implement remaining fuzz harnesses
- [ ] Create helper scripts for running fuzzers
- [ ] Add corpus seeds for better initial coverage
- [ ] Document findings and vulnerabilities

## Resources

- [Honggfuzz Documentation](https://github.com/google/honggfuzz)
- [Solana Fuzzing Best Practices](https://github.com/coral-xyz/sealevel-attacks)
- [Arbitrary Crate Documentation](https://docs.rs/arbitrary/)
