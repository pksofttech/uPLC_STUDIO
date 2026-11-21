(* ==================================================================== *)
(* IEC 61131-3 Instruction List (IL) Program                            *)
(* Factory I/O Scene 2: From A to B (Set and Reset)                     *)
(* Modbus TCP Client -> Factory I/O Server (127.0.0.1:1502)             *)
(* ==================================================================== *)

PROGRAM MainProgram
VAR
    (* --- Digital Inputs (%IX) from Factory I/O Discrete Inputs --- *)
    SENSOR_A AT %IX0.0 : BOOL; (* Pin: DI0 | Symbol: SENSOR_A | Box detected at Point A *)
    SENSOR_B AT %IX0.1 : BOOL; (* Pin: DI1 | Symbol: SENSOR_B | Box arrived at Point B *)

    (* --- Digital Outputs (%QX) to Factory I/O Coils --- *)
    CONVEYOR AT %QX0.0 : BOOL; (* Pin: COIL0 | Symbol: CONVEYOR | Entry Conveyor Motor *)
END_VAR

(* Network 01: Set Conveyor ON when box is detected at Sensor A (%IX0.0) *)
    LD   %IX0.0
    S    %QX0.0

(* Network 02: Reset Conveyor OFF when box arrives at Sensor B (%IX0.1) *)
    LD   %IX0.1
    R    %QX0.0

END_PROGRAM
