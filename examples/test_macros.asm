; Test divide: 10 / 3 = 3 remainder 1
    add # R1    ; R1 = 0
    add 1 R1    ; R1 = 01
    add # R1    ; R1 = 010
    add 1 R1    ; R1 = 0101 = 10 (dividend)

    add 1 R2    ; R2 = 1
    add 1 R2    ; R2 = 11 = 3 (divisor)

    divide R1 R2 R3 R4 R5 R6 R7 R8
    ; R3 should be quotient = 3
    ; R4 should be remainder = 1
    halt
