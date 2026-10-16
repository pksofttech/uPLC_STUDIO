use my_canvas_core::{
    ContactMode, Document, IlParser, InsertPosition, LadderElement, LadderProgram, LdParser, SimState, TypeTag,
};

#[test]
fn test_demo_program_logic_latch() {
    let mut doc = Document::default();
    doc.simulator.state = SimState::Running;

    // Initially: Motor Y0 should be false
    doc.step_simulation(10);
    assert_eq!(doc.simulator.get_var_value("Y0"), false);

    // Press Start button X0 (momentary)
    doc.simulator.set_input("X0", true);
    doc.step_simulation(10);
    // Motor Y0 should now be ENERGIZED (true)
    assert_eq!(doc.simulator.get_var_value("Y0"), true);
    assert_eq!(doc.simulator.get_var_value("Y1"), true); // Running lamp follows Y0

    // Release Start button X0
    doc.simulator.set_input("X0", false);
    doc.step_simulation(10);
    // Motor Y0 must remain LATCHED ON via parallel contact!
    assert_eq!(doc.simulator.get_var_value("Y0"), true);

    // Press Stop button X1 (NC opening breaks circuit)
    doc.simulator.set_input("X1", true);
    doc.step_simulation(10);
    // Motor Y0 must drop out (turn OFF)
    assert_eq!(doc.simulator.get_var_value("Y0"), false);

    // Release Stop button
    doc.simulator.set_input("X1", false);
    doc.step_simulation(10);
    // Motor should stay OFF until Start is pressed again
    assert_eq!(doc.simulator.get_var_value("Y0"), false);
}

#[test]
fn test_microplc_ld_parser() {
    let sample_ld = r#"
# LD FORMAT copy Form LDMICRO#
HARDWARE
Arduino UNO R3
IO LIST
X_BTN_ON at A5
Y_LED_WASH at D6
END
PROGRAM
RUNG
	CONTACTS X_BTN_ON 0 0 0
	TON T01 500
	CTC C_POWER 1
END
RUNG
	CONTACTS C_POWER 0 1 0 1 0
	COIL R_POWER 0 0 0 0
END
RUNG
	COMMENT OUTPUT STAGE
END
RUNG
	CONTACTS R_POWER 0 0 0
	COIL Y_LED_WASH 0 0 0 0
END
"#;

    let prog = LdParser::parse(sample_ld).expect("Failed to parse MicroPLC .ld");
    assert_eq!(prog.hardware, "Arduino UNO R3");
    assert_eq!(prog.rungs.len(), 4);
    assert_eq!(prog.io_mappings.len(), 2);
    assert_eq!(prog.io_mappings[0].symbol, "X_BTN_ON");
    assert_eq!(prog.io_mappings[0].hw_pin, "A5");
}

#[test]
fn test_undo_redo_stack() {
    let mut doc = Document::default();
    let initial_rungs = doc.program.rungs.len();
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());

    // Add a new rung
    doc.add_rung();
    assert_eq!(doc.program.rungs.len(), initial_rungs + 1);
    assert!(doc.can_undo());

    // Undo adding the rung
    assert!(doc.undo());
    assert_eq!(doc.program.rungs.len(), initial_rungs);
    assert!(doc.can_redo());

    // Redo adding the rung
    assert!(doc.redo());
    assert_eq!(doc.program.rungs.len(), initial_rungs + 1);
}

#[test]
fn test_cross_rung_move_element() {
    let mut doc = Document::default();
    doc.selected_rung_idx = Some(0);
    let new_id = doc.program.alloc_id();
    doc.insert_element(
        LadderElement::new_contact(new_id, "X2", "SENSOR_A", ContactMode::NormallyOpen),
        InsertPosition::AppendRung,
    );

    // Move element `new_id` from Rung 0 to Rung 1 (cross-rung Drag & Drop)
    let moved = doc.move_element(new_id, 1, None, InsertPosition::AppendRung);
    assert!(moved);
    assert_eq!(doc.selected_rung_idx, Some(1));
    assert_eq!(doc.selected_element_id, Some(new_id));
    assert!(doc.program.rungs[1].elements.iter().any(|e| e.id == new_id));
    assert!(!doc.program.rungs[0].elements.iter().any(|e| e.id == new_id));

    // Undo the cross-rung move and verify it returns to Rung 0
    assert!(doc.undo());
    assert!(doc.program.rungs[0].elements.iter().any(|e| e.id == new_id));
    assert!(!doc.program.rungs[1].elements.iter().any(|e| e.id == new_id));
}


