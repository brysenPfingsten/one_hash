//! 1# Assembler
//!
//! Compiles assembly language to 1# machine code.
//!
//! ## Assembly Syntax
//!
//! - `add 1 R1` - Append 1 to register
//! - `add # R1` - Append # to register
//! - `case R1` - Branch on first symbol of register
//! - `goto label` - Jump to label
//! - `halt` - Stop execution
//! - `label:` - Define a label
//! - `; comment` - Comment
//! - `__RAW__ <1# code>` - Embed raw 1# code directly (used by macros)

pub mod macros;

use std::collections::HashMap;

use crate::instruction::Instruction;

pub use macros::{expand_macros, reset_label_counter};

/// Assembly instruction before label resolution.
#[derive(Debug, Clone)]
enum AsmInstruction {
    Add1(usize),
    AddHash(usize),
    Goto(String),
    Case(usize),
    Label(String),
}

/// Parses a register reference (R1, R2, etc.) and returns the register number.
fn parse_register(s: &str) -> Result<usize, String> {
    let s = s.trim();
    if let Some(num_str) = s.strip_prefix('R').or_else(|| s.strip_prefix('r')) {
        num_str
            .parse::<usize>()
            .map_err(|_| format!("Invalid register number: {}", s))
    } else {
        Err(format!("Expected register (R1, R2, ...), got: {}", s))
    }
}

/// Parses assembly source into a list of assembly instructions.
fn parse_assembly(source: &str) -> Result<Vec<AsmInstruction>, String> {
    let mut instructions = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let line_num = line_num + 1;
        let line = if let Some(idx) = line.find(';') {
            &line[..idx]
        } else {
            line
        };
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if line.ends_with(':') {
            let label = line[..line.len() - 1].trim().to_string();
            if label.is_empty() {
                return Err(format!("Line {}: Empty label", line_num));
            }
            instructions.push(AsmInstruction::Label(label));
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0].to_lowercase().as_str() {
            "add" => {
                if parts.len() != 3 {
                    return Err(format!(
                        "Line {}: 'add' expects 2 arguments: add <1|#> <register>",
                        line_num
                    ));
                }
                let reg =
                    parse_register(parts[2]).map_err(|e| format!("Line {}: {}", line_num, e))?;
                match parts[1] {
                    "1" => instructions.push(AsmInstruction::Add1(reg)),
                    "#" => instructions.push(AsmInstruction::AddHash(reg)),
                    _ => {
                        return Err(format!(
                            "Line {}: 'add' expects '1' or '#', got: {}",
                            line_num, parts[1]
                        ))
                    }
                }
            }
            "goto" => {
                if parts.len() != 2 {
                    return Err(format!(
                        "Line {}: 'goto' expects 1 argument: goto <label>",
                        line_num
                    ));
                }
                instructions.push(AsmInstruction::Goto(parts[1].to_string()));
            }
            "case" => {
                if parts.len() != 2 {
                    return Err(format!(
                        "Line {}: 'case' expects 1 argument: case <register>",
                        line_num
                    ));
                }
                let reg =
                    parse_register(parts[1]).map_err(|e| format!("Line {}: {}", line_num, e))?;
                instructions.push(AsmInstruction::Case(reg));
            }
            "halt" => {
                instructions.push(AsmInstruction::Goto("__halt__".to_string()));
            }
            _ => {
                return Err(format!(
                    "Line {}: Unknown instruction: {}",
                    line_num, parts[0]
                ))
            }
        }
    }

    Ok(instructions)
}

/// Resolves label references to forward/backward jump offsets.
fn resolve_labels(asm: Vec<AsmInstruction>) -> Result<Vec<Instruction>, String> {
    let mut labels: HashMap<String, usize> = HashMap::new();
    let mut instruction_count = 0;

    for instr in &asm {
        match instr {
            AsmInstruction::Label(name) => {
                if labels.contains_key(name) {
                    return Err(format!("Duplicate label: {}", name));
                }
                labels.insert(name.clone(), instruction_count);
            }
            _ => {
                instruction_count += 1;
            }
        }
    }

    labels.insert("__halt__".to_string(), instruction_count);

    let mut result = Vec::new();
    let mut current_pos = 0;

    for instr in asm {
        match instr {
            AsmInstruction::Label(_) => {}
            AsmInstruction::Add1(reg) => {
                result.push(Instruction::AddOne(reg));
                current_pos += 1;
            }
            AsmInstruction::AddHash(reg) => {
                result.push(Instruction::AddHash(reg));
                current_pos += 1;
            }
            AsmInstruction::Case(reg) => {
                result.push(Instruction::Case(reg));
                current_pos += 1;
            }
            AsmInstruction::Goto(label) => {
                let target = labels
                    .get(&label)
                    .ok_or_else(|| format!("Undefined label: {}", label))?;

                if *target > current_pos {
                    result.push(Instruction::Forward(target - current_pos));
                } else if *target < current_pos {
                    result.push(Instruction::Backward(current_pos - target));
                } else {
                    return Err(format!(
                        "Cannot goto the same instruction position (label: {})",
                        label
                    ));
                }
                current_pos += 1;
            }
        }
    }

    Ok(result)
}

