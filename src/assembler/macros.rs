//! Macro expansion for the 1# assembler.
//!
//! Handles both user-defined macros and built-in macros.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

static LABEL_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Resets the label counter (useful for testing).
pub fn reset_label_counter() {
    LABEL_COUNTER.store(0, Ordering::SeqCst);
}

/// Generates a unique label with the given prefix.
fn unique_label(prefix: &str) -> String {
    let n = LABEL_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("__{}_{}__", prefix, n)
}

/// A user-defined macro with parameters and body.
#[derive(Debug, Clone)]
pub struct UserMacro {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<String>,
}

/// Extracts macro definitions from source, returning macros and remaining source.
pub fn parse_macro_definitions(
    source: &str,
) -> Result<(HashMap<String, UserMacro>, String), String> {
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
pub fn expand_user_macro(
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
pub fn expand_macros(source: &str) -> Result<String, String> {
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

            let (expanded, did_change) = expand_builtin_macro(&cmd, &parts, line_num)?;
            if did_change {
                output.extend(expanded);
                changed = true;
            } else {
                output.push(line.to_string());
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

/// Expands a built-in macro if recognized.
/// Returns (expanded_lines, was_changed).
fn expand_builtin_macro(
    cmd: &str,
    parts: &[&str],
    line_num: usize,
) -> Result<(Vec<String>, bool), String> {
    let mut output = Vec::new();

    match cmd {
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
            Ok((output, true))
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
            Ok((output, true))
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
            Ok((output, true))
        }
        "pop" => {
            if parts.len() != 2 {
                return Err(format!(
                    "Line {}: 'pop' expects 1 argument: pop <reg>",
                    line_num
                ));
            }
            let reg = parts[1];
            let lbl = unique_label("pop");
            output.push(format!("{}:", lbl));
            output.push(format!("    case {}", reg));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("{}_done:", lbl));
            Ok((output, true))
        }
        "swap" => {
            if parts.len() != 4 {
                return Err(format!(
                    "Line {}: 'swap' expects 3 arguments: swap <a> <b> <tmp>",
                    line_num
                ));
            }
            let (a, b, tmp) = (parts[1], parts[2], parts[3]);
            output.push(format!("    move {} {}", a, tmp));
            output.push(format!("    move {} {}", b, a));
            output.push(format!("    move {} {}", tmp, b));
            Ok((output, true))
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
            Ok((output, true))
        }
        "shift_right" => {
            if parts.len() != 2 {
                return Err(format!(
                    "Line {}: 'shift_right' expects 1 argument: shift_right <reg>",
                    line_num
                ));
            }
            let reg = parts[1];
            output.push(format!("    pop {}", reg));
            Ok((output, true))
        }
        "increment" => {
            if parts.len() != 3 {
                return Err(format!(
                    "Line {}: 'increment' expects 2 arguments: increment <reg> <tmp>",
                    line_num
                ));
            }
            let (reg, tmp) = (parts[1], parts[2]);
            let lbl = unique_label("inc");
            output.push(format!("{}:", lbl));
            output.push(format!("    case {}", reg));
            output.push(format!("    goto {}_add1", lbl));
            output.push(format!("    goto {}_one", lbl));
            output.push(format!("    goto {}_hash", lbl));
            output.push(format!("{}_one:", lbl));
            output.push(format!("    add # {}", tmp));
            output.push(format!("    goto {}", lbl));
            output.push(format!("{}_hash:", lbl));
            output.push(format!("    add 1 {}", tmp));
            output.push(format!("    goto {}_copy", lbl));
            output.push(format!("{}_add1:", lbl));
            output.push(format!("    add 1 {}", tmp));
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
            Ok((output, true))
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
            Ok((output, true))
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
            Ok((output, true))
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
            expand_bin_add(&mut output, src, dst, tmp1, carry, &lbl);
            Ok((output, true))
        }
        "bin_sub" => {
            if parts.len() != 6 {
                return Err(format!(
                    "Line {}: 'bin_sub' expects 5 arguments: bin_sub <src> <dst> <tmp1> <tmp2> <borrow>",
                    line_num
                ));
            }
            let (src, dst, tmp1, borrow) = (parts[1], parts[2], parts[3], parts[5]);
            let lbl = unique_label("bsub");
            expand_bin_sub(&mut output, src, dst, tmp1, borrow, &lbl);
            Ok((output, true))
        }
        "multiply" => {
            if parts.len() != 9 {
                return Err(format!(
                    "Line {}: 'multiply' expects 8 arguments: multiply <a> <b> <dst> <tmp1> <tmp2> <tmp3> <tmp4> <tmp5>",
                    line_num
                ));
            }
            let (a, b, dst, tmp1, tmp2, tmp3, tmp4, tmp5) = (
                parts[1], parts[2], parts[3], parts[4], parts[5], parts[6], parts[7], parts[8],
            );
            let lbl = unique_label("mul");
            output.push(format!("    clear {}", dst));
            output.push(format!("    copy {} {} {}", b, tmp1, tmp2));
            output.push(format!("{}:", lbl));
            output.push(format!("    case {}", tmp1));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("    goto {}_add", lbl));
            output.push(format!("    goto {}_shift", lbl));
            output.push(format!("{}_add:", lbl));
            output.push(format!("    copy {} {} {}", a, tmp2, tmp3));
            output.push(format!(
                "    bin_add {} {} {} {} {}",
                tmp2, dst, tmp3, tmp4, tmp5
            ));
            output.push(format!("{}_shift:", lbl));
            output.push(format!("    shift_left {} {}", a, tmp2));
            output.push(format!("    goto {}", lbl));
            output.push(format!("{}_done:", lbl));
            Ok((output, true))
        }
        "compare" => {
            if parts.len() != 9 {
                return Err(format!(
                    "Line {}: 'compare' expects 8 arguments: compare <a> <b> <tmp1> <tmp2> <tmp3> <lt> <eq> <gt>",
                    line_num
                ));
            }
            let (a, b, tmp1, tmp2, tmp3, lt, eq, gt) = (
                parts[1], parts[2], parts[3], parts[4], parts[5], parts[6], parts[7], parts[8],
            );
            let lbl = unique_label("cmp");
            expand_compare(&mut output, a, b, tmp1, tmp2, tmp3, lt, eq, gt, &lbl);
            Ok((output, true))
        }
        "divide" => {
            if parts.len() != 9 {
                return Err(format!(
                    "Line {}: 'divide' expects 8 arguments: divide <dividend> <divisor> <quotient> <remainder> <tmp1> <tmp2> <tmp3> <tmp4>",
                    line_num
                ));
            }
            let (dividend, divisor, quotient, remainder, tmp1, tmp2, tmp3, tmp4) = (
                parts[1], parts[2], parts[3], parts[4], parts[5], parts[6], parts[7], parts[8],
            );
            let lbl = unique_label("div");
            output.push(format!("    clear {}", quotient));
            output.push(format!("    move {} {}", dividend, remainder));
            output.push(format!("{}_loop:", lbl));
            output.push(format!(
                "    compare {} {} {} {} {} {}_done {}_done {}_sub",
                remainder, divisor, tmp1, tmp2, tmp3, lbl, lbl, lbl
            ));
            output.push(format!("{}_sub:", lbl));
            output.push(format!("    copy {} {} {}", divisor, tmp1, tmp2));
            output.push(format!(
                "    bin_sub {} {} {} {} {}",
                tmp1, remainder, tmp2, tmp3, tmp4
            ));
            output.push(format!("    increment {} {}", quotient, tmp1));
            output.push(format!("    goto {}_loop", lbl));
            output.push(format!("{}_done:", lbl));
            Ok((output, true))
        }
        // ================================================================
        // Textbook 1# program macros (parametric versions)
        // These are assembly implementations of standard 1# programs
        // ================================================================
        "compare_eq" => {
            // Compares two registers for equality
            // If equal, outputs 1 in result; if not, leaves result empty
            // compare_eq <a> <b> <result> <tmp1> <tmp2>
            if parts.len() != 6 {
                return Err(format!(
                    "Line {}: 'compare_eq' expects 5 arguments: compare_eq <a> <b> <result> <tmp1> <tmp2>",
                    line_num
                ));
            }
            let (a, b, result, tmp1, tmp2) = (parts[1], parts[2], parts[3], parts[4], parts[5]);
            let lbl = unique_label("ceq");
            // Copy a and b to temps to preserve them
            output.push(format!("    copy {} {} {}", a, tmp1, result));
            output.push(format!("    copy {} {} {}", b, tmp2, result));
            // Compare symbol by symbol
            output.push(format!("{}:", lbl));
            output.push(format!("    case {}", tmp1));
            output.push(format!("    goto {}_a_empty", lbl));
            output.push(format!("    goto {}_a_one", lbl));
            output.push(format!("    goto {}_a_hash", lbl));
            output.push(format!("{}_a_empty:", lbl));
            // a is empty, b should also be empty
            output.push(format!("    case {}", tmp2));
            output.push(format!("    goto {}_equal", lbl));
            output.push(format!("    goto {}_not_equal", lbl));
            output.push(format!("    goto {}_not_equal", lbl));
            output.push(format!("{}_a_one:", lbl));
            // a has 1, b should also have 1
            output.push(format!("    case {}", tmp2));
            output.push(format!("    goto {}_not_equal", lbl));
            output.push(format!("    goto {}", lbl));
            output.push(format!("    goto {}_not_equal", lbl));
            output.push(format!("{}_a_hash:", lbl));
            // a has #, b should also have #
            output.push(format!("    case {}", tmp2));
            output.push(format!("    goto {}_not_equal", lbl));
            output.push(format!("    goto {}_not_equal", lbl));
            output.push(format!("    goto {}", lbl));
            output.push(format!("{}_equal:", lbl));
            output.push(format!("    add 1 {}", result));
            output.push(format!("{}_not_equal:", lbl));
            // Clean up any remaining in tmp2
            output.push(format!("    clear {}", tmp2));
            Ok((output, true))
        }
        "length" => {
            // Counts instructions in a 1# program
            // length <prog> <count> <tmp>
            // Counts sequences of 1s followed by #s
            if parts.len() != 4 {
                return Err(format!(
                    "Line {}: 'length' expects 3 arguments: length <prog> <count> <tmp>",
                    line_num
                ));
            }
            let (prog, count, tmp) = (parts[1], parts[2], parts[3]);
            let lbl = unique_label("len");
            output.push(format!("    clear {}", count));
            output.push(format!("    move {} {}", prog, tmp));
            // State machine: look for 1s, then #s
            output.push(format!("{}:", lbl));
            output.push(format!("    case {}", tmp));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("    goto {}_ones", lbl));
            output.push(format!("    goto {}", lbl)); // skip leading #s (shouldn't happen in valid program)
            output.push(format!("{}_ones:", lbl));
            // Reading 1s of an instruction
            output.push(format!("    case {}", tmp));
            output.push(format!("    goto {}_done", lbl)); // ended mid-instruction
            output.push(format!("    goto {}_ones", lbl)); // more 1s
            output.push(format!("    goto {}_hashes", lbl)); // found first #
            output.push(format!("{}_hashes:", lbl));
            // Reading #s of an instruction
            output.push(format!("    case {}", tmp));
            output.push(format!("    goto {}_count", lbl)); // end of program, count this instruction
            output.push(format!("    goto {}_count", lbl)); // start of new instruction
            output.push(format!("    goto {}_hashes", lbl)); // more #s
            output.push(format!("{}_count:", lbl));
            // Count this instruction
            output.push(format!("    add 1 {}", count));
            output.push(format!("    goto {}", lbl));
            output.push(format!("{}_done:", lbl));
            Ok((output, true))
        }
        "write" => {
            // Generates a program that outputs the input value
            // write <input> <output> <tmp>
            // For each symbol in input:
            //   1 -> output "1#" (add 1 to R1)
            //   # -> output "1##" (add # to R1)
            if parts.len() != 4 {
                return Err(format!(
                    "Line {}: 'write' expects 3 arguments: write <input> <output> <tmp>",
                    line_num
                ));
            }
            let (input, out, tmp) = (parts[1], parts[2], parts[3]);
            let lbl = unique_label("wrt");
            output.push(format!("    clear {}", out));
            output.push(format!("    move {} {}", input, tmp));
            output.push(format!("{}:", lbl));
            output.push(format!("    case {}", tmp));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("    goto {}_one", lbl));
            output.push(format!("    goto {}_hash", lbl));
            output.push(format!("{}_one:", lbl));
            // Emit "1#" (add 1 to R1)
            output.push(format!("    add 1 {}", out));
            output.push(format!("    add # {}", out));
            output.push(format!("    goto {}", lbl));
            output.push(format!("{}_hash:", lbl));
            // Emit "1##" (add # to R1)
            output.push(format!("    add 1 {}", out));
            output.push(format!("    add # {}", out));
            output.push(format!("    add # {}", out));
            output.push(format!("    goto {}", lbl));
            output.push(format!("{}_done:", lbl));
            Ok((output, true))
        }
        "diag" => {
            // Diagonalization: given x, produces write(x) + x
            // diag <reg> <tmp1> <tmp2> <tmp3>
            // Running the result gives phi_x(x)
            if parts.len() != 5 {
                return Err(format!(
                    "Line {}: 'diag' expects 4 arguments: diag <reg> <tmp1> <tmp2> <tmp3>",
                    line_num
                ));
            }
            let (reg, tmp1, tmp2, tmp3) = (parts[1], parts[2], parts[3], parts[4]);
            let lbl = unique_label("diag");
            // Copy input to tmp1 (to preserve original for appending)
            output.push(format!("    copy {} {} {}", reg, tmp1, tmp2));
            // Run write on the input, output to tmp2
            output.push(format!("    write {} {} {}", reg, tmp2, tmp3));
            // Now tmp2 has write(x), tmp1 has x
            // Move write(x) to reg
            output.push(format!("    move {} {}", tmp2, reg));
            // Append x to reg
            output.push(format!("    move {} {}", tmp1, reg));
            output.push(format!("{}:", lbl)); // dummy label for unique_label
            Ok((output, true))
        }
        "bump" => {
            // Bumps register numbers in a program by n
            // bump <prog> <n> <output> <tmp1> <tmp2> <tmp3>
            // n is a unary number (1^n)
            if parts.len() != 7 {
                return Err(format!(
                    "Line {}: 'bump' expects 6 arguments: bump <prog> <n> <output> <tmp1> <tmp2> <tmp3>",
                    line_num
                ));
            }
            let (prog, n, out, tmp1, tmp2, tmp3) = (parts[1], parts[2], parts[3], parts[4], parts[5], parts[6]);
            let lbl = unique_label("bump");
            output.push(format!("    clear {}", out));
            output.push(format!("    move {} {}", prog, tmp1));
            // Copy n to tmp3 for repeated use
            output.push(format!("    copy {} {} {}", n, tmp3, tmp2));
            output.push(format!("{}:", lbl));
            output.push(format!("    case {}", tmp1));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("    goto {}_ones", lbl));
            output.push(format!("    goto {}", lbl)); // skip (shouldn't happen)
            output.push(format!("{}_ones:", lbl));
            // Read 1s (register number)
            output.push(format!("    add 1 {}", tmp2)); // count 1s in tmp2
            output.push(format!("    case {}", tmp1));
            output.push(format!("    goto {}_emit", lbl)); // end of input
            output.push(format!("    goto {}_ones", lbl)); // more 1s
            output.push(format!("    goto {}_hash", lbl)); // found first #
            output.push(format!("{}_hash:", lbl));
            // Add bump amount to tmp2
            output.push(format!("    copy {} {} {}", tmp3, out, prog)); // borrow prog temporarily
            output.push(format!("{}_add_n:", lbl));
            output.push(format!("    case {}", out));
            output.push(format!("    goto {}_emit_1s", lbl));
            output.push(format!("    goto {}_add_n_1", lbl));
            output.push(format!("    goto {}_add_n", lbl));
            output.push(format!("{}_add_n_1:", lbl));
            output.push(format!("    add 1 {}", tmp2));
            output.push(format!("    goto {}_add_n", lbl));
            output.push(format!("{}_emit_1s:", lbl));
            // Emit the 1s from tmp2
            output.push(format!("    case {}", tmp2));
            output.push(format!("    goto {}_emit_hashes", lbl));
            output.push(format!("    goto {}_emit_1", lbl));
            output.push(format!("    goto {}_emit_1s", lbl)); // skip #s in tmp2 (shouldn't happen)
            output.push(format!("{}_emit_1:", lbl));
            output.push(format!("    add 1 {}", out));
            output.push(format!("    goto {}_emit_1s", lbl));
            output.push(format!("{}_emit_hashes:", lbl));
            // Emit #s until we hit a 1 or end
            output.push(format!("    add # {}", out));
            output.push(format!("    case {}", tmp1));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("    goto {}_ones", lbl)); // new instruction
            output.push(format!("    goto {}_emit_hashes", lbl)); // more #s
            output.push(format!("{}_emit:", lbl));
            // Edge case: emit remaining and done
            output.push(format!("    case {}", tmp2));
            output.push(format!("    goto {}_done", lbl));
            output.push(format!("    goto {}_emit_final", lbl));
            output.push(format!("    goto {}_emit", lbl));
            output.push(format!("{}_emit_final:", lbl));
            output.push(format!("    add 1 {}", out));
            output.push(format!("    goto {}_emit", lbl));
            output.push(format!("{}_done:", lbl));
            output.push(format!("    clear {}", tmp3));
            Ok((output, true))
        }
        _ => Ok((Vec::new(), false)),
    }
}

