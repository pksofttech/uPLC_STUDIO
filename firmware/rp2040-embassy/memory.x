MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    /* 2040 KB Main Program Flash */
    FLASH : ORIGIN = 0x10000100, LENGTH = 2040K - 0x100
    /* Last 8 KB Flash reserved for Persistent PLC Bytecode Storage */
    PLC_STORAGE : ORIGIN = 0x101FE000, LENGTH = 8K
    RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}

EXTERN(BOOT2_FIRMWARE)

SECTIONS {
    /* ### Boot loader */
    .boot2 ORIGIN(BOOT2) :
    {
        KEEP(*(.boot2));
    } > BOOT2
} INSERT BEFORE .text;
