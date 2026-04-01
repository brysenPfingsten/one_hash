//! 1# instruction definitions.
//!
//! The 1# language has exactly five instruction types.

/// The five instruction types in 1#.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
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
    /// Converts the instruction to its 1# string representation.
    pub fn to_one_hash(&self) -> String {
        match self {
            Instruction::AddOne(n) => format!("{}#", "1".repeat(*n)),
            Instruction::AddHash(n) => format!("{}##", "1".repeat(*n)),
            Instruction::Forward(n) => format!("{}###", "1".repeat(*n)),
            Instruction::Backward(n) => format!("{}####", "1".repeat(*n)),
            Instruction::Case(n) => format!("{}#####", "1".repeat(*n)),
        }
    }

    /// Returns a human-readable description of the instruction.
    pub fn describe(&self) -> String {
        match self {
            Instruction::AddOne(n) => format!("Add 1 to R{}", n),
            Instruction::AddHash(n) => format!("Add # to R{}", n),
            Instruction::Forward(n) => format!("Go forward {}", n),
            Instruction::Backward(n) => format!("Go backward {}", n),
            Instruction::Case(n) => format!("Cases on R{}", n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_one_hash() {
        assert_eq!(Instruction::AddOne(1).to_one_hash(), "1#");
        assert_eq!(Instruction::AddOne(3).to_one_hash(), "111#");
        assert_eq!(Instruction::AddHash(2).to_one_hash(), "11##");
        assert_eq!(Instruction::Forward(4).to_one_hash(), "1111###");
        assert_eq!(Instruction::Backward(2).to_one_hash(), "11####");
        assert_eq!(Instruction::Case(5).to_one_hash(), "11111#####");
    }

    #[test]
    fn test_describe() {
        assert_eq!(Instruction::AddOne(1).describe(), "Add 1 to R1");
        assert_eq!(Instruction::Case(2).describe(), "Cases on R2");
    }
}
