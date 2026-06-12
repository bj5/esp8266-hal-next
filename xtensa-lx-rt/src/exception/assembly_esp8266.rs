use core::arch::{global_asm, naked_asm};

// Global assembly to export symbols if needed by linker
global_asm!(
    "
    .set XT_STK_PC,              0
    .set XT_STK_PS,              4
    .set XT_STK_A0,              8
    .equ XT_STK_A1,             12
    .set XT_STK_A2,             16
    .set XT_STK_A3,             20
    .set XT_STK_A4,             24
    .set XT_STK_A5,             28
    .set XT_STK_A6,             32
    .set XT_STK_A7,             36
    .set XT_STK_A8,             40
    .set XT_STK_A9,             44
    .set XT_STK_A10,            48
    .set XT_STK_A11,            52
    .set XT_STK_A12,            56
    .set XT_STK_A13,            60
    .set XT_STK_A14,            64
    .set XT_STK_A15,            68
    .set XT_STK_SAR,            72
    .set XT_STK_EXCCAUSE,       76
    .set XT_STK_EXCVADDR,       80

    .set XT_STK_BASESAVE,      240
    .set XT_STK_FRMSZ,         256

    .set PS_INTLEVEL_EXCM, 3
    .set PS_INTLEVEL_MASK, 0x0000000f
    .set PS_EXCM,          0x00000010
    .set PS_UM,            0x00000020
    .set PS_WOE,           0x00040000
    "
);

extern "Rust" {
    fn __exception(cause: super::ExceptionCause);
    fn __double_exception(cause: super::ExceptionCause);
    fn __level_1_interrupt(level: u32);
    fn __kernel_exception();
    fn __nmi_exception();
    fn __debug_exception();
    fn __alloc_exception();
}

#[naked]
#[no_mangle]
#[link_section = ".rwtext"]
unsafe extern "C" fn save_context() {
    naked_asm!(
        "
        .set XT_STK_PC,              0
        .set XT_STK_PS,              4
        .set XT_STK_A0,              8
        .equ XT_STK_A1,             12
        .set XT_STK_A2,             16
        .set XT_STK_A3,             20
        .set XT_STK_A4,             24
        .set XT_STK_A5,             28
        .set XT_STK_A6,             32
        .set XT_STK_A7,             36
        .set XT_STK_A8,             40
        .set XT_STK_A9,             44
        .set XT_STK_A10,            48
        .set XT_STK_A11,            52
        .set XT_STK_A12,            56
        .set XT_STK_A13,            60
        .set XT_STK_A14,            64
        .set XT_STK_A15,            68
        .set XT_STK_SAR,            72
        .set XT_STK_EXCCAUSE,       76
        .set XT_STK_EXCVADDR,       80

        .set XT_STK_BASESAVE,      240
        .set XT_STK_FRMSZ,         256

        .set PS_INTLEVEL_EXCM, 3
        .set PS_INTLEVEL_MASK, 0x0000000f
        .set PS_EXCM,          0x00000010
        .set PS_UM,            0x00000020
        .set PS_WOE,           0x00040000

        s32i    a2,  sp, +XT_STK_A2
        s32i    a3,  sp, +XT_STK_A3
        s32i    a4,  sp, +XT_STK_A4
        s32i    a5,  sp, +XT_STK_A5
        s32i    a6,  sp, +XT_STK_A6
        s32i    a7,  sp, +XT_STK_A7
        s32i    a8,  sp, +XT_STK_A8
        s32i    a9,  sp, +XT_STK_A9
        s32i    a10, sp, +XT_STK_A10
        s32i    a11, sp, +XT_STK_A11
        s32i    a12, sp, +XT_STK_A12
        s32i    a13, sp, +XT_STK_A13
        s32i    a14, sp, +XT_STK_A14
        s32i    a15, sp, +XT_STK_A15

        rsr     a3,  SAR
        s32i    a3,  sp, +XT_STK_SAR

        ret
        "
    );
}

#[naked]
#[no_mangle]
#[link_section = ".rwtext"]
unsafe extern "C" fn restore_context() {
    naked_asm!(
        "
        l32i    a3,  sp, +XT_STK_SAR
        wsr     a3,  SAR

        l32i    a2,  sp, +XT_STK_A2
        l32i    a3,  sp, +XT_STK_A3
        l32i    a4,  sp, +XT_STK_A4
        l32i    a5,  sp, +XT_STK_A5
        l32i    a6,  sp, +XT_STK_A6
        l32i    a7,  sp, +XT_STK_A7
        l32i    a8,  sp, +XT_STK_A8
        l32i    a9,  sp, +XT_STK_A9
        l32i    a10, sp, +XT_STK_A10
        l32i    a11, sp, +XT_STK_A11
        l32i    a12, sp, +XT_STK_A12
        l32i    a13, sp, +XT_STK_A13
        l32i    a14, sp, +XT_STK_A14
        l32i    a15, sp, +XT_STK_A15

        ret
        "
    );
}

