/* STM32F103C6T6: 32 KB Flash (32 x 1 KB pages) / 10 KB SRAM
 *
 * 0x0800_0000 .. 0x0800_73FF  (29 KB, pages 0..28)  Firmware code + rodata + .data init
 * 0x0800_7400 .. 0x0800_77FF  ( 1 KB, page 29)      RETAIN memory  ("PERS": M64..M95, D96..D127)
 * 0x0800_7800 .. 0x0800_7FFF  ( 2 KB, pages 30..31) PLC Bytecode   ("UPLC": 12 B header + 256 x 4 B)
 *
 * Keep in sync with FLASH_* constants in src/config.rs
 */
MEMORY
{
    FLASH       : ORIGIN = 0x08000000, LENGTH = 29K
    PLC_STORAGE : ORIGIN = 0x08007400, LENGTH = 3K
    RAM         : ORIGIN = 0x20000000, LENGTH = 10K
}
