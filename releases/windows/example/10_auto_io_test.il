(* ==================================================================== *)
(* IEC 61131-3 Instruction List (IL) Program                            *)
(* Target Hardware: Raspberry Pi Pico (RP2040)                          *)
(* Program: Automatic I/O Diagnostic & Sequential Chaser Test           *)
(* Author: uPLC Studio [PksofttecH]                                     *)
(* ==================================================================== *)
(*                                                                      *)
(* Hardware Mapping Summary:                                            *)
(* Digital Inputs (Active-Low with Pull-Up, connect to GND to trigger): *)
(*   %IX0.0 (X0 / GP10) : LAMP_TEST (Force all outputs Y0..Y5 ON)       *)
(*   %IX0.1 (X1 / GP11) : PAUSE_PB  (Pause/Freeze automatic chaser)     *)
(*   %IX0.2 (X2 / GP12) : TEST_GRP1 (Manual override for Y0, Y1, Y2)    *)
(*   %IX0.3 (X3 / GP13) : TEST_GRP2 (Manual override for Y3, Y4, Y5)    *)
(*                                                                      *)
(* Digital Outputs (3.3V GPIO Push-Pull):                               *)
(*   %QX0.0 (Y0 / GP15) : OUTPUT_Y0 (Stage 0: 0..500ms)                 *)
(*   %QX0.1 (Y1 / GP14) : OUTPUT_Y1 (Stage 1: 500..1000ms)              *)
(*   %QX0.2 (Y2 / GP6)  : OUTPUT_Y2 (Stage 2: 1000..1500ms)             *)
(*   %QX0.3 (Y3 / GP7)  : OUTPUT_Y3 (Stage 3: 1500..2000ms)             *)
(*   %QX0.4 (Y4 / GP2)  : OUTPUT_Y4 (Stage 4: 2000..2500ms)             *)
(*   %QX0.5 (Y5 / GP3)  : OUTPUT_Y5 (Stage 5: 2500..3000ms)             *)
(*                                                                      *)
(* Status LED: GP25 (Heartbeat 1Hz in RUN mode)                         *)
(* ==================================================================== *)

PROGRAM MainProgram
VAR
    (* --- Digital Inputs (%IX) --- *)
    LAMP_TEST AT %IX0.0 : BOOL; (* Pin: GP10 | Symbol: LAMP_TEST | Force All Outputs ON (Active-Low) *)
    PAUSE_PB AT %IX0.1 : BOOL; (* Pin: GP11 | Symbol: PAUSE_PB | Pause Auto Chaser (Active-Low) *)
    TEST_GRP1 AT %IX0.2 : BOOL; (* Pin: GP12 | Symbol: TEST_GRP1 | Manual Test Group 1 (Y0..Y2) *)
    TEST_GRP2 AT %IX0.3 : BOOL; (* Pin: GP13 | Symbol: TEST_GRP2 | Manual Test Group 2 (Y3..Y5) *)

    (* --- Digital Outputs (%QX) --- *)
    OUTPUT_Y0 AT %QX0.0 : BOOL; (* Pin: GP15 | Symbol: OUTPUT_Y0 | Digital Output 0 *)
    OUTPUT_Y1 AT %QX0.1 : BOOL; (* Pin: GP14 | Symbol: OUTPUT_Y1 | Digital Output 1 *)
    OUTPUT_Y2 AT %QX0.2 : BOOL; (* Pin: GP6  | Symbol: OUTPUT_Y2 | Digital Output 2 *)
    OUTPUT_Y3 AT %QX0.3 : BOOL; (* Pin: GP7  | Symbol: OUTPUT_Y3 | Digital Output 3 *)
    OUTPUT_Y4 AT %QX0.4 : BOOL; (* Pin: GP2  | Symbol: OUTPUT_Y4 | Digital Output 4 *)
    OUTPUT_Y5 AT %QX0.5 : BOOL; (* Pin: GP3  | Symbol: OUTPUT_Y5 | Digital Output 5 *)

    (* --- Internal Sequencing Relays & Timers --- *)
    M_STAGE1_DONE AT %MX0.0 : BOOL; (* Symbol: M_STAGE1_DONE | Stage 0 Complete (500ms) *)
    M_STAGE2_DONE AT %MX0.1 : BOOL; (* Symbol: M_STAGE2_DONE | Stage 1 Complete (1000ms) *)
    M_STAGE3_DONE AT %MX0.2 : BOOL; (* Symbol: M_STAGE3_DONE | Stage 2 Complete (1500ms) *)
    M_STAGE4_DONE AT %MX0.3 : BOOL; (* Symbol: M_STAGE4_DONE | Stage 3 Complete (2000ms) *)
    M_STAGE5_DONE AT %MX0.4 : BOOL; (* Symbol: M_STAGE5_DONE | Stage 4 Complete (2500ms) *)
    M_CYCLE_RESET AT %MX0.5 : BOOL; (* Symbol: M_CYCLE_RESET | Full Cycle Reset (3000ms) *)

    T0 : TON; (* Preset: 500ms | Symbol: TON_STAGE1 | Stage 0 Timer *)
    T1 : TON; (* Preset: 500ms | Symbol: TON_STAGE2 | Stage 1 Timer *)
    T2 : TON; (* Preset: 500ms | Symbol: TON_STAGE3 | Stage 2 Timer *)
    T3 : TON; (* Preset: 500ms | Symbol: TON_STAGE4 | Stage 3 Timer *)
    T4 : TON; (* Preset: 500ms | Symbol: TON_STAGE5 | Stage 4 Timer *)
    T5 : TON; (* Preset: 500ms | Symbol: TON_STAGE6 | Stage 5 Timer *)