#[naked]
#[no_mangle]
#[link_section = ".rwtext"]
unsafe extern "C" fn __default_naked_exception() {
    naked_asm!(
        "
        .macro SAVE_CONTEXT level:req
        mov     a0, a1
        addmi   sp, sp, -XT_STK_FRMSZ
        s32i    a0, sp, +XT_STK_A1

        .ifc \\level,double
        rsr     a0, DEPC
        .else
        rsr     a0, EPC\\level
        .endif
        s32i    a0, sp, +XT_STK_PC

        .ifc \\level,double
        rsr     a0, EXCSAVE2
        .else
        rsr     a0, EXCSAVE\\level
        .endif
        s32i    a0, sp, +XT_STK_A0

        .ifc \\level,1
        rsr     a0, PS
        s32i    a0, sp, +XT_STK_PS
        rsr     a0, EXCCAUSE
        s32i    a0, sp, +XT_STK_EXCCAUSE
        rsr     a0, EXCVADDR
        s32i    a0, sp, +XT_STK_EXCVADDR
        .endif

        .ifc \\level,double
        rsr     a0, EXCCAUSE
        s32i    a0, sp, +XT_STK_EXCCAUSE
        rsr     a0, EXCVADDR
        s32i    a0, sp, +XT_STK_EXCVADDR
        .endif

        call0   save_context
        .endm

        .macro RESTORE_CONTEXT level:req
        call0   restore_context

        .ifc \\level,1
        l32i    a0, sp, +XT_STK_PS
        wsr     a0, PS
        l32i    a0, sp, +XT_STK_PC
        wsr     a0, EPC\\level
        .endif

        l32i    a0, sp, +XT_STK_A0
        l32i    sp, sp, +XT_STK_A1
        rsync
        .endm

        SAVE_CONTEXT 1

        rsr.EXCCAUSE a2
        beqi    a2, 4, .Level1Interrupt

        mov     a3, sp
        call0   __exception

        j .RestoreContext

        .Level1Interrupt:
        movi    a2, 1
        mov     a3, sp
        call0   __level_1_interrupt

        .RestoreContext:
        RESTORE_CONTEXT 1

        .byte 0x00, 0x30, 0x00
        "
    );
}

#[naked]
#[no_mangle]
#[link_section = ".rwtext"]
unsafe extern "C" fn __default_naked_double_exception() {
    naked_asm!(
        "
        SAVE_CONTEXT double

        l32i    a2, sp, +XT_STK_EXCCAUSE
        mov     a3, sp
        call0   __double_exception

        RESTORE_CONTEXT double

        .byte 0x00, 0x30, 0x00
        "
    );
}

#[naked]
#[no_mangle]
#[link_section = ".rwtext"]
unsafe extern "C" fn __default_naked_kernel_exception() {
    naked_asm!(
        "
        SAVE_CONTEXT 1

        l32i    a2, sp, +XT_STK_EXCCAUSE

        mov     a3, sp
        call0   __kernel_exception

        RESTORE_CONTEXT 1

        .byte 0x00, 0x30, 0x00
        "
    );
}

#[naked]
#[no_mangle]
#[link_section = ".rwtext"]
unsafe extern "C" fn __default_naked_nmi_exception() {
    naked_asm!(
        "
        SAVE_CONTEXT 1

        l32i    a2, sp, +XT_STK_EXCCAUSE

        mov     a3, sp
        call0   __nmi_exception

        RESTORE_CONTEXT 1

        .byte 0x00, 0x30, 0x00
        "
    );
}

#[naked]
#[no_mangle]
#[link_section = ".rwtext"]
unsafe extern "C" fn __default_naked_debug_exception() {
    naked_asm!(
        "
        SAVE_CONTEXT 1

        l32i    a2, sp, +XT_STK_EXCCAUSE

        mov     a3, sp
        call0   __debug_exception

        RESTORE_CONTEXT 1

        .byte 0x00, 0x30, 0x00
        "
    );
}

#[naked]
#[no_mangle]
#[link_section = ".rwtext"]
unsafe extern "C" fn __default_naked_alloc_exception() {
    naked_asm!(
        "
        SAVE_CONTEXT 1

        l32i    a2, sp, +XT_STK_EXCCAUSE

        mov     a3, sp
        call0   __alloc_exception

        RESTORE_CONTEXT 1

        .byte 0x00, 0x30, 0x00
        "
    );
}
