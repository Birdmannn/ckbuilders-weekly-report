# CKB Smart Contract Development Guide

A comprehensive guide to understanding CKB (Nervos Network) smart contract development, covering fundamental concepts and practical patterns.

## Table of Contents

- [Core Concepts](#core-concepts)
- [Cell Model vs Account Model](#cell-model-vs-account-model)
- [Scripts in CKB](#scripts-in-ckb)
- [Entry Points and Routing](#entry-points-and-routing)
- [State Management](#state-management)
- [Error Handling](#error-handling)
- [Argument Parsing](#argument-parsing)
- [Common Patterns](#common-patterns)

---

## Core Concepts

### What is CKB?

CKB (Common Knowledge Base) is the layer 1 blockchain of the Nervos Network. Unlike account-based blockchains (Ethereum, Starknet, Solana), CKB uses a **Cell Model** similar to Bitcoin's UTXO model but more powerful.

### The Cell Model

A **Cell** is the fundamental data structure in CKB:

```rust
Cell {
    capacity: u64,           // CKB tokens (storage rent)
    lock: Script,            // Who can unlock/spend this cell
    type_: Option<Script>,   // Optional validation rules
    data: Vec<u8>,          // Your actual state data
}
```

**Key Properties:**

- Cells are immutable - you can't modify them, only consume and create new ones
- Each cell requires CKB tokens for storage (capacity)
- Cells can store arbitrary data in the `data` field

---

## Cell Model vs Account Model

### Account Model (Ethereum, Starknet, Solana)

```rust
// Starknet example
#[storage]
struct Storage {
    balances: LegacyMap<ContractAddress, u256>,
}

// Read
let balance = self.balances.read(user);

// Write
self.balances.write(user, new_balance);
```

**Characteristics:**

- Global mutable storage
- Contract stores all user data
- Direct read/write operations
- Account-centric

### Cell Model (CKB)

```rust
// Cell 1 (Alice's tokens)
Cell {
    capacity: 1000 CKB,
    lock: alice_signature_script,
    type_: token_script,
    data: [100 tokens]  // Alice's balance
}

// Cell 2 (Bob's tokens)
Cell {
    capacity: 500 CKB,
    lock: bob_signature_script,
    type_: token_script,  // Same script, different cell
    data: [50 tokens]     // Bob's balance
}
```

**Characteristics:**

- Local immutable state
- Users own their own cells
- Validation-based (not mutation-based)
- UTXO-centric

### Analogy

| Account Model              | Cell Model                |
| -------------------------- | ------------------------- |
| Bank account with balance  | Physical cash/coins       |
| Contract stores everything | Users hold their own data |
| Mutable storage            | Immutable cells           |
| Read/write operations      | Consume/create operations |

---

## Scripts in CKB

### What is a Script?

A **Script** is a small program that validates transactions. It has three components:

```rust
Script {
    code_hash: [32 bytes],  // Hash of the contract code
    hash_type: Type/Data,   // How to locate the code
    args: [bytes],          // Arguments/parameters
}
```

### Two Types of Scripts

Every cell has two scripts:

1. **Lock Script** - Controls who can unlock/spend the cell
   - Like a "key" or "ownership proof"
   - Must return 0 to unlock
   - Example: signature verification, multi-sig

2. **Type Script** (optional) - Enforces rules about the cell's data
   - Validates state transitions
   - Example: token contracts, NFT rules

### Script vs Contract

- **Contract** = The compiled code (reusable)
- **Script** = A specific instance with parameters (unique per cell)

Example:

```rust
// Same contract code, different instances
Script {
    code_hash: 0xABC123...,  // Token contract
    args: [alice_pubkey, token_id]  // Alice's instance
}

Script {
    code_hash: 0xABC123...,  // Same contract
    args: [bob_pubkey, token_id]    // Bob's instance
}
```

---

## Entry Points and Routing

### Single Entry Point

Unlike other chains with multiple entry points, CKB contracts have **one entry point** that routes to different functions:

```rust
pub fn program_entry() -> i8 {
    match run() {
        Ok(_) => 0,      // Success
        Err(err) => err as i8  // Error code
    }
}

fn run() -> Result<(), Error> {
    let script = load_script()?;
    let args = script.args().raw_data();

    // First byte is function selector
    let function_selector = args[0];

    match function_selector {
        0 => handle_transfer(&args[1..]),
        1 => handle_mint(&args[1..]),
        2 => handle_burn(&args[1..]),
        3 => handle_query(&args[1..]),
        _ => Err(Error::UnknownFunction),
    }
}
```

**Pattern:**

1. Read script arguments
2. First byte = function selector
3. Remaining bytes = function arguments
4. Dispatch to appropriate handler

---

## State Management

### Reading State

State is stored in cells. To read state, you load cells from **Sources**:

```rust
use ckb_std::ckb_constants::Source;

// Source::Input  - Cells being consumed (current state)
// Source::Output - Cells being created (new state)

fn get_total_amount(source: Source) -> Result<u64, Error> {
    let mut total = 0u64;
    let mut i = 0;

    loop {
        match load_cell_data(i, source) {
            Ok(data) => {
                let amount = parse_amount(&data)?;
                total += amount;
                i += 1;
            }
            Err(_) => break,  // No more cells
        }
    }

    Ok(total)
}
```

### Writing State

You don't "write" state - you **validate** that the transaction correctly transforms input cells into output cells:

```rust
fn handle_transfer(args: &[u8]) -> Result<(), Error> {
    // Read current state (inputs)
    let input_total = get_total_amount(Source::Input)?;

    // Read new state (outputs)
    let output_total = get_total_amount(Source::Output)?;

    // Validate the transition
    if input_total != output_total {
        return Err(Error::AmountMismatch);
    }

    Ok(())
}
```

### State Transition Examples

**Transfer (Conservation):**

```
Input:  [100 tokens]
Output: [60 tokens, 40 tokens]
Rule:   100 == 60 + 40 ✓
```

**Mint (Creation):**

```
Input:  [100 tokens]
Output: [150 tokens]
Rule:   100 + 50 == 150 ✓
```

**Burn (Destruction):**

```
Input:  [100 tokens]
Output: [70 tokens]
Rule:   100 - 30 == 70 ✓
```

---

## Error Handling

### Using Result Pattern

Instead of returning raw error codes, use Rust's `Result` type:

```rust
#[repr(i8)]
#[derive(Debug, Clone, Copy)]
enum Error {
    LoadScriptFailed = 1,
    NoFunctionSelector = 2,
    UnknownFunction = 3,
    InvalidArgs = 4,
    InsufficientBalance = 5,
    Unauthorized = 6,
}

pub fn program_entry() -> i8 {
    match run() {
        Ok(_) => 0,
        Err(err) => {
            ckb_std::debug!("Error: {:?}", err);
            err as i8
        }
    }
}

fn run() -> Result<(), Error> {
    let script = load_script()
        .map_err(|_| Error::LoadScriptFailed)?;

    // Use ? operator for clean error propagation
    let balance = get_balance()?;

    if balance < amount {
        return Err(Error::InsufficientBalance);
    }

    Ok(())
}
```

**Benefits:**

- Named errors instead of magic numbers
- Use `?` operator for clean error handling
- Automatic error logging
- Type-safe error codes

---

## Argument Parsing

### Raw Byte Slicing

```rust
let args = script.args().raw_data();

// args[0..20] means indices 0-19 (20 bytes total)
let sender = &args[0..20];      // Bytes 0-19
let recipient = &args[20..40];  // Bytes 20-39
let amount = u64::from_le_bytes([
    args[40], args[41], args[42], args[43],
    args[44], args[45], args[46], args[47],
]);
```

**Range Syntax:**

```rust
&args[0..20]   // Indices 0-19 (end is exclusive)
&args[0..]     // From 0 to end
&args[..5]     // From start to 4
&args[..]      // Everything
```

### Struct-Based Parsing

```rust
#[derive(Debug, Clone)]
struct EscrowArgs {
    sender: [u8; 20],
    recipient: [u8; 20],
    deadline: u64,
}

impl TryFrom<&[u8]> for EscrowArgs {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 48 {
            return Err(Error::InvalidArgs);
        }

        let mut sender = [0u8; 20];
        sender.copy_from_slice(&bytes[0..20]);

        let mut recipient = [0u8; 20];
        recipient.copy_from_slice(&bytes[20..40]);

        let deadline = u64::from_le_bytes([
            bytes[40], bytes[41], bytes[42], bytes[43],
            bytes[44], bytes[45], bytes[46], bytes[47],
        ]);

        Ok(EscrowArgs { sender, recipient, deadline })
    }
}

// Usage:
let escrow_args = EscrowArgs::try_from(args.as_ref())?;
```

### Client-Side Serialization

Arguments must be manually serialized on the client side:

```typescript
// JavaScript/TypeScript
function encodeEscrowArgs(
  sender: string,
  recipient: string,
  deadline: bigint,
): Uint8Array {
  const buffer = new Uint8Array(48);

  // Sender (20 bytes)
  buffer.set(hexToBytes(sender), 0);

  // Recipient (20 bytes)
  buffer.set(hexToBytes(recipient), 20);

  // Deadline (8 bytes, little-endian)
  const view = new DataView(buffer.buffer);
  view.setBigUint64(40, deadline, true);

  return buffer;
}

// Use in transaction
const args = encodeEscrowArgs("0x1234...abcd", "0x5678...ef01", 1234567890n);
```

**Important:** Both client and contract must agree on the serialization format.

---

## Common Patterns

### 1. Token Contract

```rust
fn handle_transfer(args: &[u8]) -> Result<(), Error> {
    let input_total = get_total_amount(Source::Input)?;
    let output_total = get_total_amount(Source::Output)?;

    // Conservation: no tokens created or destroyed
    if input_total != output_total {
        return Err(Error::AmountMismatch);
    }

    Ok(())
}
```

### 2. Escrow Contract

```rust
// Escrow cell locks tokens until conditions are met
Cell {
    lock: escrow_script(sender, recipient, deadline),
    type_: token_script,
    data: [100 tokens]  // Locked tokens
}

// Unlock conditions:
// 1. Both parties agree (both signatures)
// 2. Deadline passed (refund to sender)
// 3. Sender releases (transfer to recipient)
```

**Key Insight:** Tokens are stored in cells locked by the escrow script. The script defines who can unlock the cell under what conditions.

### 3. Multi-Signature

```rust
fn multisig_lock() -> Result<(), Error> {
    let args = load_script()?.args().raw_data();
    let threshold = args[0];

    let mut valid_sigs = 0;

    // Check each signature
    for i in 0..threshold {
        if verify_signature(i)? {
            valid_sigs += 1;
        }
    }

    if valid_sigs >= threshold {
        Ok(())
    } else {
        Err(Error::Unauthorized)
    }
}
```

### 4. Helper Functions

```rust
// Read token amount from cell data
fn read_token_amount(data: &[u8]) -> Result<u64, Error> {
    if data.len() < 8 {
        return Err(Error::InvalidCellData);
    }

    Ok(u64::from_le_bytes([
        data[0], data[1], data[2], data[3],
        data[4], data[5], data[6], data[7],
    ]))
}

// Count cells controlled by current script
fn count_script_cells(source: Source) -> Result<usize, Error> {
    let current_script = load_script()?;
    let current_hash = current_script.calc_script_hash();

    let mut count = 0;
    let mut i = 0;

    loop {
        match load_cell_lock(i, source) {
            Ok(lock) => {
                if lock.calc_script_hash() == current_hash {
                    count += 1;
                }
                i += 1;
            }
            Err(_) => break,
        }
    }

    Ok(count)
}
```

---

## Key Takeaways

1. **Cell Model ≠ Account Model**
   - Cells are immutable, not mutable storage
   - Users own cells, not contracts
   - Validation-based, not mutation-based

2. **Scripts are Validators**
   - Scripts don't store data, they validate transitions
   - Same script can validate many different cells
   - Scripts are stateless code

3. **Input = Read, Output = Write**
   - Input cells = current state (being consumed)
   - Output cells = new state (being created)
   - Your contract validates the transition is legal

4. **Single Entry Point with Routing**
   - One `program_entry()` function
   - Use first byte as function selector
   - Dispatch to different handlers

5. **Manual Serialization**
   - Arguments are raw bytes
   - Client must serialize structs to bytes
   - Contract must deserialize bytes to structs
   - Both sides must agree on format

6. **Ownership via Lock Scripts**
   - Lock script = ownership rules
   - Escrow = cells locked by escrow conditions
   - Tokens stored in cells, not in contracts

---

## Resources

- [CKB Documentation](https://docs.nervos.org/)
- [ckb-std Library](https://github.com/nervosnetwork/ckb-std)
- [CKB Script Templates](https://github.com/cryptape/ckb-script-templates)
- [Molecule Serialization](https://github.com/nervosnetwork/molecule)

---

## Example Project Structure

```
nervosprotocol/
├── contracts/
│   └── hello-world/
│       ├── src/
│       │   ├── main.rs      # Contract entry point
│       │   └── lib.rs       # Library exports
│       └── Cargo.toml
├── tests/
│   └── src/
│       └── tests.rs         # Integration tests
└── Cargo.toml               # Workspace config
```

---

_This guide covers the fundamental concepts of CKB smart contract development. For production contracts, always audit your code and follow security best practices._
