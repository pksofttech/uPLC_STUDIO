(* ==================================================================== *)
(* IEC 61131-3 Instruction List (IL) Program                            *)
(* Target Hardware: Raspberry Pi Pico (RP2040)                          *)
(* UART RX Test Only: UART0 GP1 (RX) @ 115200 8N1                       *)
(* ==================================================================== *)

PROGRAM MainProgram
VAR
    (* --- Digital Outputs (%QX) --- *)
    LAMP_RX_PULSE AT %QX0.0 : BOOL; (* Pin: GP14 | Symbol: LAMP_RX_PULSE | 1-scan pulse on byte received *)
    LAMP_CHAR_A   AT %QX0.1 : BOOL; (* Pin: GP15 | Symbol: LAMP_CHAR_A   | ON when last RX byte == 'A' (65) *)
    LAMP_CHAR_B   AT %QX0.2 : BOOL; (* Pin: GP16 | Symbol: LAMP_CHAR_B   | ON when last RX byte == 'B' (66) *)

    (* --- Function Blocks & Internal Registers --- *)
    RX_BYTE AT %MW10 : INT := 0; (* Symbol: RX_BYTE | Last received UART byte (0..255) *)
    U_RX : UART_RECV;
END_VAR

(* Network 01: Receive 1 byte per scan from UART0 into %MW10 (LAMP_RX_PULSE pulses 1 scan on RX) *)
    LD   %SM0
    UART_RECV U_RX(OUT => %MW10)
    ST   %QX0.0

(* Network 02: Energize %QX0.1 when received byte == 65 ('A') *)
    LD   %SM0
    EQ   %MW10 65
    ST   %QX0.1

(* Network 03: Energize %QX0.2 when received byte == 66 ('B') *)
    LD   %SM0
    EQ   %MW10 66
    ST   %QX0.2

END_PROGRAM
