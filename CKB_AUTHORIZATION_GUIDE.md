# CKB Authorization and Initialization Guide

A comprehensive guide to understanding authorization patterns and initialization in Nervos CKB smart contracts, compared with other blockchain platforms.

## Table of Contents
- [Authorization in CKB vs Other Chains](#authorization-in-ckb-vs-other-chains)
- [Understanding Lock Scripts](#understanding-lock-scripts)
- [Getting Lock Script Hashes](#getting-lock-script-hashes)
- [Authorization Patterns](#authorization-patterns)
- [Initialization in CKB](#initialization-in-ckb)
- [Available Libraries](#available-libraries)

---

## Authorization in CKB vs Other Chains

### The Fundamental Difference

CKB uses a **UTXO model** (like Bitcoin), not an **account model** (like Ethereum/Starknet/Stellar). This means there's no concept of a "caller" or "msg.sender".

### Comparison Table

| Chain | Model | Authorization Method |
|-------|-------|---------------------|
| **Ethereum** | Account | `msg.sender` - who called the function |
| **Starknet** | Account | `get_caller_address()` - who invoked the contract |
| **Stellar** | Account | `require_auth(address)` - verify signature |
| **CKB** | UTXO | Check lock scripts of input cells |

### Key Concept

In CKB, authorization works by asking: **"Which cells are being spent, and who owns them?"**

Instead of:
```rust
// Ethereum/Starknet style (doesn't exist in CKB)
if msg.sender != admin {
    revert("Unauthorized");
}
```

You do:
```rust
// CKB style
if !is_authorized_by_address(&admin_address)? {
    return Err(Error::Unauthorized);
}
```

---

## Understanding Lock Scripts

### Lock Script Structure

Every cell in CKB has a lock script that defines who can unlock/spend it:

```rust
Lock Script {
    code_hash: [u8; 32],    // Which lock script program to run
    hash_type: u8,          // "data" (0) or "type" (1)
    args: Vec<u8>,          // Arguments - typically the "address"
}
```

### Standard SECP256K1 Lock

The most common lock script (similar to Bitcoin's P2PKH):

```rust
Lock Script {
    code_hash: 0x9bd7e06f3ecf4be0f2fcd2188b23f1b9fcc88e5d4b65a8637b17723bbda3cce8,
    hash_type: "type",
    args: 0x36c329ed630d6ce750712a477543672adab57f4c  // 20 bytes - the "address"
}
```

### The "Address" is in the Args

For SECP256K1 locks:
- `args` = 20-byte blake160 hash of the user's public key
- This is what we typically think of as the "address"
- CKB addresses (like `ckb1qyq...`) encode this lock script

---

## Getting Lock Script Hashes

### Method 1: From CKB Address (Off-chain)

```javascript
// Using @ckb-lumos/helpers (JavaScript/TypeScript)
import { parseAddress } from '@ckb-lumos/helpers';
import { utils } from '@ckb-lumos/base';

const address = "ckb1qyqg8xr5z8xr5z...";
const lockScript = parseAddress(address);

console.log("Lock Script:", lockScript);
// {
//   code_hash: "0x9bd7...",
//   hash_type: "type",
//   args: "0x36c329ed630d6ce750712a477543672adab57f4c"
// }

// Compute the full lock hash
const lockHash = utils.computeScriptHash(lockScript);
console.log("Lock Hash:", lockHash);
```

### Method 2: Extract from Lock Script (On-chain)

```rust
use ckb_std::high_level::load_cell_lock;

// Get lock script from a cell
let lock = load_cell_lock(0, Source::Input)?;

// Get the full lock hash
let lock_hash = lock.calc_script_hash();

// Or extract just the args (the "address")
let lock_args = lock.args().raw_data();
let address_bytes = &lock_args[0..20]; // First 20 bytes
```

---

## Authorization Patterns

### Option A: Check Full Lock Hash

Compare the entire lock script hash:

```rust
fn is_authorized_by_lock_hash(authorized_lock_hash: &[u8; 32]) -> Result<bool, Error> {
    let mut i = 0;
    
    loop {
        match load_cell_lock(i, Source::Input) {
            Ok(lock) => {
                let lock_hash = lock.calc_script_hash();
                
                // If any input cell has the authorized lock, they signed this tx
                if lock_hash.as_slice() == authorized_lock_hash {
                    return Ok(true);
                }
                i += 1;
            }
            Err(_) => break,
        }
    }
    
    Ok(false)
}
```

**Pros:** Most precise - matches exact lock script  
**Cons:** Less flexible - must match code_hash, hash_type, and args

### Option B: Check Address Only (Recommended)

Compare just the args field (the "address"):

```rust
fn is_authorized_by_address(authorized_address: &[u8; 20]) -> Result<bool, Error> {
    let mut i = 0;
    
    loop {
        match load_cell_lock(i, Source::Input) {
            Ok(lock) => {
                let lock_args = lock.args().raw_data();
                
                // Check if the address matches
                if lock_args.len() >= 20 && &lock_args[0..20] == authorized_address {
                    return Ok(true);
                }
                i += 1;
            }
            Err(_) => break,
        }
    }
    
    Ok(false)
}
```

**Pros:** Simpler, more flexible  
**Cons:** Only checks the address part, not the full lock script

### Storing the Admin Address

You have three options:

#### 1. Hardcoded (Compile-time)

```rust
const ADMIN_ADDRESS: [u8; 20] = [
    0x36, 0xc3, 0x29, 0xed, 0x63, 0x0d, 0x6c, 0xe7,
    0x50, 0x71, 0x2a, 0x47, 0x75, 0x43, 0x67, 0x2a,
    0xda, 0xb5, 0x7f, 0x4c,
];

fn handle_mint(args: &[u8]) -> Result<(), Error> {
    if !is_authorized_by_address(&ADMIN_ADDRESS)? {
        return Err(Error::Unauthorized);
    }
    // ...
}
```

**Pros:** Simple, no runtime overhead  
**Cons:** Can't change admin without redeploying

#### 2. In Script Args (Deployment-time) - Recommended

```rust
// Script args format: [admin_address: 20 bytes][function_selector: 1 byte][function_args: ...]

const ADMIN_ADDRESS_LEN: usize = 20;
const FUNCTION_SELECTOR_OFFSET: usize = 20;

fn get_admin_address() -> Result<[u8; 20], Error> {
    let script = load_script().map_err(|_| Error::LoadScriptFailed)?;
    let args = script.args().raw_data();
    
    if args.len() < ADMIN_ADDRESS_LEN {
        return Err(Error::InvalidArgs);
    }
    
    let mut admin_address = [0u8; 20];
    admin_address.copy_from_slice(&args[0..ADMIN_ADDRESS_LEN]);
    Ok(admin_address)
}

fn run() -> Result<(), Error> {
    let script = load_script().map_err(|_| Error::LoadScriptFailed)?;
    let args = script.args().raw_data();
    
    if args.len() < FUNCTION_SELECTOR_OFFSET + 1 {
        return Err(Error::NoFunctionSelector);
    }
    
    let function_selector = args[FUNCTION_SELECTOR_OFFSET];
    let function_args = &args[FUNCTION_SELECTOR_OFFSET + 1..];
    
    // Dispatch functions...
}

fn handle_mint(args: &[u8]) -> Result<(), Error> {
    let admin_address = get_admin_address()?;
    
    if !is_authorized_by_address(&admin_address)? {
        return Err(Error::Unauthorized);
    }
    // ...
}
```

**Pros:** Flexible at deployment, different instances can have different admins  
**Cons:** Immutable after deployment

#### 3. In Cell Data (Runtime, Mutable)

Store admin address in a config cell that your contract reads from cell deps.

**Pros:** Can be updated  
**Cons:** More complex, requires cell deps

---

## Initialization in CKB

### No Global Constructor

Unlike other chains, CKB has:
- No global contract state that needs initialization
- No single "contract instance"
- Scripts are just validation code
- State lives in individual cells

### Comparison

| Chain | Initialization |
|-------|----------------|
| **Starknet** | `constructor()` runs once when contract is deployed |
| **Stellar** | `initialize()` called once, sets `initialized = true` |
| **Ethereum** | `constructor()` runs once at deployment |
| **CKB** | No global initialization - validate first cell creation |

### Detecting Initialization

Check if this is the first cell being created with your script:

```rust
fn is_initialization() -> Result<bool, Error> {
    // If there are no input cells with this script, it's initialization
    let input_count = count_script_cells(Source::Input)?;
    let output_count = count_script_cells(Source::Output)?;
    
    // Initialization: 0 inputs, 1+ outputs
    Ok(input_count == 0 && output_count > 0)
}

fn count_script_cells(source: Source) -> Result<usize, Error> {
    let current_script = load_script().map_err(|_| Error::LoadScriptFailed)?;
    let current_script_hash = current_script.calc_script_hash();
    
    let mut count = 0;
    let mut i = 0;
    
    loop {
        match load_cell_lock(i, source) {
            Ok(lock) => {
                if lock.calc_script_hash() == current_script_hash {
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

### Initialization Validation Pattern

```rust
fn run() -> Result<(), Error> {
    let script = load_script().map_err(|_| Error::LoadScriptFailed)?;
    let args = script.args().raw_data();
    
    // Check if this is initialization
    if is_initialization()? {
        return validate_initialization(&args);
    }
    
    // Normal operation - dispatch to functions
    // ...
}

fn validate_initialization(args: &[u8]) -> Result<(), Error> {
    ckb_std::debug!("=== INITIALIZATION ===");
    
    // Validate constructor arguments
    if args.len() < 20 {
        return Err(Error::InvalidArgs);
    }
    
    let admin_address = &args[0..20];
    
    // Validate initial state
    let output_data = load_cell_data(0, Source::Output)?;
    let initial_supply = read_token_amount(&output_data)?;
    
    // Enforce initialization rules
    if initial_supply != 0 {
        return Err(Error::InvalidCellData);
    }
    
    // Only admin can initialize
    if !is_authorized_by_address(admin_address.try_into().unwrap())? {
        return Err(Error::Unauthorized);
    }
    
    Ok(())
}
```

### Script Args as Constructor Parameters

When deploying, set script args to include initialization parameters:

```javascript
// Off-chain deployment
const adminAddress = "0x36c329ed630d6ce750712a477543672adab57f4c"; // 20 bytes
const maxSupply = "0x00e8764817000000"; // 8 bytes (1,000,000 in little-endian)

const typeScript = {
    code_hash: "0xYOUR_CONTRACT_CODE_HASH",
    hash_type: "type",
    args: adminAddress + maxSupply // Constructor parameters
};
```

---

## Available Libraries

### What Exists

**ckb-std** (What you already have)
- Low-level syscalls and helpers
- Functions like `load_cell_data`, `load_script`, `load_cell_lock`
- Memory allocator
- Basic debugging macros

**ckb-auth** (Signature verification only)
- Handles cryptographic signature validation
- Supports multiple algorithms (Ethereum, Bitcoin, etc.)
- Deployed as a separate library
- Only for signature verification, not general authorization

### What's Missing

Unlike Ethereum's OpenZeppelin or Starknet's standard libraries, CKB doesn't have:
- ❌ Access control helpers (Ownable, AccessControl)
- ❌ Initialization patterns
- ❌ Reentrancy guards
- ❌ Common token standards with built-in auth
- ❌ Pausable contracts
- ❌ Role-based access control

### Recommended Approach

Create your own helper module for reusable patterns:

```rust
// contracts/hello-world/src/lib.rs

use ckb_std::ckb_constants::Source;
use ckb_std::high_level::{load_cell_lock, load_script};
use ckb_std::error::SysError;

pub fn is_authorized_by_address(authorized_address: &[u8; 20]) -> Result<bool, SysError> {
    let mut i = 0;
    loop {
        match load_cell_lock(i, Source::Input) {
            Ok(lock) => {
                let lock_args = lock.args().raw_data();
                if lock_args.len() >= 20 && &lock_args[0..20] == authorized_address {
                    return Ok(true);
                }
                i += 1;
            }
            Err(SysError::IndexOutOfBound) => break,
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

pub fn is_initialization() -> Result<bool, SysError> {
    match load_cell_lock(0, Source::GroupInput) {
        Ok(_) => Ok(false),
        Err(SysError::IndexOutOfBound) => Ok(true),
        Err(e) => Err(e),
    }
}

pub fn get_admin_address() -> Result<[u8; 20], SysError> {
    let script = load_script()?;
    let args = script.args().raw_data();
    
    if args.len() < 20 {
        return Err(SysError::Encoding);
    }
    
    let mut admin_address = [0u8; 20];
    admin_address.copy_from_slice(&args[0..20]);
    Ok(admin_address)
}
```

---

## Complete Example

Here's a complete example combining authorization and initialization:

```rust
// main.rs
mod errors;
use errors::Error;

use ckb_std::ckb_constants::Source;
use ckb_std::high_level::{load_cell_data, load_cell_lock, load_script};

const ADMIN_ADDRESS_LEN: usize = 20;
const FUNCTION_SELECTOR_OFFSET: usize = 20;

fn get_admin_address() -> Result<[u8; 20], Error> {
    let script = load_script().map_err(|_| Error::LoadScriptFailed)?;
    let args = script.args().raw_data();
    
    if args.len() < ADMIN_ADDRESS_LEN {
        return Err(Error::InvalidArgs);
    }
    
    let mut admin_address = [0u8; 20];
    admin_address.copy_from_slice(&args[0..ADMIN_ADDRESS_LEN]);
    Ok(admin_address)
}

fn is_authorized_by_address(authorized_address: &[u8; 20]) -> Result<bool, Error> {
    let mut i = 0;
    
    loop {
        match load_cell_lock(i, Source::Input) {
            Ok(lock) => {
                let lock_args = lock.args().raw_data();
                if lock_args.len() >= 20 && &lock_args[0..20] == authorized_address {
                    return Ok(true);
                }
                i += 1;
            }
            Err(_) => break,
        }
    }
    
    Ok(false)
}

fn is_initialization() -> Result<bool, Error> {
    let input_count = count_script_cells(Source::Input)?;
    Ok(input_count == 0)
}

fn run() -> Result<(), Error> {
    let script = load_script().map_err(|_| Error::LoadScriptFailed)?;
    let args = script.args().raw_data();
    
    // Handle initialization
    if is_initialization()? {
        return validate_initialization(&args);
    }
    
    // Normal operation
    if args.len() < FUNCTION_SELECTOR_OFFSET + 1 {
        return Err(Error::NoFunctionSelector);
    }
    
    let function_selector = args[FUNCTION_SELECTOR_OFFSET];
    let function_args = &args[FUNCTION_SELECTOR_OFFSET + 1..];
    
    match function_selector {
        0 => handle_transfer(function_args),
        1 => handle_mint(function_args),
        2 => handle_burn(function_args),
        _ => Err(Error::UnknownFunction),
    }
}

fn validate_initialization(args: &[u8]) -> Result<(), Error> {
    let admin_address = get_admin_address()?;
    
    // Only admin can initialize
    if !is_authorized_by_address(&admin_address)? {
        return Err(Error::Unauthorized);
    }
    
    // Validate initial state
    let output_data = load_cell_data(0, Source::Output)?;
    // Add your validation logic here
    
    Ok(())
}

fn handle_mint(args: &[u8]) -> Result<(), Error> {
    let admin_address = get_admin_address()?;
    
    // Check authorization
    if !is_authorized_by_address(&admin_address)? {
        return Err(Error::Unauthorized);
    }
    
    // Mint logic...
    Ok(())
}
```

---

## Key Takeaways

1. **No msg.sender in CKB** - Check which cells are being spent instead
2. **Lock scripts define ownership** - The args field typically contains the "address"
3. **Authorization = checking input cells** - See if any input has the required lock
4. **No global constructor** - Validate during first cell creation
5. **Script args = constructor parameters** - Set at deployment, immutable
6. **No standard library** - Build your own helper functions
7. **UTXO model is different** - Think in terms of cells, not accounts

---

## Resources

- [CKB Documentation](https://docs.nervos.org/)
- [ckb-std GitHub](https://github.com/nervosnetwork/ckb-std)
- [CKB Auth Guide](https://talk.nervos.org/t/guide-how-to-use-auth-library/6650)
- [CKB Script Programming](https://docs.nervos.org/docs/script-course/intro-to-script-1)