#[test]
fn test_box_selection_and_deselect() {
    let mut doc = Document::default();
    let id_a = doc.program.rungs[1].elements[0].id;
    let id_b = doc.program.rungs[1].elements[1].id;

    // Marquee box selection of 2 elements
    doc.set_box_selection(&[(1, id_a), (1, id_b)], false);
    assert_eq!(doc.selected_element_ids.len(), 2);
    assert!(doc.is_element_selected(id_a));
    assert!(doc.is_element_selected(id_b));

    // Clicking empty space clears selection
    doc.clear_element_selection();
    assert!(doc.selected_element_id.is_none());
    assert!(doc.selected_element_ids.is_empty());

    // Re-select and batch delete
    doc.set_box_selection(&[(1, id_a), (1, id_b)], false);
    doc.delete_selected_element();
    assert!(doc.program.rungs[1].elements.is_empty());
    assert!(doc.selected_element_ids.is_empty());

    // Single Undo restores both deleted elements
    assert!(doc.undo());
    assert_eq!(doc.program.rungs[1].elements.len(), 2);
}

#[test]
fn test_drag_drop_toolbar_elements() {
    let mut doc = Document::default();
    doc.selected_rung_idx = Some(0);

    // 1. Drop a new contact via Left of existing contact
    let existing_id = doc.program.rungs[0].elements[0].id;
    doc.selected_element_id = Some(existing_id);
    let new_contact_id = doc.program.alloc_id();
    doc.insert_element(
        LadderElement::new_contact(new_contact_id, "X10", "SENSOR_X", ContactMode::NormallyOpen),
        InsertPosition::Left,
    );
    assert_eq!(doc.program.rungs[0].elements[0].id, new_contact_id);

    // 2. Drop a parallel element below the new contact
    doc.selected_element_id = Some(new_contact_id);
    let par_id = doc.program.alloc_id();
    doc.insert_element(
        LadderElement::new_contact(par_id, "X11", "BYPASS", ContactMode::NormallyOpen),
        InsertPosition::BottomParallel,
    );
    assert_eq!(doc.program.rungs[0].elements[0].element_type, my_canvas_core::TypeTag::Branch);
    assert_eq!(doc.program.rungs[0].elements[0].branch_paths.len(), 2);

    // 3. Drop an element appending to Rung
    let append_id = doc.program.alloc_id();
    doc.selected_rung_idx = Some(0);
    doc.insert_element(
        LadderElement::new_coil(append_id, "Y10", "LAMP", my_canvas_core::CoilMode::Normal),
        InsertPosition::AppendRung,
    );
    assert!(doc.program.rungs[0].elements.iter().any(|e| e.id == append_id));

    // 4. Undo and Redo operations
    assert!(doc.undo());
    assert!(!doc.program.rungs[0].elements.iter().any(|e| e.id == append_id));
    assert!(doc.redo());
    assert!(doc.program.rungs[0].elements.iter().any(|e| e.id == append_id));
}

