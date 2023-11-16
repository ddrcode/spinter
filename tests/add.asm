lda #$fa
sta $0a
lda #$a3
sta $0b

clc
lda #$a8
adc $0b
tax
adc $0b
