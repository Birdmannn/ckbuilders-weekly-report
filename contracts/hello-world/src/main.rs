#![cfg_attr(not(any(feature = "library", test)), no_std)]
#![cfg_attr(not(test), no_main)]
#[cfg(any(feature = "library", test))]
extern crate alloc;

mod errors;

use ckb_std::ckb_constants::Source;
use ckb_std::ckb_types::prelude::*;
use ckb_std::high_level::{load_cell_data, load_cell_lock, load_script};
use errors::Error;

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
// By default, the following heap configuration is used:
// * 16KB fixed heap
// * 1.2MB(rounded up to be 16-byte aligned) dynamic heap
// * Minimal memory block in dynamic heap is 64 bytes
// For more details, please refer to ckb-std's default_alloc macro
// and the buddy-alloc alloc implementation.
ckb_std::default_alloc!(16384, 1258306, 64);

// Token data structure stored in cell data
// Format: [amount: 8 bytes (u64)]
const TOKEN_DATA_LEN: usize = 8;

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
    // Load the script to get arguments
    let script = load_script().map_err(|_| Error::LoadScriptFailed)?;

    let args = script.args().raw_data();

    // Check if we have at least one byte for function selector
    if args.is_empty() {
        return Err(Error::NoFunctionSelector);
    }

    // First byte is the function selector
    let function_selector = args[0];

    ckb_std::debug!("Function selector: {}", function_selector);

    // Dispatch to different functions based on selector
    match function_selector {
        0 => handle_transfer(&args[1..]),
        1 => handle_mint(&args[1..]),
        2 => handle_burn(&args[1..]),
        3 => handle_query(&args[1..]),
        _ => Err(Error::UnknownFunction),
    }
}

// Helper: Read token amount from cell data
fn read_token_amount(data: &[u8]) -> Result<u64, Error> {
    if data.len() < TOKEN_DATA_LEN {
        return Err(Error::InvalidCellData);
    }

    Ok(u64::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ]))
}

// Helper: Get total token amount from cells in a source (Input or Output)
fn get_total_amount(source: Source) -> Result<u64, Error> {
    let mut total = 0u64;
    let mut i = 0;

    loop {
        // Try to load cell data
        match load_cell_data(i, source) {
            Ok(data) => {
                // Parse the token amount from cell data
                let amount = read_token_amount(&data)?;
                total = total.checked_add(amount).ok_or(Error::AmountMismatch)?;

                ckb_std::debug!("Cell {} in {:?}: {} tokens", i, source, amount);
                i += 1;
            }
            Err(_) => break, // No more cells
        }
    }

    ckb_std::debug!("Total in {:?}: {} tokens", source, total);
    Ok(total)
}

// Helper: Count cells controlled by current script
fn count_script_cells(source: Source) -> Result<usize, Error> {
    let current_script = load_script().map_err(|_| Error::LoadScriptFailed)?;
    let current_script_hash = current_script.calc_script_hash();

    let mut count = 0;
    let mut i = 0;

    loop {
        match load_cell_lock(i, source) {
            Ok(lock) => {
                let lock_hash = lock.calc_script_hash();
                if lock_hash == current_script_hash {
                    count += 1;
                }
                i += 1;
            }
            Err(_) => break,
        }
    }

    Ok(count)
}

fn handle_transfer(args: &[u8]) -> Result<(), Error> {
    ckb_std::debug!("=== TRANSFER ===");

    // Parse transfer arguments (expecting 8 bytes for amount)
    if args.len() < 8 {
        return Err(Error::InvalidArgs);
    }

    let transfer_amount = u64::from_le_bytes([
        args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
    ]);

    ckb_std::debug!("Transfer amount: {}", transfer_amount);

    // READING STATE: Get total tokens from input cells (being consumed)
    let input_total = get_total_amount(Source::Input)?;
    ckb_std::debug!("Input total: {}", input_total);

    // READING STATE: Get total tokens from output cells (being created)
    let output_total = get_total_amount(Source::Output)?;
    ckb_std::debug!("Output total: {}", output_total);

    // VALIDATION: Ensure no tokens are created or destroyed
    // In a real transfer, input_total should equal output_total
    // (tokens just move between cells, total stays the same)
    if input_total != output_total {
        ckb_std::debug!(
            "Amount mismatch! Input: {}, Output: {}",
            input_total,
            output_total
        );
        return Err(Error::AmountMismatch);
    }

    // Additional validation: Check that we have enough balance
    if input_total < transfer_amount {
        return Err(Error::InsufficientBalance);
    }

    ckb_std::debug!("Transfer validation passed!");
    Ok(())
}