#[test]
fn test_contact_styles_and_serialization() {
    use my_canvas_core::{ContactStyle, LadderElement};

    // 1. Verify all 5 styles exist and have descriptions
    assert_eq!(ContactStyle::ALL.len(), 5);
    for st in ContactStyle::ALL {
        assert!(!st.label().is_empty());
        assert!(!st.short_name().is_empty());
        assert!(!st.description().is_empty());
    }

    // 2. Verify Document default
    let doc = Document::default();
    assert_eq!(doc.contact_style, ContactStyle::IecStandard);

    // 3. Verify per-element override and inheritance
    let mut elem = LadderElement::new_contact(1, "X001", "START", my_canvas_core::ContactMode::NormallyOpen);
    assert_eq!(elem.contact_style, None); // Inherit by default

    elem.contact_style = Some(ContactStyle::PushButton);
    assert_eq!(elem.contact_style, Some(ContactStyle::PushButton));

    // 4. Test JSON serialization and deserialization
    let json_str = serde_json::to_string(&elem).expect("serialize element");
    assert!(json_str.contains("PushButton"));

    let deserialized: LadderElement = serde_json::from_str(&json_str).expect("deserialize element");
    assert_eq!(deserialized.contact_style, Some(ContactStyle::PushButton));

    // 5. Backward compatibility: JSON missing contact_style field defaults to None
    let mut default_elem = LadderElement::new_contact(99, "X002", "START", my_canvas_core::ContactMode::NormallyOpen);
    default_elem.contact_style = Some(ContactStyle::DinEuropean);
    let mut json_val: serde_json::Value = serde_json::to_value(&default_elem).unwrap();
    json_val.as_object_mut().unwrap().remove("contact_style");
    let legacy_elem: LadderElement = serde_json::from_value(json_val).expect("deserialize legacy element");
    assert_eq!(legacy_elem.contact_style, None);
}

#[test]
fn test_coil_styles_and_serialization() {
    use my_canvas_core::{CoilMode, CoilStyle, LadderElement};

    // 1. Verify all 5 styles exist and have descriptions
    assert_eq!(CoilStyle::ALL.len(), 5);
    for st in CoilStyle::ALL {
        assert!(!st.label().is_empty());
        assert!(!st.short_name().is_empty());
        assert!(!st.description().is_empty());
    }

    // 2. Verify Document default
    let mut doc = Document::default();
    assert_eq!(doc.coil_style, CoilStyle::IecParentheses);

    doc.coil_style = CoilStyle::CircleRelay;
    assert_eq!(doc.coil_style, CoilStyle::CircleRelay);

    // 3. Verify per-element override and inheritance
    let mut elem = LadderElement::new_coil(1, "Y001", "PUMP_VALVE", CoilMode::Normal);
    assert_eq!(elem.coil_style, None); // Inherit global theme by default

    elem.coil_style = Some(CoilStyle::SolenoidValve);
    assert_eq!(elem.coil_style, Some(CoilStyle::SolenoidValve));

    // 4. Test JSON serialization and deserialization of element
    let json_str = serde_json::to_string(&elem).expect("serialize coil element");
    assert!(json_str.contains("SolenoidValve"));

    let deserialized: LadderElement = serde_json::from_str(&json_str).expect("deserialize coil element");
    assert_eq!(deserialized.coil_style, Some(CoilStyle::SolenoidValve));

    // 5. Test JSON serialization and deserialization of Document
    let doc_json = serde_json::to_string(&doc).expect("serialize document");
    assert!(doc_json.contains("CircleRelay"));
    let deserialized_doc: Document = serde_json::from_str(&doc_json).expect("deserialize document");
    assert_eq!(deserialized_doc.coil_style, CoilStyle::CircleRelay);

    // 6. Backward compatibility: JSON missing coil_style field defaults to None for element and IecParentheses for doc
    let mut default_elem = LadderElement::new_coil(99, "Y002", "LIGHT", CoilMode::Normal);
    default_elem.coil_style = Some(CoilStyle::PilotLight);
    let mut json_val: serde_json::Value = serde_json::to_value(&default_elem).unwrap();
    json_val.as_object_mut().unwrap().remove("coil_style");
    let legacy_elem: LadderElement = serde_json::from_value(json_val).expect("deserialize legacy coil element");
    assert_eq!(legacy_elem.coil_style, None);

    let mut doc_val: serde_json::Value = serde_json::to_value(&doc).unwrap();
    doc_val.as_object_mut().unwrap().remove("coil_style");
    let legacy_doc: Document = serde_json::from_value(doc_val).expect("deserialize legacy doc");
    assert_eq!(legacy_doc.coil_style, CoilStyle::IecParentheses);
}

