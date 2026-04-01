use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};

/// A symbol in the 1# alphabet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Symbol {
    One,
    Hash,
}

impl Symbol {
    fn to_char(self) -> char {
        match self {
            Symbol::One => '1',
            Symbol::Hash => '#',
        }
    }
}

/// A word is a sequence of symbols
type Word = Vec<Symbol>;

fn word_to_string(word: &Word) -> String {
    word.iter().map(|s| s.to_char()).collect()
}

/// Convert a backwards binary word to decimal
/// In backwards binary: LSB is first, 1=1, #=0
/// Example: #11 = 011 reversed = 110 binary = 6 decimal
fn word_to_decimal(word: &Word) -> Option<u128> {
    if word.is_empty() {
        return Some(0);
    }
    let mut value: u128 = 0;
    for (i, symbol) in word.iter().enumerate() {
        if i >= 128 {
            return None; // Too large
        }
        if *symbol == Symbol::One {
            value |= 1u128 << i;
        }
    }
    Some(value)
}

/// Format a word as both backwards binary and decimal
fn format_word(word: &Word) -> String {
    let bb = word_to_string(word);
    if bb.is_empty() {
        return "(empty) = 0".to_string();
    }
    match word_to_decimal(word) {
        Some(dec) => format!("{} = {}", bb, dec),
        None => format!("{} = (overflow)", bb),
    }
}

fn string_to_word(s: &str) -> Word {
    s.chars()
        .filter_map(|c| match c {
            '1' => Some(Symbol::One),
            '#' => Some(Symbol::Hash),
            _ => None,
        })
        .collect()
}

/// The five instruction types in 1#
#[derive(Debug, Clone, PartialEq, Eq)]
enum Instruction {
    /// Add 1 to register n (1^n#)
    AddOne(usize),
    /// Add # to register n (1^n##)
    AddHash(usize),
    /// Go forward n instructions (1^n###)
    Forward(usize),
    /// Go backward n instructions (1^n####)
    Backward(usize),
    /// Case on register n (1^n#####)
    Case(usize),
}

impl Instruction {
    fn to_string(&self) -> String {
        match self {
            Instruction::AddOne(n) => format!("{}#", "1".repeat(*n)),
            Instruction::AddHash(n) => format!("{}##", "1".repeat(*n)),
            Instruction::Forward(n) => format!("{}###", "1".repeat(*n)),
            Instruction::Backward(n) => format!("{}####", "1".repeat(*n)),
            Instruction::Case(n) => format!("{}#####", "1".repeat(*n)),
        }
    }

    fn describe(&self) -> String {
        match self {
            Instruction::AddOne(n) => format!("Add 1 to R{}", n),
            Instruction::AddHash(n) => format!("Add # to R{}", n),
            Instruction::Forward(n) => format!("Go forward {}", n),
            Instruction::Backward(n) => format!("Go backward {}", n),
            Instruction::Case(n) => format!("Cases on R{}", n),
        }
    }
}