fn expand_bin_add(output: &mut Vec<String>, src: &str, dst: &str, tmp1: &str, carry: &str, lbl: &str) {
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
}

fn expand_bin_sub(output: &mut Vec<String>, src: &str, dst: &str, tmp1: &str, borrow: &str, lbl: &str) {
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
    output.push(format!("    goto {}_0_1", lbl));
    output.push(format!("    goto {}_0_0", lbl));
    output.push(format!("{}_both_e:", lbl));
    output.push(format!("    clear {}", borrow));
    output.push(format!("    goto {}_done", lbl));
    output.push(format!("{}_0_1:", lbl));
    output.push(format!("    case {}", borrow));
    output.push(format!("    goto {}_0_1_b0", lbl));
    output.push(format!("    goto {}_0_1_b1", lbl));
    output.push(format!("    goto {}_0_1_b1", lbl));
    output.push(format!("{}_0_1_b0:", lbl));
    output.push(format!("    add 1 {}", dst));
    output.push(format!("    add 1 {}", borrow));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_0_1_b1:", lbl));
    output.push(format!("    add # {}", dst));
    output.push(format!("    add 1 {}", borrow));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_0_0:", lbl));
    output.push(format!("    case {}", borrow));
    output.push(format!("    goto {}_0_0_b0", lbl));
    output.push(format!("    goto {}_0_0_b1", lbl));
    output.push(format!("    goto {}_0_0_b1", lbl));
    output.push(format!("{}_0_0_b0:", lbl));
    output.push(format!("    add # {}", dst));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_0_0_b1:", lbl));
    output.push(format!("    add 1 {}", dst));
    output.push(format!("    add 1 {}", borrow));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_t1_1:", lbl));
    output.push(format!("    case {}", src));
    output.push(format!("    goto {}_1_e", lbl));
    output.push(format!("    goto {}_1_1", lbl));
    output.push(format!("    goto {}_1_0", lbl));
    output.push(format!("{}_1_e:", lbl));
    output.push(format!("    case {}", borrow));
    output.push(format!("    goto {}_1_e_b0", lbl));
    output.push(format!("    goto {}_1_e_b1", lbl));
    output.push(format!("    goto {}_1_e_b1", lbl));
    output.push(format!("{}_1_e_b0:", lbl));
    output.push(format!("    add 1 {}", dst));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_1_e_b1:", lbl));
    output.push(format!("    add # {}", dst));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_1_1:", lbl));
    output.push(format!("    case {}", borrow));
    output.push(format!("    goto {}_1_1_b0", lbl));
    output.push(format!("    goto {}_1_1_b1", lbl));
    output.push(format!("    goto {}_1_1_b1", lbl));
    output.push(format!("{}_1_1_b0:", lbl));
    output.push(format!("    add # {}", dst));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_1_1_b1:", lbl));
    output.push(format!("    add 1 {}", dst));
    output.push(format!("    add 1 {}", borrow));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_1_0:", lbl));
    output.push(format!("    case {}", borrow));
    output.push(format!("    goto {}_1_0_b0", lbl));
    output.push(format!("    goto {}_1_0_b1", lbl));
    output.push(format!("    goto {}_1_0_b1", lbl));
    output.push(format!("{}_1_0_b0:", lbl));
    output.push(format!("    add 1 {}", dst));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_1_0_b1:", lbl));
    output.push(format!("    add # {}", dst));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_t1_h:", lbl));
    output.push(format!("    case {}", src));
    output.push(format!("    goto {}_0_e", lbl));
    output.push(format!("    goto {}_h_1", lbl));
    output.push(format!("    goto {}_h_0", lbl));
    output.push(format!("{}_0_e:", lbl));
    output.push(format!("    case {}", borrow));
    output.push(format!("    goto {}_0_e_b0", lbl));
    output.push(format!("    goto {}_0_e_b1", lbl));
    output.push(format!("    goto {}_0_e_b1", lbl));
    output.push(format!("{}_0_e_b0:", lbl));
    output.push(format!("    add # {}", dst));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_0_e_b1:", lbl));
    output.push(format!("    add 1 {}", dst));
    output.push(format!("    add 1 {}", borrow));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_h_1:", lbl));
    output.push(format!("    case {}", borrow));
    output.push(format!("    goto {}_h_1_b0", lbl));
    output.push(format!("    goto {}_h_1_b1", lbl));
    output.push(format!("    goto {}_h_1_b1", lbl));
    output.push(format!("{}_h_1_b0:", lbl));
    output.push(format!("    add 1 {}", dst));
    output.push(format!("    add 1 {}", borrow));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_h_1_b1:", lbl));
    output.push(format!("    add # {}", dst));
    output.push(format!("    add 1 {}", borrow));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_h_0:", lbl));
    output.push(format!("    case {}", borrow));
    output.push(format!("    goto {}_h_0_b0", lbl));
    output.push(format!("    goto {}_h_0_b1", lbl));
    output.push(format!("    goto {}_h_0_b1", lbl));
    output.push(format!("{}_h_0_b0:", lbl));
    output.push(format!("    add # {}", dst));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_h_0_b1:", lbl));
    output.push(format!("    add 1 {}", dst));
    output.push(format!("    add 1 {}", borrow));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_done:", lbl));
}

