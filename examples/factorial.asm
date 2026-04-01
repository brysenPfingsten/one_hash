; ============================================================================
; factorial.asm - Computes n! in backwards binary
; ============================================================================
;
; Input:  R1 = n (backwards binary)
; Output: R1 = n! (backwards binary)
;
; Backwards binary encoding:
;   - LSB first: 1=1, #=0
;   - Examples: 5=1#1, 6=#11, 720=####1#11#1
;
; Register allocation:
;   R1  = input n, then result accumulator
;   R2  = multiplicand (shifted copy of result)
;   R3  = multiplier bits (consumed during multiply)
;   R4  = addend for bin_add
;   R5-R7 = scratch for bin_add macro
;   R8  = countdown copy of n
;   R9-R10 = scratch for other macros
;
; Algorithm:
;   counter = n
;   result = 1
;   while counter > 0:
;       result = result * counter
;       counter = counter - 1
;   return result
; ============================================================================

    move R1 R8
    add 1 R1

; ----------------------------------------------------------------------------
; fact_loop
;   Precondition:  R1 = partial result, R8 = remaining counter
;   Postcondition: R1 = n!, R8 = 0
; ----------------------------------------------------------------------------
fact_loop:
    is_nonzero R8 R9 R10 do_mult done

; ----------------------------------------------------------------------------
; do_mult
;   Multiplies R1 by R8 using shift-and-add.
;   Precondition:  R1 = accumulator, R8 = multiplier
;   Postcondition: R1 = R1 * R8, R8 unchanged
; ----------------------------------------------------------------------------
do_mult:
    clear R2
    copy R1 R2 R9
    clear R3
    copy R8 R3 R9
    clear R1

; ----------------------------------------------------------------------------
; mult_loop
;   Shift-and-add multiplication loop.
;   Precondition:  R1 = partial product, R2 = shifted multiplicand, R3 = remaining bits
;   Postcondition: R1 = final product, R2 = garbage, R3 = empty
; ----------------------------------------------------------------------------
mult_loop:
    case R3
    goto mult_done
    goto mult_add
    goto mult_shift

mult_add:
    copy R2 R4 R9
    bin_add R4 R1 R5 R6 R7
    goto mult_shift

mult_shift:
    shift_left R2 R9
    goto mult_loop

mult_done:
    decrement R8 R9
    goto fact_loop

; ----------------------------------------------------------------------------
; done
;   Cleanup: clears all registers except R1.
; ----------------------------------------------------------------------------
done:
    clear R2
    clear R3
    clear R4
    clear R5
    clear R6
    clear R7
    clear R8
    clear R9
    clear R10
    halt
