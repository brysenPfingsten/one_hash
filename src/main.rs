//! 1# Interpreter and Assembler CLI
//!
//! A unified command-line interface for the 1# language.

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};

use one_hash::{
    compile, compile_verbose, expand_only, format_word, parse_program, print_parsed_program,
    string_to_word, Machine,
};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        interpreter_repl();
        return;
    }

    // Check for --asm flag
    if args.iter().any(|a| a == "--asm" || a == "-a") {
        run_assembler(args);
    } else {
        run_interpreter(args);
    }
}

// ============================================================================
// Interpreter
// ============================================================================

fn run_interpreter(args: Vec<String>) {
    let mut program_source: Option<String> = None;
    let mut registers: Vec<(usize, String)> = Vec::new();
    let mut verbose = false;
    let mut max_steps = 1000000usize;
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        if arg == "-h" || arg == "--help" {
            print_interpreter_usage();
            return;
        } else if arg == "--repl" {
            interpreter_repl();
            return;
        } else if arg == "-v" || arg == "--verbose" {
            verbose = true;
        } else if arg == "-e" {
            i += 1;
            if i >= args.len() {
                eprintln!("Error: -e requires a program argument");
                return;
            }
            program_source = Some(args[i].clone());
        } else if arg == "-m" || arg == "--max-steps" {
            i += 1;
            if i >= args.len() {
                eprintln!("Error: -m requires a number argument");
                return;
            }
            max_steps = match args[i].parse() {
                Ok(n) => n,
                Err(_) => {
                    eprintln!("Error: invalid number for -m");
                    return;
                }
            };
        } else if arg.starts_with("-r") {
            let reg_num = &arg[2..];
            let n: usize = match reg_num.parse() {
                Ok(n) => n,
                Err(_) => {
                    eprintln!("Error: invalid register number in {}", arg);
                    return;
                }
            };
            i += 1;
            if i >= args.len() {
                eprintln!("Error: {} requires a word argument", arg);
                return;
            }
            registers.push((n, args[i].clone()));
        } else if arg.starts_with('-') && arg != "--asm" && arg != "-a" {
            eprintln!("Unknown option: {}", arg);
            print_interpreter_usage();
            return;
        } else {
            // Assume it's a file
            match fs::read_to_string(arg) {
                Ok(contents) => program_source = Some(contents),
                Err(e) => {
                    eprintln!("Error reading file '{}': {}", arg, e);
                    return;
                }
            }
        }
        i += 1;
    }

    match program_source {
        Some(source) => {
            let reg_refs: Vec<(usize, &str)> =
                registers.iter().map(|(n, s)| (*n, s.as_str())).collect();
            run_program(&source, &reg_refs, verbose, Some(max_steps));
        }
        None => {
            eprintln!("No program specified");
            print_interpreter_usage();
        }
    }
}

fn run_program(
    source: &str,
    initial_registers: &[(usize, &str)],
    verbose: bool,
    max_steps: Option<usize>,
) {
    let instructions = match parse_program(source) {
        Ok(instrs) => instrs,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            return;
        }
    };

    if instructions.is_empty() {
        println!("Empty program");
        return;
    }

    if verbose {
        print_parsed_program(&instructions);
        println!();
    }

    let mut machine = Machine::new(instructions);
    if let Some(max) = max_steps {
        machine = machine.with_max_steps(max);
    }

    for (reg, value) in initial_registers {
        machine.set_register(*reg, string_to_word(value));
    }

    if verbose {
        println!("Initial state:");
        machine.print_state();
        println!();
    }

    let result = machine.run();

    println!("Result: {:?}", result);
    println!("Steps executed: {}", machine.step_count());
    println!();
    println!("Final registers:");
    let mut regs: Vec<_> = machine.registers().iter().collect();
    regs.sort_by_key(|(k, _)| *k);
    let mut any_nonempty = false;
    for (n, word) in &regs {
        if !word.is_empty() {
            println!("  R{}: {}", n, format_word(word));
            any_nonempty = true;
        }
    }
    if !any_nonempty {
        println!("  (all empty)");
    }
}