#[test]
fn test_unified_schematic_style() {
    use my_canvas_core::{CoilStyle, ContactStyle, Document, SchematicStyle};

    let mut doc = Document::default();
    assert_eq!(doc.schematic_style, SchematicStyle::IecStandard);
    assert_eq!(doc.contact_style, ContactStyle::IecStandard);
    assert_eq!(doc.coil_style, CoilStyle::IecParentheses);

    // Change to JIC / NEMA
    doc.set_schematic_style(SchematicStyle::JicNema);
    assert_eq!(doc.schematic_style, SchematicStyle::JicNema);
    assert_eq!(doc.contact_style, ContactStyle::JicSchematic);
    assert_eq!(doc.coil_style, CoilStyle::NemaBox);

    // Change to DIN European / JIS
    doc.set_schematic_style(SchematicStyle::DinEuropean);
    assert_eq!(doc.schematic_style, SchematicStyle::DinEuropean);
    assert_eq!(doc.contact_style, ContactStyle::DinEuropean);
    assert_eq!(doc.coil_style, CoilStyle::CircleRelay);

    // Change to Panel Devices (Push Button & Pilot Lamp)
    doc.set_schematic_style(SchematicStyle::PushBtnPilot);
    assert_eq!(doc.schematic_style, SchematicStyle::PushBtnPilot);
    assert_eq!(doc.contact_style, ContactStyle::PushButton);
    assert_eq!(doc.coil_style, CoilStyle::PilotLight);

    // Change to Toggle & Solenoid
    doc.set_schematic_style(SchematicStyle::ToggleSolenoid);
    assert_eq!(doc.schematic_style, SchematicStyle::ToggleSolenoid);
    assert_eq!(doc.contact_style, ContactStyle::ToggleSwitch);
    assert_eq!(doc.coil_style, CoilStyle::SolenoidValve);

    // Test JSON Serialization
    let json_str = serde_json::to_string(&doc).expect("serialize doc");
    assert!(json_str.contains("ToggleSolenoid"));
    let deserialized: Document = serde_json::from_str(&json_str).expect("deserialize doc");
    assert_eq!(deserialized.schematic_style, SchematicStyle::ToggleSolenoid);
}

