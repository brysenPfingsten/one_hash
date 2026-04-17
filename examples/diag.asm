.macro diag src tmp1 tmp2
@loop:
  case src
  goto @moveback
  goto @one
  ;; hash
  add 1## tmp1
  add # tmp2
  goto @loop

@one:
  add 1# tmp1
  add 1 tmp2
  goto @loop

@moveback:
  move tmp1 src
  move tmp2 src
.endmacro

diag R1 R2 R3
add # R2