fn interpreter_repl() {
    println!("1# Interpreter REPL");
    println!("Commands:");
    println!("  :parse <program>  - Parse and show instructions");
    println!("  :run <program>    - Run a program");
    println!("  :set R<n> <word>  - Set register n to word");
    println!("  :clear            - Clear all registers");
    println!("  :regs             - Show all registers");
    println!("  :load <file>      - Load program from file");
    println!("  :help             - Show this help");
    println!("  :quit             - Exit");
    println!();
    println!("Or just type a program to run it.");
    println!();

    let stdin = io::stdin();
    let mut registers: Vec<(usize, String)> = Vec::new();

    loop {
        print!("1#> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            break;
        }
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with(":quit") || line.starts_with(":q") {
            break;
        } else if line.starts_with(":help") || line.starts_with(":h") {
            println!("Commands:");
            println!("  :parse <program>  - Parse and show instructions");
            println!("  :run <program>    - Run a program");
            println!("  :set R<n> <word>  - Set register n to word");
            println!("  :clear            - Clear all registers");
            println!("  :regs             - Show all registers");
            println!("  :load <file>      - Load program from file");
            println!("  :help             - Show this help");
            println!("  :quit             - Exit");
        } else if let Some(program) = line.strip_prefix(":parse ") {
            match parse_program(program) {
                Ok(instrs) => print_parsed_program(&instrs),
                Err(e) => eprintln!("Parse error: {}", e),
            }
        } else if let Some(program) = line.strip_prefix(":run ") {
            let reg_refs: Vec<(usize, &str)> =
                registers.iter().map(|(n, s)| (*n, s.as_str())).collect();
            run_program(program, &reg_refs, true, Some(100000));
        } else if let Some(rest) = line.strip_prefix(":set ") {
            if let Some(rest) = rest.strip_prefix('R') {
                let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    if let Ok(n) = parts[0].parse::<usize>() {
                        let word = parts[1].to_string();
                        registers.retain(|(reg, _)| *reg != n);
                        if !word.is_empty() {
                            registers.push((n, word));
                        }
                        println!("R{} = {}", n, parts[1]);
                    } else {
                        eprintln!("Invalid register number");
                    }
                } else {
                    eprintln!("Usage: :set R<n> <word>");
                }
            } else {
                eprintln!("Usage: :set R<n> <word>");
            }
        } else if line.starts_with(":clear") {
            registers.clear();
            println!("All registers cleared");
        } else if line.starts_with(":regs") {
            if registers.is_empty() {
                println!("All registers empty");
            } else {
                let mut regs = registers.clone();
                regs.sort_by_key(|(n, _)| *n);
                for (n, word) in &regs {
                    println!("  R{}: {}", n, word);
                }
            }
        } else if let Some(filename) = line.strip_prefix(":load ") {
            match fs::read_to_string(filename) {
                Ok(contents) => {
                    let reg_refs: Vec<(usize, &str)> =
                        registers.iter().map(|(n, s)| (*n, s.as_str())).collect();
                    run_program(&contents, &reg_refs, true, Some(100000));
                }
                Err(e) => eprintln!("Error loading file: {}", e),
            }
        } else if line.starts_with(':') {
            eprintln!("Unknown command. Type :help for help.");
        } else {
            // Assume it's a program to run
            let reg_refs: Vec<(usize, &str)> =
                registers.iter().map(|(n, s)| (*n, s.as_str())).collect();
            run_program(line, &reg_refs, true, Some(100000));
        }
        println!();
    }
}

fn print_interpreter_usage() {
    eprintln!("Usage: one_hash [OPTIONS] [PROGRAM_FILE]");
    eprintln!();
    eprintln!("Run 1# programs.");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -e <program>      Execute program string directly");
    eprintln!("  -r<n> <word>      Set register n to word (e.g., -r1 11#)");
    eprintln!("  -v, --verbose     Verbose output");
    eprintln!("  -m, --max-steps <n>  Maximum steps (default: 1000000)");
    eprintln!("  --repl            Start interactive REPL");
    eprintln!("  --asm, -a         Run in assembler mode");
    eprintln!("  -h, --help        Show this help");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  one_hash -e '1#11##'                  # Run inline program");
    eprintln!("  one_hash -e '11#####1###1###' -r1 '1#1'  # With register preset");
    eprintln!("  one_hash program.1h                   # Run from file");
    eprintln!("  one_hash --repl                       # Interactive mode");
    eprintln!();
    eprintln!("Assembler mode:");
    eprintln!("  one_hash --asm input.asm              # Compile assembly to stdout");
    eprintln!("  one_hash --asm input.asm -o out.1h    # Compile to file");
}

