//! # one_hash
//!
//! A 1# (one-hash) language interpreter and assembler.
//!
//! The 1# language is a minimal Text Register Machine with only two symbols:
//! `1` (one) and `#` (hash). Despite this simplicity, it is Turing-complete.
//!
//! ## Language Overview
//!
//! ### Symbols
//! - `1` - The number one
//! - `#` - The hash symbol
//!
//! ### Instructions
//! All instructions follow the pattern `1^n` followed by 1-5 `#` symbols:
//!
//! | Instruction | Encoding | Effect |
//! |-------------|----------|--------|
//! | AddOne(n) | `1^n#` | Append '1' to register n |
//! | AddHash(n) | `1^n##` | Append '#' to register n |
//! | Forward(n) | `1^n###` | Jump forward n instructions |
//! | Backward(n) | `1^n####` | Jump backward n instructions |
//! | Case(n) | `1^n#####` | Branch on first symbol of register n |
//!
//! ### Data Encoding
//! Numbers are encoded in "backwards binary":
//! - LSB is first
//! - `1` = binary 1
//! - `#` = binary 0
//!
//! Example: `#11` = 110 binary = 6 decimal
//!
//! ## Modules
//!
//! - [`types`] - Core types (Symbol, Word)
//! - [`instruction`] - Instruction definitions
//! - [`parser`] - 1# program parser
//! - [`machine`] - Virtual machine
//! - [`assembler`] - Assembly language compiler

pub mod assembler;
pub mod instruction;
pub mod machine;
pub mod parser;
pub mod types;

pub use assembler::{compile, compile_verbose, expand_only};
pub use instruction::Instruction;
pub use machine::{Machine, RunResult, StepResult};
pub use parser::{parse_program, print_parsed_program};
pub use types::{format_word, string_to_word, word_to_decimal, word_to_string, Symbol, Word};