/// Extracts __RAW__ directives and separates them from regular assembly.
/// Returns (raw_code_segments, remaining_assembly).
fn extract_raw_code(source: &str) -> (Vec<(usize, String)>, String) {
    let mut raw_segments: Vec<(usize, String)> = Vec::new();
    let mut assembly_lines: Vec<String> = Vec::new();
    let mut line_number = 0;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("__RAW__") {
            let raw_code = trimmed.strip_prefix("__RAW__").unwrap().trim().to_string();
            // Insert a placeholder to track position
            assembly_lines.push(format!("__RAW_PLACEHOLDER_{}__:", raw_segments.len()));
            raw_segments.push((line_number, raw_code));
        } else {
            assembly_lines.push(line.to_string());
        }
        line_number += 1;
    }

    (raw_segments, assembly_lines.join("\n"))
}

/// Compiles assembly source to 1# code.
pub fn compile(source: &str) -> Result<String, String> {
    reset_label_counter();
    let expanded = expand_macros(source)?;

    // Extract raw code segments
    let (raw_segments, assembly_only) = extract_raw_code(&expanded);

    if raw_segments.is_empty() {
        // No raw code, compile normally
        let asm = parse_assembly(&assembly_only)?;
        let instructions = resolve_labels(asm)?;
        Ok(instructions
            .iter()
            .map(|i| i.to_one_hash())
            .collect::<Vec<_>>()
            .join(""))
    } else if assembly_only.lines().all(|l| {
        let t = l.trim();
        t.is_empty() || t.starts_with(';') || t.starts_with("__RAW_PLACEHOLDER_")
    }) {
        // Only raw code, no assembly to compile
        Ok(raw_segments
            .into_iter()
            .map(|(_, code)| code)
            .collect::<Vec<_>>()
            .join(""))
    } else {
        // Mixed: for now, require raw code to be standalone (no mixing)
        // This is a simplification - raw macros should be used alone
        Err("Cannot mix __RAW__ directives with regular assembly. Raw macros must be used standalone.".to_string())
    }
}

/// Compiles assembly source to 1# code with verbose output.
pub fn compile_verbose(source: &str) -> Result<String, String> {
    reset_label_counter();
    let expanded = expand_macros(source)?;

    println!("Expanded macros ({} lines):", expanded.lines().count());
    for (i, line) in expanded.lines().enumerate() {
        if !line.trim().is_empty() {
            println!("  {}: {}", i + 1, line);
        }
    }
    println!();

    // Extract raw code segments
    let (raw_segments, assembly_only) = extract_raw_code(&expanded);

    if raw_segments.is_empty() {
        let asm = parse_assembly(&assembly_only)?;
        let instructions = resolve_labels(asm)?;

        println!("Resolved ({} instructions)", instructions.len());
        println!();

        Ok(instructions
            .iter()
            .map(|i| i.to_one_hash())
            .collect::<Vec<_>>()
            .join(""))
    } else if assembly_only.lines().all(|l| {
        let t = l.trim();
        t.is_empty() || t.starts_with(';') || t.starts_with("__RAW_PLACEHOLDER_")
    }) {
        println!("Raw 1# code ({} segments)", raw_segments.len());
        println!();
        Ok(raw_segments
            .into_iter()
            .map(|(_, code)| code)
            .collect::<Vec<_>>()
            .join(""))
    } else {
        Err("Cannot mix __RAW__ directives with regular assembly. Raw macros must be used standalone.".to_string())
    }
}

/// Expands macros only without compiling.
pub fn expand_only(source: &str) -> Result<String, String> {
    reset_label_counter();
    expand_macros(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_compile() {
        let code = compile("add 1 R1\nadd # R2").unwrap();
        assert_eq!(code, "1#11##");
    }

    #[test]
    fn test_labels() {
        let code = compile("start:\n    add 1 R1\n    goto start").unwrap();
        // add 1 R1 = 1#, goto start (backward 1) = 1####
        assert_eq!(code, "1#1####");
    }

    #[test]
    fn test_case() {
        let code = compile("case R1").unwrap();
        assert_eq!(code, "1#####");
    }

    #[test]
    fn test_halt() {
        let code = compile("add 1 R1\nhalt").unwrap();
        // add 1 R1 = 1#, halt (forward 1) = 1###
        assert_eq!(code, "1#1###");
    }

    #[test]
    fn test_clear_macro() {
        let code = compile("clear R1").unwrap();
        assert!(!code.is_empty());
    }

    #[test]
    fn test_move_macro() {
        let code = compile("move R1 R2").unwrap();
        assert!(!code.is_empty());
    }
}