fn handle_mint(args: &[u8]) -> Result<(), Error> {
    ckb_std::debug!("=== MINT ===");

    if args.len() < 8 {
        return Err(Error::InvalidArgs);
    }

    let mint_amount = u64::from_le_bytes([
        args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
    ]);

    ckb_std::debug!("Mint amount: {}", mint_amount);

    // READING STATE: Get totals from inputs and outputs
    let input_total = get_total_amount(Source::Input)?;
    let output_total = get_total_amount(Source::Output)?;

    ckb_std::debug!(
        "Input total: {}, Output total: {}",
        input_total,
        output_total
    );

    // VALIDATION: For minting, output should be input + mint_amount
    let expected_output = input_total
        .checked_add(mint_amount)
        .ok_or(Error::AmountMismatch)?;

    if output_total != expected_output {
        ckb_std::debug!(
            "Mint validation failed! Expected: {}, Got: {}",
            expected_output,
            output_total
        );
        return Err(Error::AmountMismatch);
    }

    // In a real contract, you'd also check:
    // - Is the caller authorized to mint?
    // - Is there a max supply limit?
    // - Are the new tokens going to the right address?

    ckb_std::debug!("Mint validation passed!");
    Ok(())
}

fn handle_burn(args: &[u8]) -> Result<(), Error> {
    ckb_std::debug!("=== BURN ===");

    if args.len() < 8 {
        return Err(Error::InvalidArgs);
    }

    let burn_amount = u64::from_le_bytes([
        args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
    ]);

    ckb_std::debug!("Burn amount: {}", burn_amount);

    // READING STATE: Get totals
    let input_total = get_total_amount(Source::Input)?;
    let output_total = get_total_amount(Source::Output)?;

    ckb_std::debug!(
        "Input total: {}, Output total: {}",
        input_total,
        output_total
    );

    // VALIDATION: For burning, output should be input - burn_amount
    let expected_output = input_total
        .checked_sub(burn_amount)
        .ok_or(Error::InsufficientBalance)?;

    if output_total != expected_output {
        ckb_std::debug!(
            "Burn validation failed! Expected: {}, Got: {}",
            expected_output,
            output_total
        );
        return Err(Error::AmountMismatch);
    }

    // In a real contract, you'd also check:
    // - Does the caller own the tokens being burned?
    // - Are we burning from the correct cells?

    ckb_std::debug!("Burn validation passed!");
    Ok(())
}

fn handle_query(args: &[u8]) -> Result<(), Error> {
    ckb_std::debug!("=== QUERY ===");

    // READING STATE: Query current state without validation
    ckb_std::debug!("Query args length: {}", args.len());

    // Example queries:

    // 1. Get total supply (sum of all output cells)
    let total_supply = get_total_amount(Source::Output)?;
    ckb_std::debug!("Total supply: {}", total_supply);

    // 2. Count how many cells are controlled by this script
    let input_cell_count = count_script_cells(Source::Input)?;
    let output_cell_count = count_script_cells(Source::Output)?;
    ckb_std::debug!(
        "Input cells: {}, Output cells: {}",
        input_cell_count,
        output_cell_count
    );

    // 3. Read individual cell amounts
    let mut i = 0;
    loop {
        match load_cell_data(i, Source::Output) {
            Ok(data) => {
                let amount = read_token_amount(&data)?;
                ckb_std::debug!("Output cell {}: {} tokens", i, amount);
                i += 1;
            }
            Err(_) => break,
        }
    }

    Ok(())
}
