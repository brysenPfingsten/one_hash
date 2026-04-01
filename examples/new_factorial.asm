.macro replace DST SRC
  clear DST
  move SRC DST
.endmacro

.macro mult A B DST
  multiply A B DST R5 R6 R7 R8 R9
.endmacro

add 1 R2

fact:
  is_nonzero R1 R9 R10 fact_nz fact_z

fact_nz:
  clear R3 
  copy R1 R3 R9
  mult R3 R2 R4
  replace R2 R4
  decrement R1 R9
  goto fact

fact_z:
  clear R3
  replace R1 R2
  halt