END_VAR

(* Network 01: Sequence Stage 0 Timer (0..500ms) *)
    LDN  %IX0.1
    ANDN %MX0.5
    CAL  T0(IN := TRUE, PT := T#500ms)
    ST   %MX0.0

(* Network 02: Sequence Stage 1 Timer (500..1000ms) *)
    LD   %MX0.0
    CAL  T1(IN := TRUE, PT := T#500ms)
    ST   %MX0.1

(* Network 03: Sequence Stage 2 Timer (1000..1500ms) *)
    LD   %MX0.1
    CAL  T2(IN := TRUE, PT := T#500ms)
    ST   %MX0.2

(* Network 04: Sequence Stage 3 Timer (1500..2000ms) *)
    LD   %MX0.2
    CAL  T3(IN := TRUE, PT := T#500ms)
    ST   %MX0.3

(* Network 05: Sequence Stage 4 Timer (2000..2500ms) *)
    LD   %MX0.3
    CAL  T4(IN := TRUE, PT := T#500ms)
    ST   %MX0.4

(* Network 06: Sequence Stage 5 Timer & Cycle Reset (2500..3000ms) *)
    LD   %MX0.4
    CAL  T5(IN := TRUE, PT := T#500ms)
    ST   %MX0.5

(* Network 07: Output Y0 (GP15) - Active in Stage 0 or Manual Test *)
    LDN  %MX0.0
    OR   %IX0.0
    OR   %IX0.2
    ST   %QX0.0

(* Network 08: Output Y1 (GP14) - Active in Stage 1 or Manual Test *)
    LD   %MX0.0
    ANDN %MX0.1
    OR   %IX0.0
    OR   %IX0.2
    ST   %QX0.1

(* Network 09: Output Y2 (GP6) - Active in Stage 2 or Manual Test *)
    LD   %MX0.1
    ANDN %MX0.2
    OR   %IX0.0
    OR   %IX0.2
    ST   %QX0.2

(* Network 10: Output Y3 (GP7) - Active in Stage 3 or Manual Test *)
    LD   %MX0.2
    ANDN %MX0.3
    OR   %IX0.0
    OR   %IX0.3
    ST   %QX0.3

(* Network 11: Output Y4 (GP2) - Active in Stage 4 or Manual Test *)
    LD   %MX0.3
    ANDN %MX0.4
    OR   %IX0.0
    OR   %IX0.3
    ST   %QX0.4

(* Network 12: Output Y5 (GP3) - Active in Stage 5 or Manual Test *)
    LD   %MX0.4
    ANDN %MX0.5
    OR   %IX0.0
    OR   %IX0.3
    ST   %QX0.5

END_PROGRAM
