// Error codes for the contract
#[repr(i8)]
#[derive(Debug, Clone, Copy)]
pub enum Error {
    LoadScriptFailed = 1,
    NoFunctionSelector = 2,
    UnknownFunction = 3,
    InvalidArgs = 4,
    InsufficientBalance = 5,
    Unauthorized = 6,
    InvalidCellData = 7,
    AmountMismatch = 8,
}
