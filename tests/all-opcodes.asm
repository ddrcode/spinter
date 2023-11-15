* = $0200

;; READ / Absolute
LDA $0420
LDX $0421
LDY $0422

AND $0420
ORA $0421
EOR $0422

ADC $0420
SBC $0421


* = $0400
!byte $aa
!byte $08
!byte $ff