// ============================================================================
// Assembler
// ============================================================================

fn run_assembler(args: Vec<String>) {
    let mut source: Option<String> = None;
    let mut output_file: Option<String> = None;
    let mut verbose = false;
    let mut expand_only_flag = false;
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        if arg == "-h" || arg == "--help" {
            print_assembler_usage();
            return;
        } else if arg == "--asm" || arg == "-a" {
            // Already in assembler mode, skip
        } else if arg == "-v" || arg == "--verbose" {
            verbose = true;
        } else if arg == "-E" || arg == "--expand" {
            expand_only_flag = true;
        } else if arg == "-o" || arg == "--output" {
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
        } else if arg == "--repl" {
            assembler_repl();
            return;
        } else if arg.starts_with('-') {
            eprintln!("Unknown option: {}", arg);
            print_assembler_usage();
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
            print_assembler_usage();
            return;
        }
    };

    if expand_only_flag {
        match expand_only(&source) {
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

fn assembler_repl() {
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
            match expand_only(&code_buffer) {
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
            print_assembler_usage();
        } else {
            code_buffer.push_str(&line);
        }
    }
}

fn print_assembler_usage() {
    eprintln!("Usage: one_hash --asm [OPTIONS] <FILE>");
    eprintln!();
    eprintln!("Compile 1# assembly to machine code.");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o, --output <file>  Output to file instead of stdout");
    eprintln!("  -v, --verbose        Show compilation steps");
    eprintln!("  -e <code>            Compile code from command line");
    eprintln!("  -E, --expand         Expand macros only (don't compile)");
    eprintln!("  --repl               Start interactive REPL");
    eprintln!("  -h, --help           Show this help");
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
    eprintln!("  clear <reg>                           Set register to empty");
    eprintln!("  move <src> <dst>                      Move src to dst, empty src");
    eprintln!("  copy <src> <dst> <tmp>                Copy src to dst (preserves src)");
    eprintln!("  swap <a> <b> <tmp>                    Swap a and b");
    eprintln!("  pop <reg>                             Remove first symbol");
    eprintln!("  shift_left <reg> <tmp>                Prepend # (multiply by 2)");
    eprintln!("  shift_right <reg>                     Remove LSB (divide by 2)");
    eprintln!("  increment <reg> <tmp>                 Add 1 (binary)");
    eprintln!("  decrement <reg> <tmp>                 Subtract 1 (binary)");
    eprintln!("  is_nonzero <r> <t1> <t2> <nz> <z>     Branch on nonzero");
    eprintln!("  bin_add <src> <dst> <t1> <t2> <c>     Binary addition");
    eprintln!("  bin_sub <src> <dst> <t1> <t2> <b>     Binary subtraction");
    eprintln!("  multiply <a> <b> <dst> <t1-t5>        Binary multiplication");
    eprintln!("  compare <a> <b> <t1-t3> <lt> <eq> <gt>  Comparison branch");
    eprintln!("  divide <dd> <dv> <q> <r> <t1-t4>      Division with remainder");
    eprintln!();
    eprintln!("User-defined macros:");
    eprintln!("  .macro <name> <param1> <param2> ...   Begin macro definition");
    eprintln!("  .endmacro                             End macro definition");
    eprintln!("  @label                                Local label (unique per call)");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  one_hash --asm program.asm            # Compile to stdout");
    eprintln!("  one_hash --asm program.asm -o out.1h  # Compile to file");
    eprintln!("  one_hash --asm -e 'add 1 R1'          # Compile inline");
}