fn expand_compare(
    output: &mut Vec<String>,
    a: &str,
    b: &str,
    tmp1: &str,
    tmp2: &str,
    tmp3: &str,
    lt: &str,
    eq: &str,
    gt: &str,
    lbl: &str,
) {
    output.push(format!("    copy {} {} {}", a, tmp1, tmp3));
    output.push(format!("    copy {} {} {}", b, tmp2, tmp3));
    output.push(format!("{}_loop:", lbl));
    output.push(format!("    case {}", tmp1));
    output.push(format!("    goto {}_a_empty", lbl));
    output.push(format!("    goto {}_a_1", lbl));
    output.push(format!("    goto {}_a_0", lbl));
    output.push(format!("{}_a_empty:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}", eq));
    output.push(format!("    goto {}_drain_lt", lbl));
    output.push(format!("    goto {}_a_empty", lbl));
    output.push(format!("{}_a_1:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}_drain_gt", lbl));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("    goto {}_diff_a_gt", lbl));
    output.push(format!("{}_a_0:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("    goto {}_diff_a_lt", lbl));
    output.push(format!("    goto {}_loop", lbl));
    output.push(format!("{}_diff_a_gt:", lbl));
    output.push(format!("    case {}", tmp1));
    output.push(format!("    goto {}_dgt_a_e", lbl));
    output.push(format!("    goto {}_dgt_a1", lbl));
    output.push(format!("    goto {}_dgt_a0", lbl));
    output.push(format!("{}_dgt_a1:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}_drain_gt", lbl));
    output.push(format!("    goto {}_diff_a_gt", lbl));
    output.push(format!("    goto {}_diff_a_gt", lbl));
    output.push(format!("{}_dgt_a0:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}_diff_a_gt", lbl));
    output.push(format!("    goto {}_diff_a_lt", lbl));
    output.push(format!("    goto {}_diff_a_gt", lbl));
    output.push(format!("{}_dgt_a_e:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}", gt));
    output.push(format!("    goto {}_drain_lt", lbl));
    output.push(format!("    goto {}_dgt_a_e", lbl));
    output.push(format!("{}_diff_a_lt:", lbl));
    output.push(format!("    case {}", tmp1));
    output.push(format!("    goto {}_dlt_a_e", lbl));
    output.push(format!("    goto {}_dlt_a1", lbl));
    output.push(format!("    goto {}_dlt_a0", lbl));
    output.push(format!("{}_dlt_a1:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}_drain_gt", lbl));
    output.push(format!("    goto {}_diff_a_lt", lbl));
    output.push(format!("    goto {}_diff_a_gt", lbl));
    output.push(format!("{}_dlt_a0:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}_diff_a_lt", lbl));
    output.push(format!("    goto {}_diff_a_lt", lbl));
    output.push(format!("    goto {}_diff_a_lt", lbl));
    output.push(format!("{}_dlt_a_e:", lbl));
    output.push(format!("    case {}", tmp2));
    output.push(format!("    goto {}", lt));
    output.push(format!("    goto {}_drain_lt", lbl));
    output.push(format!("    goto {}_dlt_a_e", lbl));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_macro() {
        reset_label_counter();
        let source = r#"
.macro addone reg
    add 1 reg
.endmacro

addone R1
addone R2
"#;
        let expanded = expand_macros(source).unwrap();
        assert!(expanded.contains("add 1 R1"));
        assert!(expanded.contains("add 1 R2"));
    }

    #[test]
    fn test_clear_macro() {
        reset_label_counter();
        let expanded = expand_macros("clear R1").unwrap();
        assert!(expanded.contains("case R1"));
    }

    #[test]
    fn test_move_macro() {
        reset_label_counter();
        let expanded = expand_macros("move R1 R2").unwrap();
        assert!(expanded.contains("case R1"));
        assert!(expanded.contains("add 1 R2"));
        assert!(expanded.contains("add # R2"));
    }

    #[test]
    fn test_compare_eq_macro() {
        reset_label_counter();
        let expanded = expand_macros("compare_eq R1 R2 R3 R4 R5").unwrap();
        // Fully expanded - should have case instructions for comparison
        assert!(expanded.contains("case R4")); // tmp1
        assert!(expanded.contains("case R5")); // tmp2
        assert!(expanded.contains("add 1 R3")); // result = 1 if equal
    }

    #[test]
    fn test_length_macro() {
        reset_label_counter();
        let expanded = expand_macros("length R1 R2 R3").unwrap();
        // Fully expanded - should have case R2 for clearing and case R3 for reading
        assert!(expanded.contains("case R2")); // clear R2
        assert!(expanded.contains("case R3")); // reading symbols
        assert!(expanded.contains("add 1 R2")); // counting
    }

    #[test]
    fn test_write_macro() {
        reset_label_counter();
        let expanded = expand_macros("write R1 R2 R3").unwrap();
        // Fully expanded - should have case R2 for clearing and output to R2
        assert!(expanded.contains("case R2")); // clear R2
        assert!(expanded.contains("case R3")); // reading from tmp
        assert!(expanded.contains("add 1 R2"));
        assert!(expanded.contains("add # R2"));
    }

    #[test]
    fn test_diag_macro() {
        reset_label_counter();
        let expanded = expand_macros("diag R1 R2 R3 R4").unwrap();
        // diag uses copy, write, move - all expanded
        assert!(expanded.contains("case R1")); // copy source
        assert!(expanded.contains("add 1 R2")); // output from write
    }

    #[test]
    fn test_bump_macro() {
        reset_label_counter();
        let expanded = expand_macros("bump R1 R2 R3 R4 R5 R6").unwrap();
        // Fully expanded
        assert!(expanded.contains("case R3")); // clear R3
        assert!(expanded.contains("case R4")); // reading program
        assert!(expanded.contains("add 1 R3")); // output
    }

    #[test]
    fn test_parametric_macro_wrong_args() {
        reset_label_counter();
        // These macros require specific argument counts
        let result = expand_macros("compare_eq R1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expects 5 arguments"));

        let result = expand_macros("length R1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expects 3 arguments"));
    }
}
