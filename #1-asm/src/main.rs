//! # 1# Assembler
//!
//! An assembler for the 1# (one-hash) language, a Text Register Machine.
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
//!
//! ## Built-in Macros
//!
//! | Macro | Arguments | Effect |
//! |-------|-----------|--------|
//! | `clear` | reg | Sets reg to empty |
//! | `move` | src dst | Moves src to dst, empties src |
//! | `copy` | src dst tmp | Copies src to dst using tmp, preserves src |
//! | `shift_left` | reg tmp | Prepends # to reg (multiplies by 2 in binary) |
//! | `decrement` | reg tmp | Subtracts 1 from reg (binary arithmetic) |
//! | `is_nonzero` | reg tmp1 tmp2 nz_lbl z_lbl | Jumps to nz_lbl if reg != 0, else z_lbl |
//! | `bin_add` | src dst tmp1 tmp2 carry | Adds src to dst (binary), consumes src |
//!
//! ## User-Defined Macros
//!
//! ```text
//! .macro name param1 param2 ...
//!     <body>
//! .endmacro
//! ```
//!
//! Use `@label` for local labels that are unique per invocation.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

static LABEL_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generates a unique label with the given prefix.
fn unique_label(prefix: &str) -> String {
    let n = LABEL_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("__{}_{}__", prefix, n)
}

/// A user-defined macro with parameters and body.
#[derive(Debug, Clone)]
struct UserMacro {
    name: String,
    params: Vec<String>,
    body: Vec<String>,
}

/// Extracts macro definitions from source, returning macros and remaining source.
fn parse_macro_definitions(source: &str) -> Result<(HashMap<String, UserMacro>, String), String> {
    let mut macros: HashMap<String, UserMacro> = HashMap::new();
    let mut output_lines = Vec::new();
    let mut in_macro = false;
    let mut current_macro: Option<(String, Vec<String>, Vec<String>)> = None;
    let mut macro_start_line = 0;

    for (line_num, line) in source.lines().enumerate() {
        let line_num = line_num + 1;
        let code = if let Some(idx) = line.find(';') {
            &line[..idx]
        } else {
            line
        };
        let trimmed = code.trim();

        if trimmed.to_lowercase().starts_with(".macro")
            || trimmed.to_lowercase().starts_with(".def")
        {
            if in_macro {
                return Err(format!(
                    "Line {}: Nested macro definitions not allowed",
                    line_num
                ));
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(format!("Line {}: .macro requires a name", line_num));
            }
            let name = parts[1].to_lowercase();
            let params: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();
            in_macro = true;
            macro_start_line = line_num;
            current_macro = Some((name, params, Vec::new()));
        } else if trimmed.to_lowercase() == ".endmacro" || trimmed.to_lowercase() == ".end" {
            if !in_macro {
                return Err(format!(
                    "Line {}: .endmacro without matching .macro",
                    line_num
                ));
            }
            if let Some((name, params, body)) = current_macro.take() {
                if macros.contains_key(&name) {
                    return Err(format!(
                        "Line {}: Duplicate macro definition: {}",
                        macro_start_line, name
                    ));
                }
                macros.insert(name.clone(), UserMacro { name, params, body });
            }
            in_macro = false;
        } else if in_macro {
            if let Some((_, _, ref mut body)) = current_macro {
                body.push(line.to_string());
            }
        } else {
            output_lines.push(line.to_string());
        }
    }

    if in_macro {
        return Err(format!(
            "Line {}: Unclosed macro definition",
            macro_start_line
        ));
    }

    Ok((macros, output_lines.join("\n")))
}

