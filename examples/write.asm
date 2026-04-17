
.macro write src tmp
@loop:
  case src
  goto @moveback
  goto @one
  ;; hash
  add 1## tmp
  goto @loop

@one:
  add 1# tmp
  goto @loop
  
@moveback:
  move src tmp
.endmacro

write R1 R2
