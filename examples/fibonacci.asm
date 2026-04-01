; Fibonacci: computes fib(n) where n is in R1 (backwards binary)
; Result is left in R1, all other registers cleared
;
; Algorithm (iterative):
;   if n == 0: return 0
;   if n == 1: return 1
;   a = 0, b = 1
;   repeat n-1 times:
;       temp = a + b
;       a = b
;       b = temp
;   return b
;
; Register usage:
;   R1: input n, then loop counter (n-1), then output
;   R2: a = fib(k-1)
;   R3: b = fib(k)
;   R4: temp for a+b computation
;   R5: temp copy for bin_add
;   R6-R8: temps for bin_add
;   R9: temp for is_nonzero

; PRE:  R1 = n (input), all other registers empty
; POST: branches to check_one (n > 0) or is_zero (n == 0)
is_nonzero R1 R8 R9 check_one is_zero

is_zero:
  ; PRE:  R1 = 0 (empty or #s only)
  ; POST: R1 = 0 (empty), program halts
  clear R1
  halt

check_one:
  ; PRE:  R1 = n, where n >= 1
  ; POST: R1 = n-1, branches to init_loop (n >= 2) or is_one (n == 1)
  decrement R1 R8
  is_nonzero R1 R8 R9 init_loop is_one

is_one:
  ; PRE:  R1 = 0 (was n=1, now decremented)
  ; POST: R1 = 1, program halts
  clear R1
  add 1 R1
  halt

init_loop:
  ; PRE:  R1 = n-1 (where n >= 2), R2 = empty, R3 = empty
  ; POST: R1 = n-1, R2 = 0 (empty), R3 = 1
  ;       Falls through to fib_loop
  add 1 R3

fib_loop:
  ; PRE:  R1 = loop counter (k iterations remaining, k >= 1)
  ;       R2 = fib(n-1-k) = a
  ;       R3 = fib(n-k) = b
  ;       (On first iteration with n=5: R1=4, R2=0, R3=1)
  ; POST: R1 = k-1
  ;       R2 = old b = fib(n-k)
  ;       R3 = a+b = fib(n-k+1)
  ;       Branches to fib_loop (k > 1) or fib_done (k == 1)

  ; Compute temp = a + b in R4
  clear R4
  copy R2 R4 R8           ; R4 = a
  clear R5
  copy R3 R5 R8           ; R5 = b
  bin_add R5 R4 R6 R7 R8  ; R4 += R5, so R4 = a + b

  ; a = b
  clear R2
  move R3 R2              ; R2 = b, R3 = empty

  ; b = temp
  move R4 R3              ; R3 = a + b

  ; Decrement counter and loop
  decrement R1 R8
  is_nonzero R1 R8 R9 fib_loop fib_done

fib_done:
  ; PRE:  R1 = 0 (counter exhausted)
  ;       R2 = fib(n-1)
  ;       R3 = fib(n) (the answer)
  ; POST: R1 = fib(n), all other registers empty, program halts

  clear R1
  move R3 R1

  ; Clear all other registers
  clear R2
  clear R4
  clear R5
  clear R6
  clear R7
  clear R8
  clear R9
  halt
