# registers

## version string

- rv2a, rv3a, rv4: Core peripherals, by Qingke RISC-V cores
- v1: v103
- v2: v20x, not in v30x
- v3: v30x (might including v2, v1)
- v3_d8c: connectivity MCUs, PLLMUL is different
  - CH32V305*, CH32V307*
  - CH32F205RB, CH32F207VC
- x0: CH32X0

## Peripherals

### timer_common

ADTM

CTLR1.CAPOV and CTLR1.CAPLVL are not implemented in CH32V2, CH32V3, CH32V1.

SPEC only exists in CH32X0.

GPTM vs ADTM

- No RPTCR
- No BDTR
- No CCER.CCNE, CCER.CCNP
- No CTLR2.CCPC, .CCUS, .OIS, .OISN
- No DMAADR.COMIE, .BIE, .COMDE
- No INTFR.COMIF, .BIF,
- No SMCFGR.COMG, .BG

GPTM and GPTM_32(only in V208)

- ATRLR, CHCVR(x4) are 32 bits

BCTM vs GPTM

- No SMCFGR
- No CHCTLR_Input, CHCTLR_Output, CCER
- No CHCVR
- No CNT.DIR, .CMS, .CKD
- No CTLR2.CC*, .TI1S
- No DMAADDR, DMACFGR
