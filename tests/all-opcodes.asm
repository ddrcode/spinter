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

CMP $0420
CPX $0421
CPY $0422

BIT $0420


;; WRITE / Absolute
STA $0423
STX $0424
STY $0425


;; WRITE / ZeroPage
STA $23
STX $24
STY $25

;; READ / ZeroPage
LDA $23
LDX $24
LDY $25

* = $0400
!byte $aa
!byte $08
!byte $ff

