(* ==================================================================== *)
(* IEC 61131-3 Instruction List (IL) Program                            *)
(* Target Hardware: Raspberry Pi Pico (RP2040)                          *)
(* Transceiver: MAX485 / SP3485 (RS-485 Half-Duplex)                   *)
(* RS-485 Pins: GP20 (TX), GP21 (RX), GP22 (Direction DE/RE)            *)
(* Purpose: RS-485 Send Test (Modbus RTU FC06 Write Single Register)    *)
(* ==================================================================== *)

PROGRAM MainProgram
VAR
    (* --- Digital Inputs (%IX) --- *)
    AUTO_SEND_SW    AT %IX0.0 : BOOL; (* Pin: GP2 | Symbol: AUTO_SEND_SW    | Enable 1s Cyclic RS-485 Send (NO) *)
    MANUAL_SEND_BTN AT %IX0.1 : BOOL; (* Pin: GP3 | Symbol: MANUAL_SEND_BTN | Manual Single-Shot Send Trigger (NO) *)

    (* --- Digital Outputs (%QX) --- *)
    TX_ACTIVE_LED   AT %QX0.0 : BOOL; (* Pin: GP25 | Symbol: TX_ACTIVE_LED  | Onboard LED - High while sending RS-485 *)

    (* --- Internal Sequencer Relays (%MX) --- *)
    M_SEND_PULSE    AT %MX0.0 : BOOL; (* 1s Cyclic Timer Pulse *)
    M_TX_ACTIVE     AT %MX0.1 : BOOL; (* RS-485 Transmit Active Flag *)

    (* --- Test Data Register (%MW10 / D10) --- *)
    TX_TEST_VAL     AT %MW10 : INT := 0; (* Symbol: TX_TEST_VAL | Incrementing Test Value sent over RS-485 *)

    (* --- Function Blocks --- *)
    T_SEND : TON; (* Preset: 1000ms | 1-Second Cyclic Send Interval *)
    MB_TX  : MB_MASTER; (* Slave: 1, Func: 6, Addr: 0, Count: 1, Buffer: D10 | Write Test Value via RS-485 *)
END_VAR

(* Network 01: 1000ms Cyclic Timer Pulse when AUTO_SEND_SW is ON *)
    LD   %IX0.0
    ANDN %MX0.0
    ANDN %MX0.1
    CAL  T_SEND(IN := TRUE, PT := T#1000ms)
    ST   %MX0.0

(* Network 02: Trigger RS-485 Send on Timer Pulse or Manual Button (with Busy Interlock) *)
    LD   %MX0.0
    OR   %IX0.1
    ANDN %MX0.1
    S    %MX0.1

(* Network 03: Increment Test Counter (D10) and Send via RS-485 (Slave 1, FC 06, Addr 0) *)
    LD   %MX0.1
    ADD  %MW10 1
    CAL  MB_TX(SLAVE := 1, FUNC := 6, ADDR := 0, COUNT := 1, BUFFER := D10)
    R    %MX0.1

(* Network 04: Status Indication - Turn Onboard LED ON while RS-485 TX is active *)
    LD   %MX0.1
    ST   %QX0.0

END_PROGRAM
