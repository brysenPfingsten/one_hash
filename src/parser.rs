//! Parser for 1# programs.
//!
//! Parses raw 1# source code into a list of instructions.

use crate::instruction::Instruction;

/// Parses a 1# program into a list of instructions.
///
/// The parser handles:
/// - Whitespace (ignored)
/// - Comments (semicolon to end of line)
/// - The two valid symbols: `1` and `#`
pub fn parse_program(source: &str) -> Result<Vec<Instruction>, String> {
    let mut instructions = Vec::new();
    let mut ones = 0;
    let mut hashes = 0;
    let mut in_comment = false;

    for c in source.chars() {
        // Handle comments (semicolon to end of line)
        if c == ';' {
            in_comment = true;
            continue;
        }
        if c == '\n' {
            in_comment = false;
            continue;
        }
        if in_comment {
            continue;
        }

        // Skip whitespace
        if c.is_whitespace() {
            continue;
        }

        match c {
            '1' => {
                // If we had accumulated hashes, that means we finished an instruction
                if hashes > 0 {
                    if let Some(instr) = make_instruction(ones, hashes)? {
                        instructions.push(instr);
                    }
                    ones = 0;
                    hashes = 0;
                }
                ones += 1;
            }
            '#' => {
                hashes += 1;
            }
            _ => {
                // Ignore other characters
            }
        }
    }

    // Handle final instruction
    if hashes > 0 {
        if let Some(instr) = make_instruction(ones, hashes)? {
            instructions.push(instr);
        }
    }

    Ok(instructions)
}

/// Creates an instruction from a count of ones and hashes.
fn make_instruction(ones: usize, hashes: usize) -> Result<Option<Instruction>, String> {
    if ones == 0 {
        return Err(format!(
            "Invalid instruction: {} hashes with no ones",
            hashes
        ));
    }

    match hashes {
        0 => Ok(None), // No instruction yet
        1 => Ok(Some(Instruction::AddOne(ones))),
        2 => Ok(Some(Instruction::AddHash(ones))),
        3 => Ok(Some(Instruction::Forward(ones))),
        4 => Ok(Some(Instruction::Backward(ones))),
        5 => Ok(Some(Instruction::Case(ones))),
        _ => Err(format!(
            "Invalid instruction: {} ones followed by {} hashes (max 5 hashes allowed)",
            ones, hashes
        )),
    }
}

/// Prints a parsed program in a human-readable format.
pub fn print_parsed_program(instructions: &[Instruction]) {
    println!("Parsed program ({} instructions):", instructions.len());
    for (i, instr) in instructions.iter().enumerate() {
        println!("  {}: {} ({})", i + 1, instr.to_one_hash(), instr.describe());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let instrs = parse_program("1#").unwrap();
        assert_eq!(instrs.len(), 1);
        assert_eq!(instrs[0], Instruction::AddOne(1));
    }

    #[test]
    fn test_parse_multiple() {
        let instrs = parse_program("1#11##11##111##").unwrap();
        assert_eq!(instrs.len(), 4);
        assert_eq!(instrs[0], Instruction::AddOne(1));
        assert_eq!(instrs[1], Instruction::AddHash(2));
        assert_eq!(instrs[2], Instruction::AddHash(2));
        assert_eq!(instrs[3], Instruction::AddHash(3));
    }

    #[test]
    fn test_parse_all_instruction_types() {
        let instrs = parse_program("1# 11## 111### 1111#### 11111#####").unwrap();
        assert_eq!(instrs.len(), 5);
        assert_eq!(instrs[0], Instruction::AddOne(1));
        assert_eq!(instrs[1], Instruction::AddHash(2));
        assert_eq!(instrs[2], Instruction::Forward(3));
        assert_eq!(instrs[3], Instruction::Backward(4));
        assert_eq!(instrs[4], Instruction::Case(5));
    }

    #[test]
    fn test_comments() {
        let instrs = parse_program("1# ; add 1 to R1\n11## ; add # to R2").unwrap();
        assert_eq!(instrs.len(), 2);
        assert_eq!(instrs[0], Instruction::AddOne(1));
        assert_eq!(instrs[1], Instruction::AddHash(2));
    }

    #[test]
    fn test_whitespace() {
        let instrs = parse_program("  1  #  11 ##  ").unwrap();
        assert_eq!(instrs.len(), 2);
        assert_eq!(instrs[0], Instruction::AddOne(1));
        assert_eq!(instrs[1], Instruction::AddHash(2));
    }

    #[test]
    fn test_invalid_too_many_hashes() {
        let result = parse_program("1######");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_hashes_without_ones() {
        let result = parse_program("##");
        assert!(result.is_err());
    }
}