#[test]
fn test_group_selection_parallel_and_series_insertion() {
    use my_canvas_core::{ContactMode, Document, LadderElement, TypeTag};

    let mut doc = Document::new_empty();

    // 1. Set up a rung with 3 contacts: [X00, X01, X02]
    let id1 = 1;
    let id2 = 2;
    let id3 = 3;
    let elem1 = LadderElement::new_contact(id1, "X00", "PB1", ContactMode::NormallyOpen);
    let elem2 = LadderElement::new_contact(id2, "X01", "PB2", ContactMode::NormallyOpen);
    let elem3 = LadderElement::new_contact(id3, "X02", "PB3", ContactMode::NormallyOpen);

    doc.insert_element(elem1, my_canvas_core::InsertPosition::AppendRung);
    doc.insert_element(elem2, my_canvas_core::InsertPosition::AppendRung);
    doc.insert_element(elem3, my_canvas_core::InsertPosition::AppendRung);

    assert_eq!(doc.program.rungs[0].elements.len(), 3);

    // 2. Select a group of elements: [X00, X01] (IDs: 1, 2)
    doc.selected_element_ids = vec![id1, id2];
    doc.selected_element_id = Some(id1);

    // 3. Insert in Parallel (วงจรขนาน) across the selected group [X00, X01] with new contact X_PAR
    let id_par = 4;
    let elem_par = LadderElement::new_contact(id_par, "X_PAR", "PAR", ContactMode::NormallyOpen);
    doc.insert_parallel_to_selection(elem_par, false);

    // Verify rung structure:
    // Elements on rung should now be: [Branch(path0: [X00, X01], path1: [X_PAR]), X02]
    assert_eq!(doc.program.rungs[0].elements.len(), 2);
    let branch = &doc.program.rungs[0].elements[0];
    assert_eq!(branch.element_type, TypeTag::Branch);
    assert_eq!(branch.branch_paths.len(), 2);

    // Path 0 has the selected group: [X00, X01]
    assert_eq!(branch.branch_paths[0].len(), 2);
    assert_eq!(branch.branch_paths[0][0].name, "X00");
    assert_eq!(branch.branch_paths[0][1].name, "X01");

    // Path 1 has the newly inserted parallel element: [X_PAR]
    assert_eq!(branch.branch_paths[1].len(), 1);
    assert_eq!(branch.branch_paths[1][0].name, "X_PAR");

    // Element after the branch is still X02
    assert_eq!(doc.program.rungs[0].elements[1].name, "X02");

    // 4. Test Undo: Reverts back to 3 series elements [X00, X01, X02]
    assert!(doc.undo());
    assert_eq!(doc.program.rungs[0].elements.len(), 3);
    assert_eq!(doc.program.rungs[0].elements[0].name, "X00");
    assert_eq!(doc.program.rungs[0].elements[1].name, "X01");
    assert_eq!(doc.program.rungs[0].elements[2].name, "X02");

    // 5. Test Redo: Restores the parallel branch circuit
    assert!(doc.redo());
    assert_eq!(doc.program.rungs[0].elements.len(), 2);
    assert_eq!(doc.program.rungs[0].elements[0].element_type, TypeTag::Branch);

    // 6. Test Series Insertion (วงจรอนุกรม):
    // Select the branch and X02 as a group
    let branch_id = doc.program.rungs[0].elements[0].id;
    doc.selected_element_ids = vec![branch_id, id3];
    doc.selected_element_id = Some(id3);

    // Insert in Series Right (ต่ออนุกรม ถัดไป)
    let id_ser = 5;
    let elem_ser = LadderElement::new_contact(id_ser, "X_SER", "SER", ContactMode::NormallyOpen);
    doc.insert_series_to_selection(elem_ser, true);

    // Verify elements are [Branch, X02, X_SER]
    assert_eq!(doc.program.rungs[0].elements.len(), 3);
    assert_eq!(doc.program.rungs[0].elements[0].element_type, TypeTag::Branch);
    assert_eq!(doc.program.rungs[0].elements[1].name, "X02");
    assert_eq!(doc.program.rungs[0].elements[2].name, "X_SER");

    // Insert in Series Left (ต่ออนุกรม ก่อนหน้า)
    doc.selected_element_ids = vec![branch_id];
    let id_front = 6;
    let elem_front = LadderElement::new_contact(id_front, "X_FRONT", "FRONT", ContactMode::NormallyOpen);
    doc.insert_series_to_selection(elem_front, false);

    // Verify elements are [X_FRONT, Branch, X02, X_SER]
    assert_eq!(doc.program.rungs[0].elements.len(), 4);
    assert_eq!(doc.program.rungs[0].elements[0].name, "X_FRONT");
    assert_eq!(doc.program.rungs[0].elements[1].element_type, TypeTag::Branch);
    assert_eq!(doc.program.rungs[0].elements[2].name, "X02");
    assert_eq!(doc.program.rungs[0].elements[3].name, "X_SER");
}

