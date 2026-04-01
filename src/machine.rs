//! The 1# virtual machine.
//!
//! Executes 1# programs on a register-based machine.

use std::collections::HashMap;

use crate::instruction::Instruction;
use crate::types::{word_to_string, Symbol, Word};

/// The result of running the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunResult {
    /// The program halted normally (PC reached end of program).
    Halted,
    /// The program stopped improperly (PC went out of bounds).
    StoppedImproperly,
    /// The maximum step limit was reached.
    MaxStepsReached,
}

/// The result of a single step.
#[derive(Debug)]
pub enum StepResult {
    Continue,
    Halted,
    StoppedImproperly,
}

/// The state of the 1# machine.
pub struct Machine {
    registers: HashMap<usize, Word>,
    program: Vec<Instruction>,
    pc: usize,
    step_count: usize,
    max_steps: Option<usize>,
}

impl Machine {
    /// Creates a new machine with the given program.
    pub fn new(program: Vec<Instruction>) -> Self {
        Machine {
            registers: HashMap::new(),
            program,
            pc: 0,
            step_count: 0,
            max_steps: None,
        }
    }

    /// Sets the maximum number of steps before stopping.
    pub fn with_max_steps(mut self, max: usize) -> Self {
        self.max_steps = Some(max);
        self
    }

    /// Sets a register to the given word.
    pub fn set_register(&mut self, n: usize, word: Word) {
        self.registers.insert(n, word);
    }

    /// Gets the contents of a register.
    pub fn get_register(&self, n: usize) -> &Word {
        static EMPTY: Word = Vec::new();
        self.registers.get(&n).unwrap_or(&EMPTY)
    }

    /// Gets a mutable reference to a register, creating it if needed.
    fn get_register_mut(&mut self, n: usize) -> &mut Word {
        self.registers.entry(n).or_insert_with(Vec::new)
    }

    /// Returns the number of steps executed.
    pub fn step_count(&self) -> usize {
        self.step_count
    }

    /// Returns all non-empty registers.
    pub fn registers(&self) -> &HashMap<usize, Word> {
        &self.registers
    }

    /// Runs the machine until it halts or stops improperly.
    pub fn run(&mut self) -> RunResult {
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

    /// Executes a single step, returning whether to continue.
    pub fn step(&mut self) -> StepResult {
        let n = self.program.len();

        // Check if we've halted (pc is exactly at n, one past the last instruction)
        if self.pc == n {
            return StepResult::Halted;
        }

        // Check if we've stopped improperly
        if self.pc > n {
            return StepResult::StoppedImproperly;
        }

        let instruction = self.program[self.pc].clone();
        self.step_count += 1;

        match instruction {
            Instruction::AddOne(reg) => {
                self.get_register_mut(reg).push(Symbol::One);
                self.pc += 1;
            }
            Instruction::AddHash(reg) => {
                self.get_register_mut(reg).push(Symbol::Hash);
                self.pc += 1;
            }
            Instruction::Forward(offset) => {
                self.pc += offset;
            }
            Instruction::Backward(offset) => {
                if offset > self.pc {
                    // Going before the start of the program
                    return StepResult::StoppedImproperly;
                }
                self.pc -= offset;
            }
            Instruction::Case(reg) => {
                let word = self.get_register_mut(reg);
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

    /// Prints the current state of the machine.
    pub fn print_state(&self) {
        println!("PC: {} / {}", self.pc + 1, self.program.len());
        if self.pc < self.program.len() {
            println!(
                "Next: {} ({})",
                self.program[self.pc].to_one_hash(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::string_to_word;

    #[test]
    fn test_add_one() {
        let instrs = vec![Instruction::AddOne(1)];
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("11#"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "11#1");
    }

    #[test]
    fn test_add_hash() {
        let instrs = vec![Instruction::AddHash(1)];
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("11"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "11#");
    }

    #[test]
    fn test_forward_halt() {
        let instrs = vec![Instruction::Forward(1)];
        let mut machine = Machine::new(instrs);
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
    }

    #[test]
    fn test_forward_improper() {
        let instrs = vec![Instruction::Forward(2)];
        let mut machine = Machine::new(instrs);
        let result = machine.run();
        assert!(matches!(result, RunResult::StoppedImproperly));
    }

    #[test]
    fn test_case_empty() {
        let instrs = vec![Instruction::Case(1), Instruction::AddOne(2)];
        let mut machine = Machine::new(instrs);
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(2)), "1");
    }

    #[test]
    fn test_case_one() {
        let instrs = vec![
            Instruction::Case(1),
            Instruction::AddOne(2),
            Instruction::AddHash(2),
        ];
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("1#"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "#");
        assert_eq!(word_to_string(machine.get_register(2)), "#");
    }

    #[test]
    fn test_case_hash() {
        let instrs = vec![
            Instruction::Case(1),
            Instruction::AddOne(2),
            Instruction::AddOne(3),
            Instruction::AddHash(2),
        ];
        let mut machine = Machine::new(instrs);
        machine.set_register(1, string_to_word("#1"));
        let result = machine.run();
        assert!(matches!(result, RunResult::Halted));
        assert_eq!(word_to_string(machine.get_register(1)), "1");
        assert_eq!(word_to_string(machine.get_register(2)), "#");
    }

    #[test]
    fn test_infinite_loop_detection() {
        let instrs = vec![Instruction::Forward(1), Instruction::Backward(1)];
        let mut machine = Machine::new(instrs).with_max_steps(100);
        let result = machine.run();
        assert!(matches!(result, RunResult::MaxStepsReached));
    }
}