/// Parse a 1# program into a list of instructions
fn parse_program(source: &str) -> Result<Vec<Instruction>, String> {
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

/// Create an instruction from a count of ones and hashes
fn make_instruction(ones: usize, hashes: usize) -> Result<Option<Instruction>, String> {
    if ones == 0 {
        return Err(format!("Invalid instruction: {} hashes with no ones", hashes));
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

/// The state of the 1# machine
struct Machine {
    registers: HashMap<usize, Word>,
    program: Vec<Instruction>,
    pc: usize, // Program counter (0-indexed)
    step_count: usize,
    max_steps: Option<usize>,
}

/// The result of running the machine
#[derive(Debug)]
enum RunResult {
    Halted,
    StoppedImproperly,
    MaxStepsReached,
}

impl Machine {
    fn new(program: Vec<Instruction>) -> Self {
        Machine {
            registers: HashMap::new(),
            program,
            pc: 0,
            step_count: 0,
            max_steps: None,
        }
    }

    fn with_max_steps(mut self, max: usize) -> Self {
        self.max_steps = Some(max);
        self
    }

    fn set_register(&mut self, n: usize, word: Word) {
        self.registers.insert(n, word);
    }

    fn get_register(&self, n: usize) -> &Word {
        static EMPTY: Word = Vec::new();
        self.registers.get(&n).unwrap_or(&EMPTY)
    }

    fn get_register_mut(&mut self, n: usize) -> &mut Word {
        self.registers.entry(n).or_insert_with(Vec::new)
    }

    /// Run the machine until it halts or stops improperly
    fn run(&mut self) -> RunResult {
        loop {
            if let Some(max) = self.max_steps {
                if self.step_count >= max {
                    return RunResult::MaxStepsReached;
                }
            }

            match self.step() {
                StepResult::Continue => {}
                StepResult::Halted => return RunResult::Halted,
                StepResult::StoppedImproperly => return RunResult::StoppedImproperly,
            }
        }
    }

    /// Execute a single step, returning whether to continue
    fn step(&mut self) -> StepResult {
        let n = self.program.len();

        // Check if we've halted (pc is exactly at n, one past the last instruction)
        if self.pc == n {
            return StepResult::Halted;
        }

        // Check if we've stopped improperly
        if self.pc > n {
            return StepResult::StoppedImproperly;
        }

        let instruction = &self.program[self.pc].clone();
        self.step_count += 1;

        match instruction {
            Instruction::AddOne(reg) => {
                self.get_register_mut(*reg).push(Symbol::One);
                self.pc += 1;
            }
            Instruction::AddHash(reg) => {
                self.get_register_mut(*reg).push(Symbol::Hash);
                self.pc += 1;
            }
            Instruction::Forward(offset) => {
                self.pc += offset;
            }
            Instruction::Backward(offset) => {
                if *offset > self.pc {
                    // Going before the start of the program
                    return StepResult::StoppedImproperly;
                }
                self.pc -= offset;
            }
            Instruction::Case(reg) => {
                let word = self.get_register_mut(*reg);
                if word.is_empty() {
                    // Empty: go to next instruction
                    self.pc += 1;
                } else {
                    let first = word.remove(0);
                    match first {
                        Symbol::One => {
                            // First symbol is 1: go to second instruction after case
                            self.pc += 2;
                        }
                        Symbol::Hash => {
                            // First symbol is #: go to third instruction after case
                            self.pc += 3;
                        }
                    }
                }
            }
        }

        // Check bounds after the step
        if self.pc == n {
            StepResult::Halted
        } else if self.pc > n {
            // Forward jumped too far
            StepResult::StoppedImproperly
        } else {
            StepResult::Continue
        }
    }

    /// Print the current state of the machine
    fn print_state(&self) {
        println!("PC: {} / {}", self.pc + 1, self.program.len());
        if self.pc < self.program.len() {
            println!(
                "Next: {} ({})",
                self.program[self.pc].to_string(),
                self.program[self.pc].describe()
            );
        }
        println!("Steps: {}", self.step_count);
        println!("Registers:");
        let mut regs: Vec<_> = self.registers.iter().collect();
        regs.sort_by_key(|(k, _)| *k);
        for (n, word) in regs {
            if !word.is_empty() {
                println!("  R{}: {}", n, word_to_string(word));
            }
        }
    }
}

#[derive(Debug)]
enum StepResult {
    Continue,
    Halted,
    StoppedImproperly,
}

fn print_parsed_program(instructions: &[Instruction]) {
    println!("Parsed program ({} instructions):", instructions.len());
    for (i, instr) in instructions.iter().enumerate() {
        println!("  {}: {} ({})", i + 1, instr.to_string(), instr.describe());
    }
}

fn run_program(source: &str, initial_registers: &[(usize, &str)], verbose: bool, max_steps: Option<usize>) {
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
    println!("Steps executed: {}", machine.step_count);
    println!();
    println!("Final registers:");
    let mut regs: Vec<_> = machine.registers.iter().collect();
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

fn repl() {
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
        } else if line.starts_with(":parse ") {
            let program = &line[7..];
            match parse_program(program) {
                Ok(instrs) => print_parsed_program(&instrs),
                Err(e) => eprintln!("Parse error: {}", e),
            }
        } else if line.starts_with(":run ") {
            let program = &line[5..];
            let reg_refs: Vec<(usize, &str)> = registers.iter().map(|(n, s)| (*n, s.as_str())).collect();
            run_program(program, &reg_refs, true, Some(100000));
        } else if line.starts_with(":set ") {
            let rest = &line[5..];
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
        } else if line.starts_with(":load ") {
            let filename = &line[6..];
            match fs::read_to_string(filename) {
                Ok(contents) => {
                    let reg_refs: Vec<(usize, &str)> = registers.iter().map(|(n, s)| (*n, s.as_str())).collect();
                    run_program(&contents, &reg_refs, true, Some(100000));
                }
                Err(e) => eprintln!("Error loading file: {}", e),
            }
        } else if line.starts_with(':') {
            eprintln!("Unknown command. Type :help for help.");
        } else {
            // Assume it's a program to run
            let reg_refs: Vec<(usize, &str)> = registers.iter().map(|(n, s)| (*n, s.as_str())).collect();
            run_program(line, &reg_refs, true, Some(100000));
        }
        println!();
    }
}

fn print_usage() {
    eprintln!("Usage: one_hash [OPTIONS] [PROGRAM_FILE]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -e <program>      Execute program string directly");
    eprintln!("  -r<n> <word>      Set register n to word (e.g., -r1 11#)");
    eprintln!("  -v                Verbose output");
    eprintln!("  -m <steps>        Maximum steps (default: 1000000)");
    eprintln!("  --repl            Start interactive REPL");
    eprintln!("  -h, --help        Show this help");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  one_hash -e '1#11##'");
    eprintln!("  one_hash -e '11#####1###1###1###' -r2 '1#1'");
    eprintln!("  one_hash program.1#");
    eprintln!("  one_hash --repl");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        repl();
        return;
    }

    let mut program_source: Option<String> = None;
    let mut registers: Vec<(usize, String)> = Vec::new();
    let mut verbose = false;
    let mut max_steps = 1000000usize;
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        if arg == "-h" || arg == "--help" {
            print_usage();
            return;
        } else if arg == "--repl" {
            repl();
            return;
        } else if arg == "-v" {
            verbose = true;
        } else if arg == "-e" {
            i += 1;
            if i >= args.len() {
                eprintln!("Error: -e requires a program argument");
                return;
            }
            program_source = Some(args[i].clone());
        } else if arg == "-m" {
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
        } else if arg.starts_with('-') {
            eprintln!("Unknown option: {}", arg);
            print_usage();
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
            let reg_refs: Vec<(usize, &str)> = registers.iter().map(|(n, s)| (*n, s.as_str())).collect();
            run_program(&source, &reg_refs, verbose, Some(max_steps));
        }
        None => {
            eprintln!("No program specified");
            print_usage();
        }
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
    fn test_add_one() {
        let instrs = parse_program("1#").unwrap();
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("11#"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "11#1");
    }

    #[test]
    fn test_add_hash() {
        let instrs = parse_program("1##").unwrap();
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("11"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "11#");
    }

    #[test]
    fn test_forward_halt() {
        // 1### goes forward 1, which halts properly
        let instrs = parse_program("1###").unwrap();
        let mut machine = Machine::new(instrs);
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
    }

    #[test]
    fn test_forward_improper() {
        // 11### goes forward 2, which stops improperly
        let instrs = parse_program("11###").unwrap();
        let mut machine = Machine::new(instrs);
        let result = machine.run();
        assert!(matches!(result, RunResult::StoppedImproperly));
    }

    #[test]
    fn test_case_empty() {
        // Case on R1 (empty) -> next instruction adds 1 to R2
        let instrs = parse_program("1##### 11#").unwrap();
        let mut machine = Machine::new(instrs);
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(2)), "1");
    }

    #[test]
    fn test_case_one() {
        // Case on R1 (starts with 1) -> skip one, go to second instruction after
        // Instruction 0: case on R1
        // Instruction 1: add 1 to R2 (skipped)
        // Instruction 2: add # to R2 (executed when R1 starts with 1)
        let instrs = parse_program("1##### 11# 11##").unwrap();
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("1#"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "#"); // 1 was removed
        assert_eq!(word_to_string(machine.get_register(2)), "#"); // # was added
    }

    #[test]
    fn test_case_hash() {
        // Case on R1 (starts with #) -> skip two, go to third instruction after
        // Instruction 0: case on R1
        // Instruction 1: add 1 to R2 (skipped)
        // Instruction 2: add 1 to R3 (skipped)
        // Instruction 3: add # to R2 (executed when R1 starts with #)
        let instrs = parse_program("1##### 11# 111# 11##").unwrap();
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("#1"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "1"); // # was removed
        assert_eq!(word_to_string(machine.get_register(2)), "#"); // # was added
    }

    #[test]
    fn test_move_program() {
        // move2,1: moves contents of R2 to R1
        let program = "11#####111111###111###1##1111####1#111111####";
        let instrs = parse_program(program).unwrap();
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word(""));
        machine.set_register(2, string_to_word("1#1"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "1#1");
        assert_eq!(word_to_string(machine.get_register(2)), "");
    }

    #[test]
    fn test_infinite_loop_detection() {
        // 1### 1#### is an infinite loop
        let instrs = parse_program("1###1####").unwrap();
        let mut machine = Machine::new(instrs).with_max_steps(100);
        let result = machine.run();
        assert!(matches!(result, RunResult::MaxStepsReached));
    }

    #[test]
    fn test_pop1() {
        // pop1: removes first symbol from R1
        let program = "1##### 1### 1###";
        let instrs = parse_program(program).unwrap();
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("1#1"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "#1");
    }

    #[test]
    fn test_comments() {
        let instrs = parse_program("1# ; add 1 to R1\n11## ; add # to R2").unwrap();
        assert_eq!(instrs.len(), 2);
        assert_eq!(instrs[0], Instruction::AddOne(1));
        assert_eq!(instrs[1], Instruction::AddHash(2));
    }
}