#[test]
fn test_plc_standard_ladder_elements_and_conduction() {
    use my_canvas_core::{
        CoilMode, ContactMode, Document, InsertPosition, LadderElement, PlcSimulator, TypeTag,
    };

    // 1. Verify right-aligned output element rules
    assert!(TypeTag::Coil.is_right_aligned());
    assert!(TypeTag::Ctu.is_right_aligned());
    assert!(TypeTag::Ctd.is_right_aligned());
    assert!(TypeTag::Ctc.is_right_aligned());
    assert!(TypeTag::Move.is_right_aligned());
    assert!(TypeTag::Math.is_right_aligned());
    assert!(!TypeTag::Contact.is_right_aligned());
    assert!(!TypeTag::Branch.is_right_aligned());

    // 2. Build a standard PLC motor latching circuit:
    // Rung 0: [ Branch([X00 (START)], [Y00 (SEAL_IN)]), X01 (STOP_NC), Y00 (MOTOR_COIL) ]
    let mut doc = Document::new_empty();

    let start_contact = LadderElement::new_contact(1, "X00", "START_PB", ContactMode::NormallyOpen);
    let seal_contact = LadderElement::new_contact(2, "Y00", "SEAL_IN", ContactMode::NormallyOpen);
    let stop_contact = LadderElement::new_contact(3, "X01", "STOP_PB", ContactMode::NormallyClosed);
    let motor_coil = LadderElement::new_coil(4, "Y00", "MOTOR", CoilMode::Normal);

    // Create parallel branch with START and SEAL_IN
    let branch = LadderElement {
        id: 10,
        element_type: TypeTag::Branch,
        name: "Branch".to_string(),
        symbol: "OR".to_string(),
        branch_paths: vec![vec![start_contact], vec![seal_contact]],
        ..Default::default()
    };

    doc.insert_element(branch, InsertPosition::AppendRung);
    doc.insert_element(stop_contact, InsertPosition::AppendRung);
    doc.insert_element(motor_coil, InsertPosition::AppendRung);

    assert_eq!(doc.program.rungs[0].elements.len(), 3);
    assert_eq!(doc.program.rungs[0].elements[0].element_type, TypeTag::Branch);
    assert_eq!(doc.program.rungs[0].elements[1].name, "X01");
    assert_eq!(doc.program.rungs[0].elements[2].name, "Y00");
    assert!(doc.program.rungs[0].elements[2].element_type.is_right_aligned());

    // 3. Simulate step:
    let mut sim = PlcSimulator::new();

    // Initial state: X00=false, X01=false, Y00=false
    // NC contact X01 conducts because X01 is false.
    // Branch does NOT conduct because both X00 and Y00 are false.
    sim.step_scan(&mut doc.program, 10);
    assert!(!doc.program.rungs[0].is_energized);
    assert!(!doc.program.rungs[0].elements[0].is_energized);
    assert!(!*sim.outputs.get("Y00").unwrap_or(&false));

    // Press START (X00 = true)
    sim.set_input("X00", true);
    sim.step_scan(&mut doc.program, 10);

    // Motor should now be energized and rung conducting!
    assert!(doc.program.rungs[0].is_energized);
    assert!(doc.program.rungs[0].elements[0].is_energized);
    assert!(doc.program.rungs[0].elements[2].is_energized);
    assert!(*sim.outputs.get("Y00").unwrap_or(&false));

    // Release START (X00 = false), Motor should STAY latched via SEAL_IN (Y00)
    sim.set_input("X00", false);
    sim.step_scan(&mut doc.program, 10);

    assert!(doc.program.rungs[0].is_energized);
    assert!(doc.program.rungs[0].elements[0].is_energized);
    assert!(*sim.outputs.get("Y00").unwrap_or(&false));

    // Press STOP (X01 = true)
    sim.set_input("X01", true);
    sim.step_scan(&mut doc.program, 10);

    // Motor should drop out and rung de-energize
    assert!(!doc.program.rungs[0].is_energized);
    assert!(!*sim.outputs.get("Y00").unwrap_or(&false));
}

#[test]
fn test_iec_61131_3_il_roundtrip() {
    let demo = LadderProgram::new_demo_program();
    let il_text = IlParser::export_il(&demo);

    assert!(il_text.contains("PROGRAM MainProgram"));
    assert!(il_text.contains("VAR"));
    assert!(il_text.contains("END_VAR"));
    assert!(il_text.contains("END_PROGRAM"));
    assert!(il_text.contains("LD  "));
    assert!(il_text.contains("ST  "));

    // Verify auto-detection identifies it as IEC IL
    assert!(IlParser::is_iec_il(&il_text));

    // Parse it back
    let loaded = IlParser::parse(&il_text).expect("Failed to parse IEC 61131-3 IL");
    assert_eq!(loaded.hardware, demo.hardware);
    assert_eq!(loaded.rungs.len(), demo.rungs.len());
    assert!(!loaded.io_mappings.is_empty());
}

