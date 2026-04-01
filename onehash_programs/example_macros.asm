; ============================================================================
; example_macros.asm - Demonstrates user-defined macros
; ============================================================================
;
; This file shows how to define and use custom macros in 1# assembly.
;
; Macro syntax:
;   .macro <name> <param1> <param2> ...
;       <body>
;   .endmacro
;
; Local labels:
;   Use @label for labels that should be unique per macro invocation.
;   Each call to the macro gets its own copy of these labels.
;
; ============================================================================

; ----------------------------------------------------------------------------
; double <reg> <tmp>
;   Effect: reg := reg * 2
;   Uses:   copy, bin_add (requires R5-R7, R9 as scratch)
; ----------------------------------------------------------------------------
.macro double reg tmp
    copy reg tmp R9
    bin_add tmp reg R5 R6 R7
.endmacro

; ----------------------------------------------------------------------------
; triple <reg> <tmp1> <tmp2>
;   Effect: reg := reg * 3
;   Uses:   copy, bin_add (requires R5-R7, R9 as scratch)
; ----------------------------------------------------------------------------
.macro triple reg tmp1 tmp2
    copy reg tmp1 R9
    copy reg tmp2 R9
    bin_add tmp1 reg R5 R6 R7
    bin_add tmp2 reg R5 R6 R7
.endmacro

; ============================================================================
; Main program
;   Computes: 3 * 2 * 3 = 18
; ============================================================================

    add 1 R1
    add 1 R1

    double R1 R2
    triple R1 R2 R3

    clear R2
    clear R3
    clear R4
    clear R5
    clear R6
    clear R7
    clear R9

    halt
