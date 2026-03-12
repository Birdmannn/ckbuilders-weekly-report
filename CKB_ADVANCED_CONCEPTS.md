# CKB Advanced Concepts Guide

A comprehensive guide covering advanced CKB smart contract development concepts, including cell indexing, token identification, state management, and querying patterns.

## Table of Contents
- [Source Types: Input vs Output](#source-types-input-vs-output)
- [Cell Indexing and Iteration](#cell-indexing-and-iteration)
- [Token Identification in CKB](#token-identification-in-ckb)
- [Type Script Hash as Token ID](#type-script-hash-as-token-id)
- [Constructor Arguments and Initialization](#constructor-arguments-and-initialization)
- [Events and Logging](#events-and-logging)
- [State Storage Patterns](#state-storage-patterns)
- [Timestamp Handling](#timestamp-handling)
- [Cell Identification Patterns](#cell-identification-patterns)
- [On-chain vs Off-chain Querying](#on-chain-vs-off-chain-querying)
- [Campaign/Task System Design](#campaigntask-system-design)

---

## Source Types: Input vs Output

### Understanding Source Enum

CKB provides different "sources" to access cells in a transaction:

**Source::Input**
- Cells being consumed/spent in the transaction
- Represents the "before" state
- Used to read current balances, existing state

**Source::Output**
- Cells being created in the transaction
- Represents the "after" state
- Used to validate new balances, updated state

**Source::GroupInput / Source::GroupOutput**
- Filtered views showing only cells with the same script as the currently running script
- More efficient when you only care about your contract's cells

**Source::CellDep**
- Referenced cells (read-only)
- Used to access data from other cells without consuming them

**Source::HeaderDep**
- Referenced block headers
- Used to access timestamp and other block metadata

### When to Use Each Source

- **Reading existing state**: Use `Source::Input` or `Source::GroupInput`
- **Validating new state**: Use `Source::Output` or `Source::GroupOutput`
- **Reading external data**: Use `Source::CellDep`
- **Getting timestamps**: Use `Source::HeaderDep`

### Key Principle

Your script validates the transformation from inputs to outputs. It doesn't execute the transformation - it only checks if the transformation is valid.

---

## Cell Indexing and Iteration

### How load_cell_* Functions Work

When you call `load_cell_data(index, source)`, you're accessing cells by their position in the transaction:

- `index` = position in the array (0, 1, 2, ...)
- `source` = which array to access (Input, Output, CellDep, etc.)

### Transaction Context

Your script runs inside a transaction and can see all cells in that transaction:

```
Transaction {
    inputs: [Cell0, Cell1, Cell2, ...],
    outputs: [Cell0, Cell1, Cell2, ...],
    cell_deps: [Cell0, Cell1, ...],
    header_deps: [Header0, Header1, ...]
}
```

### Index 0 is NOT Always Safe

Don't assume specific cells are at specific positions. Always:
1. Iterate through cells to find what you need
2. Validate cell ownership by checking lock scripts
3. Identify cell types by checking type scripts

### Iteration Pattern

The standard pattern for iterating through cells:
1. Start with index 0
2. Try to load the cell
3. If successful, process it and increment index
4. If error (IndexOutOfBound), stop - no more cells
5. Repeat

### Group Sources

`Source::GroupInput` and `Source::GroupOutput` automatically filter to show only cells with your script, making iteration more efficient.

---

## Token Identification in CKB

### No "Token Address" Concept

Unlike Ethereum/Starknet where tokens have contract addresses, CKB uses **Type Script Hash** as the token identifier.

### Type Script Hash = Token ID

Each fungible token is identified by the hash of its type script:

```
Token Cell {
    capacity: CKB amount for storage,
    lock: user_lock_script (who owns this balance),
    type: token_type_script (which token this is),
    data: token_amount (how many tokens)
}
```

The type script hash uniquely identifies the token across the entire blockchain.

### Multiple Tokens = Multiple Cells

A user holding 3 different tokens has 3 separate cells, each with a different type script.

### Token Standards

**Simple UDT (sUDT)**
- Most common token standard
- Type script args contain the owner's lock hash (who can mint/burn)

**Extended UDT**
- Includes metadata in type script args
- Can store decimals, name, symbol, etc.

### Computing Token ID

The token ID is calculated by hashing the type script structure (code_hash + hash_type + args).

---

## Type Script Hash as Token ID

### Type Script Structure

```
TypeScript {
    code_hash: [32 bytes],  // Hash of token contract code
    hash_type: 1 byte,      // "type" or "data"
    args: Vec<u8>           // Token-specific parameters
}
```

### Token ID Calculation

The token ID is the hash of the entire type script structure. Same type script = same token.

### Type Script Args = Constructor Parameters

The args field in the type script serves as immutable constructor parameters:
- Owner address
- Decimals
- Token name
- Token symbol
- Maximum supply
- Any other token-specific configuration

### Storage Implications

Every cell stores the complete type script (including args), so there's a storage cost. Consider minimizing args size or storing metadata in a separate info cell.

---

## Constructor Arguments and Initialization

### No Global Constructor

CKB has no global constructor that runs once. Instead:
- Script args serve as constructor parameters
- Set at deployment time
- Immutable after creation
- Stored in every cell using that script

### Initialization Detection

Detect first-time cell creation by checking if there are input cells with your script:
- 0 input cells with your script = initialization
- 1+ input cells with your script = update operation

### Initialization Validation

During initialization, validate:
- Constructor parameters are valid
- Initial state is correct
- Only authorized party can initialize
- Required conditions are met

### Script Args vs Cell Data

**Script Args (Immutable)**
- Configuration parameters
- Admin addresses
- Contract settings
- Part of script identity

**Cell Data (Mutable)**
- State that changes
- Balances
- Counters
- User data

---

## Events and Logging

### No Traditional Events

CKB does NOT emit events like Ethereum. There are no event logs stored on-chain.

### Why No Events?

The UTXO model makes events unnecessary - the transaction structure itself is the event:
- Transaction inputs/outputs show what changed
- Cell data shows state transitions
- Lock scripts show who authorized the action

### Alternatives to Events

**1. Transaction Structure as Event**
The transaction itself contains all event information. Off-chain indexers parse transactions to extract "events".

**2. Debug Logs (Development Only)**
Use `ckb_std::debug!()` for debugging, but these are NOT stored on-chain.

**3. Cell Data as Event Storage**
Create dedicated "event log" cells to store event data on-chain (rarely used due to cost).

**4. Witness Data**
Store event-like data in transaction witnesses for off-chain indexing.

### Off-chain Event Extraction

Build indexers that:
1. Monitor transactions involving your script
2. Parse transaction structure
3. Extract state changes
4. Store in database
5. Provide query APIs

---

## State Storage Patterns

### No Global Storage

CKB has no global contract storage. All state lives in individual cells.

### Storage Patterns

**Pattern 1: Individual Cells**
- Each user/entity gets their own cell
- Cell data contains their state
- Lock script determines ownership

**Pattern 2: Single Registry Cell**
- One cell stores packed data for multiple entries
- More efficient for small datasets
- Requires careful update logic

**Pattern 3: Cell Deps for Read-Only Data**
- Store reference data in separate cells
- Include as cell deps when needed
- Doesn't consume the cell

### State Updates

Updating state means:
1. Consume old cell (input)
2. Create new cell with updated data (output)
3. Script validates the transition is valid

### State Persistence

State persists as long as the cell exists. When a cell is consumed, its state is "destroyed" (but recorded in blockchain history).

---

## Timestamp Handling

### No Direct Block Timestamp Access

CKB scripts cannot directly access block timestamps like Ethereum's `block.timestamp`.

### Accessing Timestamps

**Method 1: Header Dependencies**
- Transaction includes current block header as header dep
- Script loads header and extracts timestamp
- Use `load_header()` with `Source::HeaderDep`
- Access timestamp via `header.raw().timestamp()`

**Method 2: Since Field**
- CKB's built-in time-lock mechanism
- Can represent absolute time or relative time
- Simpler but less flexible than header deps

**Method 3: Timestamp Oracles**
- External services provide signed timestamps
- More complex but most flexible
- Useful for precise time requirements

### Transaction Creator Responsibility

The transaction creator must include appropriate header dependencies for the script to access timestamps.

### Time-based Validation

Common patterns:
- Check if enough time has elapsed
- Validate time windows
- Enforce time-based constraints
- Calculate durations

---

## Cell Identification Patterns

### The Challenge

In a transaction with multiple cells, how do you identify which cell is which?

### Pattern 1: Iterate and Check Lock Scripts

Search through cells and check their lock scripts to find cells belonging to specific users.

### Pattern 2: Iterate and Check Type Scripts

Search through cells and check their type scripts to find cells of specific types (tokens, contracts, etc.).

### Pattern 3: Enforce Transaction Structure

Define a specification for cell positions and validate that the transaction follows it.

### Pattern 4: Pass Positions in Arguments

Include cell indices in function arguments to tell the script where to find specific cells.

### Pattern 5: Use Group Sources

Use `Source::GroupInput/GroupOutput` to automatically filter to your script's cells.

### Best Practice

Combine approaches:
1. Use Group sources when possible
2. Validate cell ownership via lock scripts
3. Identify cell types via type scripts
4. Don't assume fixed positions

### Helper Functions

Create helper functions to:
- Extract addresses from lock scripts
- Check if a cell belongs to a specific user
- Identify cell types
- Find cells matching criteria

---

## On-chain vs Off-chain Querying

### No On-chain Querying

**CKB scripts CANNOT:**
- Query arbitrary cells on the blockchain
- Search for cells by criteria
- Call other contracts
- Read global state
- Access data outside the current transaction

### Why No On-chain Querying?

**Determinism**
Scripts must be deterministic - same inputs always produce same outputs. Arbitrary queries would break this.

**Transaction-Scoped Execution**
Scripts only see the current transaction. They validate state transitions, not execute arbitrary logic.

### What Scripts CAN Access

Scripts can only access:
- Cells in the current transaction (inputs/outputs)
- Referenced cells (cell deps)
- Referenced headers (header deps)

### Off-chain Querying

All querying happens off-chain using RPC methods:
- `getTransaction` - Get transaction by hash
- `getLiveCell` - Get live cell by outpoint
- `getCells` - Search for cells by script
- Custom indexers for complex queries

### The CKB Pattern

```
OFF-CHAIN:
1. Query blockchain for data
2. Build transaction with necessary cells
3. Include required cells as inputs/deps
4. Submit transaction

ON-CHAIN:
1. Script validates transaction
2. Reads cells from inputs/outputs/deps
3. Validates state transitions
4. Cannot query external data
```

### Accessing Other Contract Data

**Option 1: Include as Cell Deps**
Include cells you need to read as cell dependencies (read-only).

**Option 2: Pass Data in Arguments**
Query data off-chain and pass it as function arguments.

**Option 3: Include as Inputs**
If you need to update the data, include the cell as an input.

---

## Campaign/Task System Design

### System Overview

A campaign/task system allows creating time-bound campaigns that accept deposits and distribute rewards.

### Core Components

**Campaign Cell**
- Stores campaign state in cell data
- Type script validates all operations
- Lock script controls who can spend it

**Campaign State**
- Creation timestamp
- Start duration (time until campaign starts)
- Task duration (how long campaign runs)
- Creator address
- Campaign type (enum)
- Maximum deposit amount
- Current deposits
- Status (Created, Active, Completed, Cancelled)

### Campaign Types

Different campaign types with different behaviors:
- Simple tasks (no deposits)
- Funded tasks (require deposits)
- Crowdfunding (deposit-based with goals)
- Timed challenges (time-sensitive with deposits)

### Operations

**Create Campaign**
- Transaction creates new campaign cell
- No input cells with campaign script (initialization)
- Validates creator authorization
- Sets initial state

**Deposit to Campaign**
- Transaction updates campaign cell
- Input: old campaign cell + depositor's tokens
- Output: updated campaign cell + remaining tokens
- Validates deposit doesn't exceed maximum
- Updates current deposits
- May trigger campaign start if funding goal reached

**Update Campaign**
- Only creator can update
- Can modify maximum amount and task duration
- Only allowed before task duration elapses
- Validates new values are reasonable

**Distribute Rewards**
- Triggered after campaign completes
- Validates distribution matches registrations
- Ensures all participants receive correct amounts

### Time-based Logic

**Campaign Start Conditions**
- Current time >= (created_at + start_duration), OR
- Current deposits >= maximum_amount

**Campaign End Condition**
- Current time >= (start_time + task_duration)

**Timestamp Access**
- Use header dependencies to get current time
- Transaction must include current block header
- Script extracts timestamp from header

### State Transitions

```
Created → Active (when started)
Active → Completed (when duration elapses)
Any → Cancelled (by creator)
```

### Validation Rules

**Deposit Validation**
- Campaign must accept deposits (based on type)
- Campaign must not be completed or cancelled
- Task duration must not have elapsed
- Deposit must not exceed maximum amount
- Depositor must authorize transaction
- Token transfer must actually occur

**Update Validation**
- Only creator can update
- Task duration must not have elapsed
- New maximum >= current deposits
- Only specific fields can be updated

### Cell Identification

**During Creation**
- No campaign cells in inputs
- First input cell = creator's cell

**During Deposit**
- Campaign cell in inputs (being updated)
- Need to find depositor's cell (non-campaign input)
- Skip campaign cells when searching for depositor

### Data Encoding

Campaign state is encoded as fixed-size byte array:
- Each field has specific byte range
- Use little-endian encoding for numbers
- Enums encoded as single bytes
- Total size: 62 bytes

### Off-chain Integration

**Query Campaign State**
- Use RPC to get campaign cell
- Parse cell data to extract state
- Monitor for updates

**Build Transactions**
- Query current state off-chain
- Validate operations off-chain
- Build transaction with correct structure
- Submit to network

**Monitor Campaigns**
- Poll for cell updates
- Track state changes
- Notify users of events

---

## Key Takeaways

### Fundamental Differences from Account-Based Chains

1. **No global state** - State lives in cells, not in contract storage
2. **No on-chain queries** - Scripts only see current transaction
3. **No events** - Transaction structure is the event
4. **No constructors** - Script args serve as constructor parameters
5. **Validation not execution** - Scripts validate transitions, don't execute them

### The UTXO Mental Model

- Think in terms of cells being consumed and created
- State transitions, not state mutations
- Transaction structure defines the operation
- Scripts validate the structure is correct

### Best Practices

1. **Always iterate through cells** - Don't assume positions
2. **Validate ownership** - Check lock scripts
3. **Identify by type** - Use type scripts to identify cell types
4. **Query off-chain** - Do all searching and preparation off-chain
5. **Include necessary cells** - Add cells you need as inputs or deps
6. **Use Group sources** - More efficient for filtering
7. **Handle timestamps properly** - Use header deps or since field
8. **Design for determinism** - Scripts must be deterministic

### Common Patterns

- **Registry pattern**: Individual cells for each entry
- **Packed data pattern**: Single cell with multiple entries
- **Cell deps pattern**: Reference data without consuming
- **Authorization pattern**: Check lock scripts of input cells
- **Time-lock pattern**: Use timestamps for time-based logic
- **State machine pattern**: Validate state transitions

---

## Resources

- [CKB Documentation](https://docs.nervos.org/)
- [ckb-std GitHub](https://github.com/nervosnetwork/ckb-std)
- [CKB Script Programming](https://docs.nervos.org/docs/script-course/intro-to-script-1)
- [CKB Authorization Guide](./CKB_AUTHORIZATION_GUIDE.md)
- [CKB Learning Guide](./CKB_LEARNING_GUIDE.md)

---

## Glossary

**Cell**: The basic unit of state in CKB, containing capacity, lock, type, and data

**Lock Script**: Determines who can spend/unlock a cell

**Type Script**: Defines the rules for creating and destroying cells of a specific type

**Source**: Enum indicating which set of cells to access (Input, Output, CellDep, etc.)

**Group Source**: Filtered view showing only cells with the same script

**Type Script Hash**: Hash of a type script, used as token identifier

**Script Args**: Immutable parameters passed to a script, like constructor arguments

**Cell Dep**: Referenced cell that can be read but not consumed

**Header Dep**: Referenced block header for accessing timestamps

**OutPoint**: Unique identifier for a cell (transaction hash + index)

**UTXO**: Unspent Transaction Output model, used by CKB

**Deterministic**: Always produces the same output for the same input

**State Transition**: Change from one state to another, validated by scripts