#[test]
fn test_iec_61131_3_il_manual_script() {
    let script = r#"
(* ==================================================================== *)
(* IEC 61131-3 Instruction List (IL) Program                            *)
(* Target Hardware: Arduino UNO R3                                      *)
(* ==================================================================== *)

PROGRAM MainProgram
VAR
    X00 AT %IX0.0 : BOOL; (* Pin: 2 | Symbol: START | Start PB *)
    X01 AT %IX0.1 : BOOL; (* Pin: 3 | Symbol: STOP | Stop PB *)
    Y00 AT %QX0.0 : BOOL; (* Pin: 8 | Symbol: MOTOR | Motor Contactor *)
    T00 : TON; (* Preset: 3000ms *)
END_VAR

(* Network 01: Motor Start/Stop with Self-Holding *)
    LD   X00
    OR   Y00
    ANDN X01
    ST   Y00

(* Network 02: Timer Rung *)
    LD   Y00
    CAL  T00(IN := TRUE, PT := T#3000ms)
    ST   Y00

END_PROGRAM
"#;

    let prog = IlParser::parse(script).expect("Failed to parse manual IL");
    assert_eq!(prog.hardware, "Arduino UNO R3");
    assert_eq!(prog.rungs.len(), 2);
    assert_eq!(prog.rungs[0].comment, "Motor Start/Stop with Self-Holding");
    assert_eq!(prog.rungs[0].elements.len(), 3);
    assert_eq!(prog.rungs[0].elements[0].element_type, TypeTag::Branch);
    assert_eq!(prog.rungs[0].elements[1].element_type, TypeTag::Contact);
    assert_eq!(prog.rungs[0].elements[1].contact_mode, ContactMode::NormallyClosed);
    assert_eq!(prog.rungs[0].elements[2].element_type, TypeTag::Coil);

    assert_eq!(prog.rungs[1].elements.len(), 3);
    assert_eq!(prog.rungs[1].elements[1].element_type, TypeTag::Ton);
    assert_eq!(prog.rungs[1].elements[1].preset_value, 3000);
}

#[test]
fn test_load_all_example_il_files() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let example_dir = manifest_dir.parent().unwrap().parent().unwrap().join("example");

    let files = [
        "01_motor_start_stop",
        "02_star_delta_timer",
        "03_traffic_light",
        "04_counter_batch_pack",
        "05_auto_door_control",
        "06_math_compare_analog",
    ];

    for base in files {
        let il_path = example_dir.join(format!("{}.il", base));
        let il_content = std::fs::read_to_string(&il_path).expect(&format!("Failed to read {:?}", il_path));
        let prog = IlParser::parse(&il_content).expect(&format!("Failed to parse {:?}", il_path));
        assert_eq!(prog.hardware, "Arduino UNO R3");
        assert!(!prog.rungs.is_empty());
        assert!(!prog.io_mappings.is_empty());

        // Re-export and verify round-trip
        let exported = IlParser::export_il(&prog);
        let reloaded = IlParser::parse(&exported).expect(&format!("Failed to reload {:?}", il_path));
        assert_eq!(reloaded.rungs.len(), prog.rungs.len());
    }
}

