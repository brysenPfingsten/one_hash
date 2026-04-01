# one_hash

A 1# (one-hash) language interpreter and assembler.

## What is 1#?

1# is a minimal Text Register Machine with only two symbols: `1` (one) and `#` (hash). Despite this simplicity, it is Turing-complete.

### Data Encoding

Numbers are encoded in "backwards binary" where:
- LSB (least significant bit) comes first
- `1` = binary 1
- `#` = binary 0

| Decimal | Binary | Backwards Binary (1#) |
|---------|--------|----------------------|
| 0 | 0 | (empty) |
| 1 | 1 | `1` |
| 2 | 10 | `#1` |
| 3 | 11 | `11` |
| 4 | 100 | `##1` |
| 5 | 101 | `1#1` |
| 6 | 110 | `#11` |

### Instructions

The 1# language has exactly five instruction types. Each instruction follows the pattern `1^n` (n ones) followed by 1-5 `#` symbols:

| Instruction | Encoding | Effect |
|-------------|----------|--------|
| AddOne(n) | `1^n#` | Append '1' to register n |
| AddHash(n) | `1^n##` | Append '#' to register n |
| Forward(n) | `1^n###` | Jump forward n instructions |
| Backward(n) | `1^n####` | Jump backward n instructions |
| Case(n) | `1^n#####` | Branch on first symbol of register n |

**Case instruction behavior:**
- If register n is empty: go to the next instruction
- If register n starts with `1`: remove the `1`, skip 1 instruction
- If register n starts with `#`: remove the `#`, skip 2 instructions

## Installation

```bash
cargo build --release
```

The binary will be at `target/release/one_hash`.

## Usage

### Running 1# Programs (Interpreter)

```bash
# Run a program from a file
one_hash program.1h

# Run with initial register values
one_hash program.1h -r1 '1#1' -r2 '11'

# Run inline program
one_hash -e '1#11##'

# Verbose output (shows parsed instructions)
one_hash -v program.1h

# Set maximum execution steps (default: 1,000,000)
one_hash -m 500000 program.1h

# Start interactive REPL
one_hash --repl
```

### Assembling Programs (Assembler Mode)

Use the `--asm` or `-a` flag to run in assembler mode:

```bash
# Compile assembly to stdout
one_hash --asm program.asm

# Compile to a file
one_hash --asm program.asm -o output.1h

# Verbose compilation (shows expanded macros)
one_hash --asm -v program.asm

# Expand macros only (don't compile)
one_hash --asm -E program.asm

# Compile inline assembly
one_hash --asm -e 'add 1 R1'

# Start assembler REPL
one_hash --asm --repl
```

### Command Line Options

**Interpreter Mode:**

| Option | Description |
|--------|-------------|
| `-e <program>` | Execute program string directly |
| `-r<n> <word>` | Set register n to word (e.g., `-r1 11#`) |
| `-v, --verbose` | Verbose output |
| `-m, --max-steps <n>` | Maximum steps (default: 1000000) |
| `--repl` | Start interactive REPL |
| `--asm, -a` | Switch to assembler mode |
| `-h, --help` | Show help |

**Assembler Mode (`--asm`):**

| Option | Description |
|--------|-------------|
| `-o, --output <file>` | Output to file instead of stdout |
| `-v, --verbose` | Show compilation steps |
| `-e <code>` | Compile code from command line |
| `-E, --expand` | Expand macros only |
| `--repl` | Start interactive REPL |
| `-h, --help` | Show help |

## Assembly Language

The assembler provides a human-readable syntax for writing 1# programs.

### Basic Instructions

```asm
add 1 R1      ; Append 1 to register 1
add # R2      ; Append # to register 2
case R1       ; Branch on first symbol of R1
goto label    ; Jump to label
halt          ; Stop execution
label:        ; Define a label
; comment     ; Comment (to end of line)
```

### Built-in Macros

| Macro | Arguments | Effect |
|-------|-----------|--------|
| `clear` | `<reg>` | Set register to empty |
| `move` | `<src> <dst>` | Move src to dst, empty src |
| `copy` | `<src> <dst> <tmp>` | Copy src to dst (preserves src) |
| `swap` | `<a> <b> <tmp>` | Swap a and b |
| `pop` | `<reg>` | Remove first symbol |
| `shift_left` | `<reg> <tmp>` | Prepend # (multiply by 2) |
| `shift_right` | `<reg>` | Remove LSB (divide by 2) |
| `increment` | `<reg> <tmp>` | Add 1 (binary) |
| `decrement` | `<reg> <tmp>` | Subtract 1 (binary) |
| `is_nonzero` | `<r> <t1> <t2> <nz> <z>` | Branch on nonzero |
| `bin_add` | `<src> <dst> <t1> <t2> <c>` | Binary addition |
| `bin_sub` | `<src> <dst> <t1> <t2> <b>` | Binary subtraction |
| `multiply` | `<a> <b> <dst> <t1-t5>` | Binary multiplication |
| `compare` | `<a> <b> <t1-t3> <lt> <eq> <gt>` | Comparison branch |
| `divide` | `<dd> <dv> <q> <r> <t1-t4>` | Division with remainder |

### User-Defined Macros

```asm
.macro name param1 param2 ...
    ; macro body
    add 1 param1
    goto @local_label    ; @ creates unique labels per invocation
@local_label:
    ; ...
.endmacro

; Use the macro
name R1 R2
```

## Examples

### Simple Example: Add 1 and # to R1

```bash
one_hash -e '1#1##'
```

Output:
```
Result: Halted
Steps executed: 2

Final registers:
  R1: 1# = 1
```

### Computing Factorial

The `onehash_programs/factorial.asm` program computes n!:

```bash
# Compute 5! = 120
one_hash --asm onehash_programs/factorial.asm -o /tmp/fact.1h
one_hash /tmp/fact.1h -r1 '1#1'
```

Output:
```
Result: Halted
Steps executed: 2378

Final registers:
  R1: ###1111 = 120
```

Input register R1 contains n in backwards binary (`1#1` = 5), and the result (120) is left in R1.

### Interactive REPL

```bash
$ one_hash --repl
1# Interpreter REPL
Commands:
  :parse <program>  - Parse and show instructions
  :run <program>    - Run a program
  :set R<n> <word>  - Set register n to word
  :clear            - Clear all registers
  :regs             - Show all registers
  :load <file>      - Load program from file
  :help             - Show this help
  :quit             - Exit

1#> :set R1 111
R1 = 111

1#> 1#####1###1###
Result: Halted
Steps executed: 3

Final registers:
  R1: 11 = 3
```

## Project Structure

```
one_hash/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs           # Library entry point
│   ├── main.rs          # CLI entry point
│   ├── types.rs         # Symbol, Word types
│   ├── instruction.rs   # Instruction enum
│   ├── parser.rs        # 1# program parser
│   ├── machine.rs       # Virtual machine
│   └── assembler/
│       ├── mod.rs       # Assembler main module
│       └── macros.rs    # Macro expansion
└── onehash_programs/
    ├── factorial.asm    # Factorial in assembly
    ├── factorial.1h     # Compiled factorial
    └── example_macros.asm
```

## Library Usage

The project can also be used as a Rust library:

```rust
use one_hash::{parse_program, Machine, compile};

// Run a 1# program
let instructions = parse_program("1#11##").unwrap();
let mut machine = Machine::new(instructions);
machine.run();

// Compile assembly to 1#
let one_hash_code = compile("add 1 R1\nadd # R2").unwrap();
```

## License

MIT License - see LICENSE file for details.