/// Expands a user-defined macro invocation with the given arguments.
fn expand_user_macro(
    mac: &UserMacro,
    args: &[&str],
    line_num: usize,
) -> Result<Vec<String>, String> {
    if args.len() != mac.params.len() {
        return Err(format!(
            "Line {}: Macro '{}' expects {} arguments, got {}",
            line_num,
            mac.name,
            mac.params.len(),
            args.len()
        ));
    }

    let mut subs: HashMap<&str, &str> = HashMap::new();
    for (param, arg) in mac.params.iter().zip(args.iter()) {
        subs.insert(param.as_str(), *arg);
    }

    let prefix = unique_label(&mac.name);
    let mut output = Vec::new();

    for body_line in &mac.body {
        let mut new_line = body_line.clone();

        for (param, arg) in &subs {
            let mut result = String::new();
            let mut chars = new_line.chars().peekable();
            let mut current_word = String::new();

            while let Some(c) = chars.next() {
                if c.is_alphanumeric() || c == '_' {
                    current_word.push(c);
                } else {
                    if !current_word.is_empty() {
                        if current_word == *param {
                            result.push_str(arg);
                        } else {
                            result.push_str(&current_word);
                        }
                        current_word.clear();
                    }
                    result.push(c);
                }
            }
            if !current_word.is_empty() {
                if current_word == *param {
                    result.push_str(arg);
                } else {
                    result.push_str(&current_word);
                }
            }
            new_line = result;
        }

        let trimmed = new_line.trim();
        if trimmed.starts_with('@') {
            let mut result = String::new();
            let mut in_local = false;
            let mut local_name = String::new();

            for c in new_line.chars() {
                if c == '@' {
                    in_local = true;
                    local_name.clear();
                } else if in_local {
                    if c.is_alphanumeric() || c == '_' {
                        local_name.push(c);
                    } else {
                        result.push_str(&format!("{}{}", prefix, local_name));
                        result.push(c);
                        in_local = false;
                        local_name.clear();
                    }
                } else {
                    result.push(c);
                }
            }
            if in_local && !local_name.is_empty() {
                result.push_str(&format!("{}{}", prefix, local_name));
            }
            new_line = result;
        } else {
            let mut result = String::new();
            let mut chars = new_line.chars().peekable();

            while let Some(c) = chars.next() {
                if c == '@' {
                    let mut local_name = String::new();
                    while let Some(&nc) = chars.peek() {
                        if nc.is_alphanumeric() || nc == '_' {
                            local_name.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    result.push_str(&format!("{}{}", prefix, local_name));
                } else {
                    result.push(c);
                }
            }
            new_line = result;
        }

        output.push(new_line);
    }

    Ok(output)
}

/// Expands all macros (user-defined and built-in) in the source code.
///
/// Performs multiple passes to handle nested macro calls.
fn expand_macros(source: &str) -> Result<String, String> {
    let (user_macros, source_without_defs) = parse_macro_definitions(source)?;
    let mut current = source_without_defs;
    let max_passes = 10;

    for pass in 0..max_passes {
        let mut output = Vec::new();
        let mut changed = false;

        for (line_num, line) in current.lines().enumerate() {
            let line_num = line_num + 1;
            let (code, _) = if let Some(idx) = line.find(';') {
                (&line[..idx], &line[idx..])
            } else {
                (line, "")
            };

            let trimmed = code.trim();
            if trimmed.is_empty() || trimmed.ends_with(':') {
                output.push(line.to_string());
                continue;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.is_empty() {
                output.push(line.to_string());
                continue;
            }

            let cmd = parts[0].to_lowercase();

            if let Some(mac) = user_macros.get(&cmd) {
                let expanded = expand_user_macro(mac, &parts[1..], line_num)?;
                output.extend(expanded);
                changed = true;
                continue;
            }

            match cmd.as_str() {
                "clear" => {
                    if parts.len() != 2 {
                        return Err(format!(
                            "Line {}: 'clear' expects 1 argument: clear <reg>",
                            line_num
                        ));
                    }
                    let reg = parts[1];
                    let lbl = unique_label("clr");
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", reg));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "move" => {
                    if parts.len() != 3 {
                        return Err(format!(
                            "Line {}: 'move' expects 2 arguments: move <src> <dst>",
                            line_num
                        ));
                    }
                    let (src, dst) = (parts[1], parts[2]);
                    let lbl = unique_label("mv");
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", src));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_one", lbl));
                    output.push(format!("    goto {}_hash", lbl));
                    output.push(format!("{}_one:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_hash:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "copy" => {
                    if parts.len() != 4 {
                        return Err(format!(
                            "Line {}: 'copy' expects 3 arguments: copy <src> <dst> <tmp>",
                            line_num
                        ));
                    }
                    let (src, dst, tmp) = (parts[1], parts[2], parts[3]);
                    let lbl = unique_label("cp");
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", src));
                    output.push(format!("    goto {}_restore", lbl));
                    output.push(format!("    goto {}_one", lbl));
                    output.push(format!("    goto {}_hash", lbl));
                    output.push(format!("{}_one:", lbl));
                    output.push(format!("    add 1 {}", tmp));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_hash:", lbl));
                    output.push(format!("    add # {}", tmp));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_restore:", lbl));
                    output.push(format!("    case {}", tmp));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_r1", lbl));
                    output.push(format!("    goto {}_rh", lbl));
                    output.push(format!("{}_r1:", lbl));
                    output.push(format!("    add 1 {}", src));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_restore", lbl));
                    output.push(format!("{}_rh:", lbl));
                    output.push(format!("    add # {}", src));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_restore", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "shift_left" => {
                    if parts.len() != 3 {
                        return Err(format!(
                            "Line {}: 'shift_left' expects 2 arguments: shift_left <reg> <tmp>",
                            line_num
                        ));
                    }
                    let (reg, tmp) = (parts[1], parts[2]);
                    let lbl = unique_label("shl");
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", reg));
                    output.push(format!("    goto {}_ins", lbl));
                    output.push(format!("    goto {}_one", lbl));
                    output.push(format!("    goto {}_hash", lbl));
                    output.push(format!("{}_one:", lbl));
                    output.push(format!("    add 1 {}", tmp));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_hash:", lbl));
                    output.push(format!("    add # {}", tmp));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_ins:", lbl));
                    output.push(format!("    add # {}", reg));
                    output.push(format!("{}_rest:", lbl));
                    output.push(format!("    case {}", tmp));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_r1", lbl));
                    output.push(format!("    goto {}_rh", lbl));
                    output.push(format!("{}_r1:", lbl));
                    output.push(format!("    add 1 {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("{}_rh:", lbl));
                    output.push(format!("    add # {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "decrement" => {
                    if parts.len() != 3 {
                        return Err(format!(
                            "Line {}: 'decrement' expects 2 arguments: decrement <reg> <tmp>",
                            line_num
                        ));
                    }
                    let (reg, tmp) = (parts[1], parts[2]);
                    let lbl = unique_label("dec");
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("    goto {}_one", lbl));
                    output.push(format!("    goto {}_hash", lbl));
                    output.push(format!("{}_one:", lbl));
                    output.push(format!("    add # {}", tmp));
                    output.push(format!("    goto {}_copy", lbl));
                    output.push(format!("{}_hash:", lbl));
                    output.push(format!("    add 1 {}", tmp));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_copy:", lbl));
                    output.push(format!("    case {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("    goto {}_c1", lbl));
                    output.push(format!("    goto {}_ch", lbl));
                    output.push(format!("{}_c1:", lbl));
                    output.push(format!("    add 1 {}", tmp));
                    output.push(format!("    goto {}_copy", lbl));
                    output.push(format!("{}_ch:", lbl));
                    output.push(format!("    add # {}", tmp));
                    output.push(format!("    goto {}_copy", lbl));
                    output.push(format!("{}_rest:", lbl));
                    output.push(format!("    case {}", tmp));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_r1", lbl));
                    output.push(format!("    goto {}_rh", lbl));
                    output.push(format!("{}_r1:", lbl));
                    output.push(format!("    add 1 {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("{}_rh:", lbl));
                    output.push(format!("    add # {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "is_nonzero" => {
                    if parts.len() != 6 {
                        return Err(format!(
                            "Line {}: 'is_nonzero' expects 5 arguments: is_nonzero <reg> <tmp1> <tmp2> <nonzero> <zero>",
                            line_num
                        ));
                    }
                    let (reg, tmp1, tmp2, nonzero, zero) =
                        (parts[1], parts[2], parts[3], parts[4], parts[5]);
                    let lbl = unique_label("isz");
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", reg));
                    output.push(format!("    goto {}_restore", lbl));
                    output.push(format!("    goto {}_one", lbl));
                    output.push(format!("    goto {}_hash", lbl));
                    output.push(format!("{}_one:", lbl));
                    output.push(format!("    add 1 {}", tmp1));
                    output.push(format!("    add 1 {}", tmp2));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_hash:", lbl));
                    output.push(format!("    add # {}", tmp1));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_restore:", lbl));
                    output.push(format!("    case {}", tmp1));
                    output.push(format!("    goto {}_check", lbl));
                    output.push(format!("    goto {}_r1", lbl));
                    output.push(format!("    goto {}_rh", lbl));
                    output.push(format!("{}_r1:", lbl));
                    output.push(format!("    add 1 {}", reg));
                    output.push(format!("    goto {}_restore", lbl));
                    output.push(format!("{}_rh:", lbl));
                    output.push(format!("    add # {}", reg));
                    output.push(format!("    goto {}_restore", lbl));
                    output.push(format!("{}_check:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}", zero));
                    output.push(format!("    goto {}_drain", lbl));
                    output.push(format!("    goto {}_drain", lbl));
                    output.push(format!("{}_drain:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}", nonzero));
                    output.push(format!("    goto {}_drain", lbl));
                    output.push(format!("    goto {}_drain", lbl));
                    changed = true;
                }
                "bin_add" => {
                    if parts.len() != 6 {
                        return Err(format!(
                            "Line {}: 'bin_add' expects 5 arguments: bin_add <src> <dst> <tmp1> <tmp2> <carry>",
                            line_num
                        ));
                    }
                    let (src, dst, tmp1, carry) = (parts[1], parts[2], parts[3], parts[5]);
                    let lbl = unique_label("badd");

                    output.push(format!("{}_mvdst:", lbl));
                    output.push(format!("    case {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("    goto {}_mv1", lbl));
                    output.push(format!("    goto {}_mvh", lbl));
                    output.push(format!("{}_mv1:", lbl));
                    output.push(format!("    add 1 {}", tmp1));
                    output.push(format!("    goto {}_mvdst", lbl));
                    output.push(format!("{}_mvh:", lbl));
                    output.push(format!("    add # {}", tmp1));
                    output.push(format!("    goto {}_mvdst", lbl));
                    output.push(format!("{}_loop:", lbl));
                    output.push(format!("    case {}", tmp1));
                    output.push(format!("    goto {}_t1e", lbl));
                    output.push(format!("    goto {}_t1_1", lbl));
                    output.push(format!("    goto {}_t1_h", lbl));
                    output.push(format!("{}_t1e:", lbl));
                    output.push(format!("    case {}", src));
                    output.push(format!("    goto {}_both_e", lbl));
                    output.push(format!("    goto {}_01", lbl));
                    output.push(format!("    goto {}_00", lbl));
                    output.push(format!("{}_both_e:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_addcarry", lbl));
                    output.push(format!("    goto {}_addcarry", lbl));
                    output.push(format!("{}_addcarry:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("{}_01:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_01_c0", lbl));
                    output.push(format!("    goto {}_01_c1", lbl));
                    output.push(format!("    goto {}_01_c1", lbl));
                    output.push(format!("{}_01_c0:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_01_c1:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    add 1 {}", carry));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_00:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_00_c0", lbl));
                    output.push(format!("    goto {}_00_c1", lbl));
                    output.push(format!("    goto {}_00_c1", lbl));
                    output.push(format!("{}_00_c0:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_00_c1:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_t1_1:", lbl));
                    output.push(format!("    case {}", src));
                    output.push(format!("    goto {}_10", lbl));
                    output.push(format!("    goto {}_11", lbl));
                    output.push(format!("    goto {}_1h", lbl));
                    output.push(format!("{}_10:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_10_c0", lbl));
                    output.push(format!("    goto {}_10_c1", lbl));
                    output.push(format!("    goto {}_10_c1", lbl));
                    output.push(format!("{}_10_c0:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_10_c1:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    add 1 {}", carry));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_11:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_11_c0", lbl));
                    output.push(format!("    goto {}_11_c1", lbl));
                    output.push(format!("    goto {}_11_c1", lbl));
                    output.push(format!("{}_11_c0:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    add 1 {}", carry));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_11_c1:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    add 1 {}", carry));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_1h:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_1h_c0", lbl));
                    output.push(format!("    goto {}_1h_c1", lbl));
                    output.push(format!("    goto {}_1h_c1", lbl));
                    output.push(format!("{}_1h_c0:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_1h_c1:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    add 1 {}", carry));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_t1_h:", lbl));
                    output.push(format!("    case {}", src));
                    output.push(format!("    goto {}_h0", lbl));
                    output.push(format!("    goto {}_h1", lbl));
                    output.push(format!("    goto {}_hh", lbl));
                    output.push(format!("{}_h0:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_h0_c0", lbl));
                    output.push(format!("    goto {}_h0_c1", lbl));
                    output.push(format!("    goto {}_h0_c1", lbl));
                    output.push(format!("{}_h0_c0:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_h0_c1:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_h1:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_h1_c0", lbl));
                    output.push(format!("    goto {}_h1_c1", lbl));
                    output.push(format!("    goto {}_h1_c1", lbl));
                    output.push(format!("{}_h1_c0:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_h1_c1:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    add 1 {}", carry));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_hh:", lbl));
                    output.push(format!("    case {}", carry));
                    output.push(format!("    goto {}_hh_c0", lbl));
                    output.push(format!("    goto {}_hh_c1", lbl));
                    output.push(format!("    goto {}_hh_c1", lbl));
                    output.push(format!("{}_hh_c0:", lbl));
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_hh_c1:", lbl));
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "pop" => {
                    // pop <reg> - removes the first symbol from reg
                    if parts.len() != 2 {
                        return Err(format!(
                            "Line {}: 'pop' expects 1 argument: pop <reg>",
                            line_num
                        ));
                    }
                    let reg = parts[1];
                    let lbl = unique_label("pop");
                    // Just case and discard the first symbol
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", reg));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "swap" => {
                    // swap <a> <b> <tmp> - swaps contents of a and b
                    if parts.len() != 4 {
                        return Err(format!(
                            "Line {}: 'swap' expects 3 arguments: swap <a> <b> <tmp>",
                            line_num
                        ));
                    }
                    let (a, b, tmp) = (parts[1], parts[2], parts[3]);
                    // tmp = a; a = b; b = tmp
                    output.push(format!("    move {} {}", a, tmp));
                    output.push(format!("    move {} {}", b, a));
                    output.push(format!("    move {} {}", tmp, b));
                    changed = true;
                }
                "increment" => {
                    // increment <reg> <tmp> - adds 1 to reg (binary)
                    if parts.len() != 3 {
                        return Err(format!(
                            "Line {}: 'increment' expects 2 arguments: increment <reg> <tmp>",
                            line_num
                        ));
                    }
                    let (reg, tmp) = (parts[1], parts[2]);
                    let lbl = unique_label("inc");
                    // Binary increment: flip bits until we find a 0
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", reg));
                    output.push(format!("    goto {}_add1", lbl)); // empty -> just add 1
                    output.push(format!("    goto {}_one", lbl));
                    output.push(format!("    goto {}_hash", lbl));
                    output.push(format!("{}_one:", lbl));
                    // bit is 1, becomes 0, carry propagates
                    output.push(format!("    add # {}", tmp));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_hash:", lbl));
                    // bit is 0, becomes 1, done with carry
                    output.push(format!("    add 1 {}", tmp));
                    output.push(format!("    goto {}_copy", lbl));
                    output.push(format!("{}_add1:", lbl));
                    output.push(format!("    add 1 {}", tmp));
                    output.push(format!("{}_copy:", lbl));
                    // copy remaining bits and restore
                    output.push(format!("    case {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("    goto {}_c1", lbl));
                    output.push(format!("    goto {}_ch", lbl));
                    output.push(format!("{}_c1:", lbl));
                    output.push(format!("    add 1 {}", tmp));
                    output.push(format!("    goto {}_copy", lbl));
                    output.push(format!("{}_ch:", lbl));
                    output.push(format!("    add # {}", tmp));
                    output.push(format!("    goto {}_copy", lbl));
                    output.push(format!("{}_rest:", lbl));
                    output.push(format!("    case {}", tmp));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_r1", lbl));
                    output.push(format!("    goto {}_rh", lbl));
                    output.push(format!("{}_r1:", lbl));
                    output.push(format!("    add 1 {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("{}_rh:", lbl));
                    output.push(format!("    add # {}", reg));
                    output.push(format!("    goto {}_rest", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "shift_right" => {
                    // shift_right <reg> - removes first bit (divide by 2, truncate)
                    if parts.len() != 2 {
                        return Err(format!(
                            "Line {}: 'shift_right' expects 1 argument: shift_right <reg>",
                            line_num
                        ));
                    }
                    let reg = parts[1];
                    // Just pop the first symbol (LSB)
                    output.push(format!("    pop {}", reg));
                    changed = true;
                }
                "multiply" => {
                    // multiply <a> <b> <dst> <tmp1> <tmp2> <tmp3> <tmp4> <tmp5>
                    // Computes a * b, stores in dst. Destroys a, preserves b.
                    if parts.len() != 9 {
                        return Err(format!(
                            "Line {}: 'multiply' expects 8 arguments: multiply <a> <b> <dst> <tmp1> <tmp2> <tmp3> <tmp4> <tmp5>",
                            line_num
                        ));
                    }
                    let (a, b, dst, tmp1, tmp2, tmp3, tmp4, tmp5) =
                        (parts[1], parts[2], parts[3], parts[4], parts[5], parts[6], parts[7], parts[8]);
                    let lbl = unique_label("mul");
                    // Algorithm: shift-and-add
                    // multiplicand = a (shifted left each iteration)
                    // multiplier = copy of b (consumed bit by bit)
                    // result = dst (accumulates)
                    output.push(format!("    clear {}", dst));
                    output.push(format!("    copy {} {} {}", b, tmp1, tmp2)); // tmp1 = multiplier bits
                    output.push(format!("{}:", lbl));
                    output.push(format!("    case {}", tmp1));
                    output.push(format!("    goto {}_done", lbl));
                    output.push(format!("    goto {}_add", lbl));
                    output.push(format!("    goto {}_shift", lbl));
                    output.push(format!("{}_add:", lbl));
                    output.push(format!("    copy {} {} {}", a, tmp2, tmp3)); // tmp2 = copy of multiplicand
                    output.push(format!("    bin_add {} {} {} {} {}", tmp2, dst, tmp3, tmp4, tmp5));
                    output.push(format!("{}_shift:", lbl));
                    output.push(format!("    shift_left {} {}", a, tmp2));
                    output.push(format!("    goto {}", lbl));
                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "bin_sub" => {
                    // bin_sub <src> <dst> <tmp1> <tmp2> <borrow>
                    // Computes dst = dst - src (destroys src)
                    if parts.len() != 6 {
                        return Err(format!(
                            "Line {}: 'bin_sub' expects 5 arguments: bin_sub <src> <dst> <tmp1> <tmp2> <borrow>",
                            line_num
                        ));
                    }
                    let (src, dst, tmp1, borrow) = (parts[1], parts[2], parts[3], parts[5]);
                    let lbl = unique_label("bsub");

                    // Move dst to tmp1
                    output.push(format!("{}_mvdst:", lbl));
                    output.push(format!("    case {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("    goto {}_mv1", lbl));
                    output.push(format!("    goto {}_mvh", lbl));
                    output.push(format!("{}_mv1:", lbl));
                    output.push(format!("    add 1 {}", tmp1));
                    output.push(format!("    goto {}_mvdst", lbl));
                    output.push(format!("{}_mvh:", lbl));
                    output.push(format!("    add # {}", tmp1));
                    output.push(format!("    goto {}_mvdst", lbl));

                    // Main loop: process bits
                    output.push(format!("{}_loop:", lbl));
                    output.push(format!("    case {}", tmp1)); // dst bit
                    output.push(format!("    goto {}_t1e", lbl)); // dst empty
                    output.push(format!("    goto {}_t1_1", lbl)); // dst bit = 1
                    output.push(format!("    goto {}_t1_h", lbl)); // dst bit = 0

                    // dst empty, drain any remaining src with borrow
                    output.push(format!("{}_t1e:", lbl));
                    output.push(format!("    case {}", src));
                    output.push(format!("    goto {}_both_e", lbl));
                    output.push(format!("    goto {}_0_1", lbl)); // 0 - 1
                    output.push(format!("    goto {}_0_0", lbl)); // 0 - 0

                    // Both empty - just handle any remaining borrow
                    output.push(format!("{}_both_e:", lbl));
                    output.push(format!("    clear {}", borrow)); // ignore underflow
                    output.push(format!("    goto {}_done", lbl));

                    // 0 - 1: result is 1 with borrow (0 - 1 = -1 = borrow 1, result 1)
                    output.push(format!("{}_0_1:", lbl));
                    output.push(format!("    case {}", borrow));
                    output.push(format!("    goto {}_0_1_b0", lbl));
                    output.push(format!("    goto {}_0_1_b1", lbl));
                    output.push(format!("    goto {}_0_1_b1", lbl));
                    output.push(format!("{}_0_1_b0:", lbl)); // 0 - 1 - 0 = 1, borrow 1
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    add 1 {}", borrow));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_0_1_b1:", lbl)); // 0 - 1 - 1 = 0, borrow 1
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    add 1 {}", borrow));
                    output.push(format!("    goto {}_loop", lbl));

                    // 0 - 0
                    output.push(format!("{}_0_0:", lbl));
                    output.push(format!("    case {}", borrow));
                    output.push(format!("    goto {}_0_0_b0", lbl));
                    output.push(format!("    goto {}_0_0_b1", lbl));
                    output.push(format!("    goto {}_0_0_b1", lbl));
                    output.push(format!("{}_0_0_b0:", lbl)); // 0 - 0 - 0 = 0
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_0_0_b1:", lbl)); // 0 - 0 - 1 = 1, borrow 1
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    add 1 {}", borrow));
                    output.push(format!("    goto {}_loop", lbl));

                    // dst bit = 1
                    output.push(format!("{}_t1_1:", lbl));
                    output.push(format!("    case {}", src));
                    output.push(format!("    goto {}_1_e", lbl)); // 1 - empty
                    output.push(format!("    goto {}_1_1", lbl)); // 1 - 1
                    output.push(format!("    goto {}_1_0", lbl)); // 1 - 0

                    // 1 - empty (treat as 1 - 0)
                    output.push(format!("{}_1_e:", lbl));
                    output.push(format!("    case {}", borrow));
                    output.push(format!("    goto {}_1_e_b0", lbl));
                    output.push(format!("    goto {}_1_e_b1", lbl));
                    output.push(format!("    goto {}_1_e_b1", lbl));
                    output.push(format!("{}_1_e_b0:", lbl)); // 1 - 0 - 0 = 1
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_1_e_b1:", lbl)); // 1 - 0 - 1 = 0
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));

                    // 1 - 1
                    output.push(format!("{}_1_1:", lbl));
                    output.push(format!("    case {}", borrow));
                    output.push(format!("    goto {}_1_1_b0", lbl));
                    output.push(format!("    goto {}_1_1_b1", lbl));
                    output.push(format!("    goto {}_1_1_b1", lbl));
                    output.push(format!("{}_1_1_b0:", lbl)); // 1 - 1 - 0 = 0
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_1_1_b1:", lbl)); // 1 - 1 - 1 = 1, borrow 1
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    add 1 {}", borrow));
                    output.push(format!("    goto {}_loop", lbl));

                    // 1 - 0
                    output.push(format!("{}_1_0:", lbl));
                    output.push(format!("    case {}", borrow));
                    output.push(format!("    goto {}_1_0_b0", lbl));
                    output.push(format!("    goto {}_1_0_b1", lbl));
                    output.push(format!("    goto {}_1_0_b1", lbl));
                    output.push(format!("{}_1_0_b0:", lbl)); // 1 - 0 - 0 = 1
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_1_0_b1:", lbl)); // 1 - 0 - 1 = 0
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));

                    // dst bit = 0
                    output.push(format!("{}_t1_h:", lbl));
                    output.push(format!("    case {}", src));
                    output.push(format!("    goto {}_0_e", lbl)); // 0 - empty
                    output.push(format!("    goto {}_h_1", lbl)); // 0 - 1
                    output.push(format!("    goto {}_h_0", lbl)); // 0 - 0

                    // 0 - empty (treat as 0 - 0)
                    output.push(format!("{}_0_e:", lbl));
                    output.push(format!("    case {}", borrow));
                    output.push(format!("    goto {}_0_e_b0", lbl));
                    output.push(format!("    goto {}_0_e_b1", lbl));
                    output.push(format!("    goto {}_0_e_b1", lbl));
                    output.push(format!("{}_0_e_b0:", lbl)); // 0 - 0 - 0 = 0
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_0_e_b1:", lbl)); // 0 - 0 - 1 = 1, borrow 1
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    add 1 {}", borrow));
                    output.push(format!("    goto {}_loop", lbl));

                    // 0 - 1
                    output.push(format!("{}_h_1:", lbl));
                    output.push(format!("    case {}", borrow));
                    output.push(format!("    goto {}_h_1_b0", lbl));
                    output.push(format!("    goto {}_h_1_b1", lbl));
                    output.push(format!("    goto {}_h_1_b1", lbl));
                    output.push(format!("{}_h_1_b0:", lbl)); // 0 - 1 - 0 = 1, borrow 1
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    add 1 {}", borrow));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_h_1_b1:", lbl)); // 0 - 1 - 1 = 0, borrow 1
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    add 1 {}", borrow));
                    output.push(format!("    goto {}_loop", lbl));

                    // 0 - 0
                    output.push(format!("{}_h_0:", lbl));
                    output.push(format!("    case {}", borrow));
                    output.push(format!("    goto {}_h_0_b0", lbl));
                    output.push(format!("    goto {}_h_0_b1", lbl));
                    output.push(format!("    goto {}_h_0_b1", lbl));
                    output.push(format!("{}_h_0_b0:", lbl)); // 0 - 0 - 0 = 0
                    output.push(format!("    add # {}", dst));
                    output.push(format!("    goto {}_loop", lbl));
                    output.push(format!("{}_h_0_b1:", lbl)); // 0 - 0 - 1 = 1, borrow 1
                    output.push(format!("    add 1 {}", dst));
                    output.push(format!("    add 1 {}", borrow));
                    output.push(format!("    goto {}_loop", lbl));

                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                "compare" => {
                    // compare <a> <b> <tmp1> <tmp2> <tmp3> <lt> <eq> <gt>
                    // Compares a and b, jumps to lt/eq/gt. Preserves both.
                    if parts.len() != 9 {
                        return Err(format!(
                            "Line {}: 'compare' expects 8 arguments: compare <a> <b> <tmp1> <tmp2> <tmp3> <lt> <eq> <gt>",
                            line_num
                        ));
                    }
                    let (a, b, tmp1, tmp2, tmp3, lt, eq, gt) =
                        (parts[1], parts[2], parts[3], parts[4], parts[5], parts[6], parts[7], parts[8]);
                    let lbl = unique_label("cmp");

                    // Use copy macro to copy a to tmp1 and b to tmp2 (preserves originals)
                    // Then compare tmp1 vs tmp2 destructively
                    output.push(format!("    copy {} {} {}", a, tmp1, tmp3)); // tmp1 = a, uses tmp3 as scratch
                    output.push(format!("    copy {} {} {}", b, tmp2, tmp3)); // tmp2 = b, uses tmp3 as scratch

                    // Now tmp1 has copy of a, tmp2 has copy of b
                    // Compare loop - process bits from LSB, track last difference
                    output.push(format!("{}_loop:", lbl));
                    output.push(format!("    case {}", tmp1)); // a's bit
                    output.push(format!("    goto {}_a_empty", lbl));
                    output.push(format!("    goto {}_a_1", lbl));
                    output.push(format!("    goto {}_a_0", lbl));

                    // a empty - if b has remaining 1-bits, a < b
                    output.push(format!("{}_a_empty:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}", eq)); // both empty = equal
                    output.push(format!("    goto {}_drain_lt", lbl)); // b has 1 = b larger
                    output.push(format!("    goto {}_a_empty", lbl)); // b has 0, continue

                    // a has 1
                    output.push(format!("{}_a_1:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}_drain_gt", lbl)); // b empty, a has 1 = a larger
                    output.push(format!("    goto {}_loop", lbl)); // both 1, continue
                    output.push(format!("    goto {}_diff_a_gt", lbl)); // a=1, b=0, tentatively a>b

                    // a has 0
                    output.push(format!("{}_a_0:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}_loop", lbl)); // b empty (implicit 0), continue
                    output.push(format!("    goto {}_diff_a_lt", lbl)); // a=0, b=1, tentatively a<b
                    output.push(format!("    goto {}_loop", lbl)); // both 0, continue

                    // In diff_a_gt state: at the current highest differing bit, a > b
                    output.push(format!("{}_diff_a_gt:", lbl));
                    output.push(format!("    case {}", tmp1));
                    output.push(format!("    goto {}_dgt_a_e", lbl));
                    output.push(format!("    goto {}_dgt_a1", lbl));
                    output.push(format!("    goto {}_dgt_a0", lbl));
                    output.push(format!("{}_dgt_a1:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}_drain_gt", lbl)); // b empty
                    output.push(format!("    goto {}_diff_a_gt", lbl)); // both 1, stay in a>b
                    output.push(format!("    goto {}_diff_a_gt", lbl)); // a=1 b=0, stay in a>b
                    output.push(format!("{}_dgt_a0:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}_diff_a_gt", lbl)); // b empty (0), stay in a>b
                    output.push(format!("    goto {}_diff_a_lt", lbl)); // a=0 b=1, switch to a<b
                    output.push(format!("    goto {}_diff_a_gt", lbl)); // both 0, stay in a>b
                    output.push(format!("{}_dgt_a_e:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}", gt)); // both done, a was greater
                    output.push(format!("    goto {}_drain_lt", lbl)); // b has 1 remaining
                    output.push(format!("    goto {}_dgt_a_e", lbl)); // b has 0, continue draining

                    // In diff_a_lt state: at the current highest differing bit, a < b
                    output.push(format!("{}_diff_a_lt:", lbl));
                    output.push(format!("    case {}", tmp1));
                    output.push(format!("    goto {}_dlt_a_e", lbl));
                    output.push(format!("    goto {}_dlt_a1", lbl));
                    output.push(format!("    goto {}_dlt_a0", lbl));
                    output.push(format!("{}_dlt_a1:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}_drain_gt", lbl)); // b empty, a has 1
                    output.push(format!("    goto {}_diff_a_lt", lbl)); // both 1, stay in a<b
                    output.push(format!("    goto {}_diff_a_gt", lbl)); // a=1 b=0, switch to a>b
                    output.push(format!("{}_dlt_a0:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}_diff_a_lt", lbl)); // b empty
                    output.push(format!("    goto {}_diff_a_lt", lbl)); // a=0 b=1, stay in a<b
                    output.push(format!("    goto {}_diff_a_lt", lbl)); // both 0, stay in a<b
                    output.push(format!("{}_dlt_a_e:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}", lt)); // both done, a was less
                    output.push(format!("    goto {}_drain_lt", lbl)); // b has 1
                    output.push(format!("    goto {}_dlt_a_e", lbl)); // b has 0

                    // Drain remaining bits
                    output.push(format!("{}_drain_lt:", lbl));
                    output.push(format!("    case {}", tmp2));
                    output.push(format!("    goto {}", lt));
                    output.push(format!("    goto {}_drain_lt", lbl));
                    output.push(format!("    goto {}_drain_lt", lbl));
                    output.push(format!("{}_drain_gt:", lbl));
                    output.push(format!("    case {}", tmp1));
                    output.push(format!("    goto {}", gt));
                    output.push(format!("    goto {}_drain_gt", lbl));
                    output.push(format!("    goto {}_drain_gt", lbl));
                    changed = true;
                }
                "divide" => {
                    // divide <dividend> <divisor> <quotient> <remainder> <tmp1> <tmp2> <tmp3> <tmp4>
                    // Computes quotient = dividend / divisor, remainder = dividend % divisor
                    // Destroys dividend, preserves divisor
                    if parts.len() != 9 {
                        return Err(format!(
                            "Line {}: 'divide' expects 8 arguments: divide <dividend> <divisor> <quotient> <remainder> <tmp1> <tmp2> <tmp3> <tmp4>",
                            line_num
                        ));
                    }
                    let (dividend, divisor, quotient, remainder, tmp1, tmp2, tmp3, tmp4) =
                        (parts[1], parts[2], parts[3], parts[4], parts[5], parts[6], parts[7], parts[8]);
                    let lbl = unique_label("div");

                    // Repeated subtraction algorithm:
                    // quotient = 0
                    // remainder = dividend
                    // while remainder >= divisor:
                    //     remainder -= divisor
                    //     quotient++

                    output.push(format!("    clear {}", quotient));
                    output.push(format!("    move {} {}", dividend, remainder)); // remainder = dividend

                    // Main loop
                    output.push(format!("{}_loop:", lbl));
                    // Compare remainder vs divisor
                    output.push(format!("    compare {} {} {} {} {} {}_done {}_done {}_sub",
                        remainder, divisor, tmp1, tmp2, tmp3, lbl, lbl, lbl));

                    // remainder >= divisor: subtract and increment quotient
                    output.push(format!("{}_sub:", lbl));
                    output.push(format!("    copy {} {} {}", divisor, tmp1, tmp2)); // tmp1 = divisor
                    output.push(format!("    bin_sub {} {} {} {} {}", tmp1, remainder, tmp2, tmp3, tmp4)); // remainder -= divisor
                    output.push(format!("    increment {} {}", quotient, tmp1)); // quotient++
                    output.push(format!("    goto {}_loop", lbl));

                    output.push(format!("{}_done:", lbl));
                    changed = true;
                }
                _ => {
                    output.push(line.to_string());
                }
            }
        }

        current = output.join("\n");

        if !changed {
            break;
        }

        if pass == max_passes - 1 {
            return Err("Too many macro expansion passes (possible infinite recursion)".to_string());
        }
    }

    Ok(current)
}

/// Assembly instruction before label resolution.
#[derive(Debug, Clone)]
enum AsmInstruction {
    Add1(usize),
    AddHash(usize),
    Goto(String),
    Case(usize),
    Label(String),
}

/// Resolved 1# instruction with computed jump offsets.
#[derive(Debug, Clone)]
enum Instruction {
    AddOne(usize),
    AddHash(usize),
    Forward(usize),
    Backward(usize),
    Case(usize),
}

impl Instruction {
    /// Converts the instruction to 1# encoding.
    fn to_one_hash(&self) -> String {
        match self {
            Instruction::AddOne(n) => format!("{}#", "1".repeat(*n)),
            Instruction::AddHash(n) => format!("{}##", "1".repeat(*n)),
            Instruction::Forward(n) => format!("{}###", "1".repeat(*n)),
            Instruction::Backward(n) => format!("{}####", "1".repeat(*n)),
            Instruction::Case(n) => format!("{}#####", "1".repeat(*n)),
        }
    }
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

/// Compiles assembly source to 1# code.
fn compile(source: &str) -> Result<String, String> {
    LABEL_COUNTER.store(0, Ordering::SeqCst);
    let expanded = expand_macros(source)?;
    let asm = parse_assembly(&expanded)?;
    let instructions = resolve_labels(asm)?;
    Ok(instructions
        .iter()
        .map(|i| i.to_one_hash())
        .collect::<Vec<_>>()
        .join(""))
}

/// Compiles assembly source to 1# code with verbose output.
fn compile_verbose(source: &str) -> Result<String, String> {
    LABEL_COUNTER.store(0, Ordering::SeqCst);
    let expanded = expand_macros(source)?;

    println!("Expanded macros ({} lines):", expanded.lines().count());
    for (i, line) in expanded.lines().enumerate() {
        if !line.trim().is_empty() {
            println!("  {}: {}", i + 1, line);
        }
    }
    println!();

    let asm = parse_assembly(&expanded)?;
    let instructions = resolve_labels(asm)?;

    println!("Resolved ({} instructions)", instructions.len());
    println!();

    Ok(instructions
        .iter()
        .map(|i| i.to_one_hash())
        .collect::<Vec<_>>()
        .join(""))
}

fn print_usage() {
    eprintln!("Usage: one_hash_asm [OPTIONS] <FILE>");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o <file>     Output to file instead of stdout");
    eprintln!("  -v            Verbose output (show compilation steps)");
    eprintln!("  -e <code>     Compile code from command line");
    eprintln!("  -E            Expand macros only (don't compile)");
    eprintln!("  -h, --help    Show this help");
    eprintln!();
    eprintln!("Assembly syntax:");
    eprintln!("  add 1 R1      Append 1 to register 1");
    eprintln!("  add # R2      Append # to register 2");
    eprintln!("  case R1       Branch on first symbol of R1");
    eprintln!("  goto label    Jump to label");
    eprintln!("  halt          Stop execution");
    eprintln!("  label:        Define a label");
    eprintln!("  ; comment     Comment");
    eprintln!();
    eprintln!("Built-in macros:");
    eprintln!("  clear <reg>                           reg := empty");
    eprintln!("  move <src> <dst>                      dst := src; src := empty");
    eprintln!("  copy <src> <dst> <tmp>                dst := src (preserves src)");
    eprintln!("  swap <a> <b> <tmp>                    swap a and b");
    eprintln!("  pop <reg>                             remove first symbol from reg");
    eprintln!("  shift_left <reg> <tmp>                reg := # + reg (multiply by 2)");
    eprintln!("  shift_right <reg>                     remove LSB (divide by 2)");
    eprintln!("  increment <reg> <tmp>                 reg := reg + 1 (binary)");
    eprintln!("  decrement <reg> <tmp>                 reg := reg - 1 (binary)");
    eprintln!("  is_nonzero <r> <t1> <t2> <nz> <z>     goto nz if r!=0, else goto z");
    eprintln!("  bin_add <src> <dst> <t1> <t2> <c>     dst := dst + src (binary)");
    eprintln!("  bin_sub <src> <dst> <t1> <t2> <b>     dst := dst - src (binary)");
    eprintln!("  multiply <a> <b> <dst> <t1-t5>        dst := a * b (destroys a)");
    eprintln!("  compare <a> <b> <t1-t3> <lt> <eq> <gt>  branch on a vs b");
    eprintln!("  divide <dd> <dv> <q> <r> <t1-t4>      q := dd/dv, r := dd%dv");
    eprintln!();
    eprintln!("User-defined macros:");
    eprintln!("  .macro <name> <param1> <param2> ...   Begin macro definition");
    eprintln!("  .endmacro                             End macro definition");
    eprintln!("  @label                                Local label (unique per call)");
}

fn repl() {
    println!("1# Assembler REPL");
    println!("Commands: .compile .expand .clear .show .help .quit");
    println!();

    let stdin = io::stdin();
    let mut code_buffer = String::new();

    loop {
        print!("asm> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            break;
        }
        let trimmed = line.trim();

        if trimmed == ".quit" || trimmed == ".q" {
            break;
        } else if trimmed == ".compile" || trimmed == ".c" {
            match compile_verbose(&code_buffer) {
                Ok(code) => {
                    println!("1# output ({} chars):", code.len());
                    println!("{}", code);
                }
                Err(e) => eprintln!("Error: {}", e),
            }
            println!();
        } else if trimmed == ".expand" || trimmed == ".e" {
            LABEL_COUNTER.store(0, Ordering::SeqCst);
            match expand_macros(&code_buffer) {
                Ok(expanded) => {
                    println!("Expanded:");
                    println!("{}", expanded);
                }
                Err(e) => eprintln!("Error: {}", e),
            }
            println!();
        } else if trimmed == ".clear" {
            code_buffer.clear();
            println!("Buffer cleared.");
        } else if trimmed == ".show" {
            if code_buffer.is_empty() {
                println!("(buffer is empty)");
            } else {
                println!("Current buffer:");
                for (i, line) in code_buffer.lines().enumerate() {
                    println!("  {}: {}", i + 1, line);
                }
            }
        } else if trimmed == ".help" || trimmed == ".h" {
            print_usage();
        } else {
            code_buffer.push_str(&line);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        repl();
        return;
    }

    let mut source: Option<String> = None;
    let mut output_file: Option<String> = None;
    let mut verbose = false;
    let mut expand_only = false;
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        if arg == "-h" || arg == "--help" {
            print_usage();
            return;
        } else if arg == "-v" {
            verbose = true;
        } else if arg == "-E" {
            expand_only = true;
        } else if arg == "-o" {
            i += 1;
            if i >= args.len() {
                eprintln!("Error: -o requires a filename");
                return;
            }
            output_file = Some(args[i].clone());
        } else if arg == "-e" {
            i += 1;
            if i >= args.len() {
                eprintln!("Error: -e requires code");
                return;
            }
            source = Some(args[i].clone());
        } else if arg.starts_with('-') {
            eprintln!("Unknown option: {}", arg);
            print_usage();
            return;
        } else {
            match fs::read_to_string(arg) {
                Ok(contents) => source = Some(contents),
                Err(e) => {
                    eprintln!("Error reading file '{}': {}", arg, e);
                    return;
                }
            }
        }
        i += 1;
    }

    let source = match source {
        Some(s) => s,
        None => {
            eprintln!("No input specified");
            print_usage();
            return;
        }
    };

    if expand_only {
        LABEL_COUNTER.store(0, Ordering::SeqCst);
        match expand_macros(&source) {
            Ok(expanded) => println!("{}", expanded),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    let result = if verbose {
        compile_verbose(&source)
    } else {
        compile(&source)
    };

    match result {
        Ok(code) => {
            if let Some(out_file) = output_file {
                match fs::write(&out_file, &code) {
                    Ok(_) => {
                        if verbose {
                            println!("Written to {}", out_file);
                        }
                    }
                    Err(e) => eprintln!("Error writing file: {}", e),
                }
            } else {
                println!("{}", code);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_copy_macro() {
        let code = compile("copy R1 R2 R9").unwrap();
        assert!(!code.is_empty());
    }

    #[test]
    fn test_user_macro() {
        let source = r#"
.macro addone reg
    add 1 reg
.endmacro

addone R1
addone R2
"#;
        let code = compile(source).unwrap();
        assert!(!code.is_empty());
        assert!(code.contains("1#"));
        assert!(code.contains("11#"));
    }

    #[test]
    fn test_user_macro_with_local_labels() {
        let source = r#"
.macro clearit reg
@loop:
    case reg
    goto @done
    goto @loop
    goto @loop
@done:
.endmacro

clearit R1
clearit R2
"#;
        let code = compile(source).unwrap();
        assert!(!code.is_empty());
    }

    #[test]
    fn test_nested_macro_expansion() {
        let source = r#"
.macro myclear reg
    clear reg
.endmacro

myclear R1
"#;
        let code = compile(source).unwrap();
        assert!(!code.is_empty());
    }
}