#[test]
fn test_iec_61131_3_transitions_and_function_blocks() {
    use my_canvas_core::{
        ContactMode, Document, LadderElement, PlcSimulator, TypeTag, IlParser, Rung,
    };

    let mut doc = Document::new();
    doc.program.rungs.clear();

    // Rung 1: Transition Contact P -> Coil Y0
    let mut rung1 = Rung::new(1);
    let contact_p = LadderElement::new_contact(1, "X0", "IN", ContactMode::PositiveTransition);
    let coil_y0 = LadderElement::new_coil(2, "Y0", "OUT", my_canvas_core::CoilMode::Normal);
    rung1.elements.push(contact_p);
    rung1.elements.push(coil_y0);
    doc.program.rungs.push(rung1);

    // Rung 2: Transition Contact N -> Coil Y1
    let mut rung2 = Rung::new(2);
    let contact_n = LadderElement::new_contact(3, "X1", "IN2", ContactMode::NegativeTransition);
    let coil_y1 = LadderElement::new_coil(4, "Y1", "OUT2", my_canvas_core::CoilMode::Normal);
    rung2.elements.push(contact_n);
    rung2.elements.push(coil_y1);
    doc.program.rungs.push(rung2);

    // Rung 3: Normal Contact X2 -> TP Timer T0
    let mut rung3 = Rung::new(3);
    let contact_x2 = LadderElement::new_contact(5, "X2", "TRIG", ContactMode::NormallyOpen);
    let mut tp_elem = LadderElement::new_ton(6, "T0", "TP_TIMER", 100);
    tp_elem.element_type = TypeTag::Tp;
    rung3.elements.push(contact_x2);
    rung3.elements.push(tp_elem);
    doc.program.rungs.push(rung3);

    let mut sim = PlcSimulator::new();

    // Scan 0: Everything false
    sim.set_input("X0", false);
    sim.set_input("X1", true); // Set X1 to true initially so we can test falling edge later
    sim.step_scan(&mut doc.program, 10);
    assert_eq!(*sim.outputs.get("Y0").unwrap_or(&false), false);

    // Scan 1: X0 transitions false -> true (Rising edge!) -> Y0 is energized!
    sim.set_input("X0", true);
    sim.step_scan(&mut doc.program, 10);
    assert_eq!(*sim.outputs.get("Y0").unwrap_or(&false), true);

    // Scan 2: X0 remains true -> Y0 is DE-ENERGIZED (only 1 scan!)
    sim.step_scan(&mut doc.program, 10);
    assert_eq!(*sim.outputs.get("Y0").unwrap_or(&false), false);

    // Scan 3: X1 transitions true -> false (Falling edge!) -> Y1 is energized!
    sim.set_input("X1", false);
    sim.step_scan(&mut doc.program, 10);
    assert_eq!(*sim.outputs.get("Y1").unwrap_or(&false), true);

    // Scan 4: X1 remains false -> Y1 is DE-ENERGIZED (only 1 scan!)
    sim.step_scan(&mut doc.program, 10);
    assert_eq!(*sim.outputs.get("Y1").unwrap_or(&false), false);

    // Test TP pulse timer:
    // Trigger with X2 = true
    sim.set_input("X2", true);
    sim.step_scan(&mut doc.program, 20);
    // TP timer is running and energized
    assert!(doc.program.rungs[2].elements[1].is_energized);

    // Release X2 back to false while timer is still running; standard TP remains active for full PT!
    sim.set_input("X2", false);
    sim.step_scan(&mut doc.program, 20);
    assert!(doc.program.rungs[2].elements[1].is_energized);

    // Advance beyond 100ms preset -> TP finishes and turns off
    sim.step_scan(&mut doc.program, 100);
    assert!(!doc.program.rungs[2].elements[1].is_energized);

    // 4. Test IEC 61131-3 IL Roundtrip
    let il_code = IlParser::export_il(&doc.program);
    assert!(il_code.contains("LDP\tX0") || il_code.contains("LDP  X0"));
    assert!(il_code.contains("LDF\tX1") || il_code.contains("LDF  X1"));
    assert!(il_code.contains("TP"));

    let loaded = IlParser::parse(&il_code).expect("Should parse IEC IL successfully");
    assert_eq!(loaded.rungs.len(), 3);
    assert_eq!(loaded.rungs[0].elements[0].contact_mode, ContactMode::PositiveTransition);
    assert_eq!(loaded.rungs[1].elements[0].contact_mode, ContactMode::NegativeTransition);
    assert_eq!(loaded.rungs[2].elements[1].element_type, TypeTag::Tp);
}




