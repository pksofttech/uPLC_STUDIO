use my_canvas_core::{
    CoilMode, ContactMode, Document, IlParser, InsertPosition, LadderElement, LadderProgram, LdParser, SimState, TypeTag,
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
        "001",
        "01_motor_start_stop",
        "02_star_delta_timer",
        "03_traffic_light",
        "04_counter_batch_pack",
        "05_auto_door_control",
        "06_math_compare_analog",
        "07_all_iec_components",
        "08_rp2040_demo_led_blink",
        "09_modbus_rtu_master",
    ];

    for base in files {
        let il_path = example_dir.join(format!("{}.il", base));
        let il_content = std::fs::read_to_string(&il_path).expect(&format!("Failed to read {:?}", il_path));
        let prog = IlParser::parse(&il_content).expect(&format!("Failed to parse {:?}", il_path));
        assert!(
            prog.hardware == "Arduino UNO R3" || prog.hardware == "Raspberry Pi Pico (RP2040)",
            "Unexpected hardware: {}",
            prog.hardware
        );
        assert!(!prog.rungs.is_empty());
        assert!(!prog.io_mappings.is_empty());

        // Verify all hardware I/O mappings use standard IEC %IX / %QX addresses and symbolic names
        for io in &prog.io_mappings {
            assert!(
                io.iec_address.starts_with("%IX") || io.iec_address.starts_with("%QX"),
                "Example {} has non-IEC io_mapping address: {}",
                base,
                io.iec_address
            );
            assert!(!io.symbol.is_empty(), "Example {} has empty symbol", base);
            assert!(
                !io.symbol.starts_with("X0") && !io.symbol.starts_with("Y0"),
                "Example {} still uses legacy X0/Y0 symbol: {}",
                base,
                io.symbol
            );
        }

        // Verify all collected elements have valid IEC addresses (preserving bit index .0, .1, etc.) and symbols
        let iec_elems = prog.collect_all_iec_elements();
        assert!(!iec_elems.is_empty());
        for (raw_name, iec_addr, symbol, _, _) in &iec_elems {
            if raw_name.starts_with('%') {
                assert_eq!(raw_name, iec_addr);
                if raw_name.starts_with("%IX") || raw_name.starts_with("%QX") || raw_name.starts_with("%MX") {
                    assert!(
                        raw_name.contains('.'),
                        "Example {} bit address {} lost its bit index",
                        base,
                        raw_name
                    );
                }
            }
            assert!(!symbol.is_empty());
        }

        // Re-export and verify round-trip
        let exported = IlParser::export_il(&prog);
        let reloaded = IlParser::parse(&exported).expect(&format!("Failed to reload {:?}", il_path));
        assert_eq!(reloaded.rungs.len(), prog.rungs.len());
        assert_eq!(reloaded.io_mappings.len(), prog.io_mappings.len());
    }
}

#[test]
fn test_modbus_rtu_master_full_lifecycle() {
    use my_canvas_core::bytecode::BytecodeCompiler;
    use uplc_vm::modbus::*;
    use uplc_vm::MicroVm;

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let il_path = manifest_dir.parent().unwrap().parent().unwrap().join("example/09_modbus_rtu_master.il");
    let il_content = std::fs::read_to_string(&il_path).expect("Failed to read 09_modbus_rtu_master.il");

    // 1. Parse IL to LadderProgram
    let prog = IlParser::parse(&il_content).expect("Failed to parse IL");
    assert_eq!(prog.rungs.len(), 6);

    // 2. Compile to Bytecode
    let bc = BytecodeCompiler::compile_program(&prog);
    assert!(!bc.instructions.is_empty());

    for (i, ins) in bc.instructions.iter().enumerate() {
        println!("{}: {:?} addr={} imm={} il={}", i, ins.op, ins.addr, ins.imm, ins.il_source);
    }

    // 3. Load into MicroVm
    let raw_payload = bc.to_raw_bytes();
    let mut vm = MicroVm::new();
    vm.load_bytecode(&raw_payload).expect("Failed to load bytecode into VM");
    assert!(vm.prog_len > 0);

    // 4. Test Manual Trigger to Step 1
    // Press MANUAL_TRIG (%IX0.1 -> X1)
    vm.set_digital_input(1, true);
    vm.step_scan(10);
    vm.set_digital_input(1, false);

    // Verify Step 1 is active (M_STEP1_TEMP = %MX0.0 -> bit_mem[0x40])
    assert_eq!(vm.bit_mem[0x40], 1, "M_STEP1_TEMP should be active");

    // 5. Scan: MB0 triggers request to Slave 1 (Temp Sensor)
    vm.step_scan(10);
    assert!(vm.mb_master[0].busy, "MB0 should be BUSY");
    assert!(vm.mb_out_job.is_some(), "MB0 should dispatch a Modbus job");

    let job1 = vm.mb_out_job.take().unwrap();
    assert_eq!(job1.slave_id, 1);
    assert_eq!(job1.function_code, MB_FC_READ_HOLDING_REGISTERS);
    assert_eq!(job1.reg_addr, 1);
    assert_eq!(job1.count, 1);
    assert_eq!(job1.buffer_addr, 0x20 + 10); // D10

    // Verify RTU Request Frame encoding
    let mut tx_frame = [0u8; 16];
    let tx_len = encode_rtu_request(&job1, &vm.word_mem, &vm.bit_mem, &mut tx_frame).unwrap();
    assert_eq!(tx_len, 8);
    assert_eq!(&tx_frame[..6], &[0x01, 0x03, 0x00, 0x01, 0x00, 0x01]);
    let req_crc = compute_crc16(&tx_frame[..6]);
    assert_eq!(u16::from_le_bytes([tx_frame[6], tx_frame[7]]), req_crc);

    // Simulate Slave 1 Response: Temp = 285 (28.5 °C)
    let mut resp1 = [0x01, 0x03, 0x02, 0x01, 0x1D, 0x00, 0x00];
    let crc1 = compute_crc16(&resp1[..5]);
    let [c1_lo, c1_hi] = crc1.to_le_bytes();
    resp1[5] = c1_lo;
    resp1[6] = c1_hi;

    let dec_res = decode_rtu_response(&job1, &resp1, &mut vm.word_mem, &mut vm.bit_mem);
    assert!(dec_res.is_ok());
    assert_eq!(vm.word_mem[0x20 + 10], 285, "D10 should contain 285");

    vm.mb_in_result = Some(ModbusJobResult {
        job_id: job1.job_id,
        instance_id: job1.instance_id,
        success: true,
        error_code: MB_ERR_OK,
    });

    // 6. Next scan: MB0 finishes (DONE), Step 1 resets, Step 2 sets (M_STEP2_METER)
    vm.step_scan(10);
    assert!(!vm.mb_master[0].busy);
    assert_eq!(vm.bit_mem[0x40], 0, "M_STEP1_TEMP should be reset");
    assert_eq!(vm.bit_mem[0x41], 1, "M_STEP2_METER should be active");

    // 7. Scan: MB1 triggers request to Slave 2 (Power Meter)
    vm.step_scan(10);
    assert!(vm.mb_master[1].busy, "MB1 should be BUSY");
    let job2 = vm.mb_out_job.take().unwrap();
    assert_eq!(job2.slave_id, 2);
    assert_eq!(job2.function_code, MB_FC_READ_HOLDING_REGISTERS);
    assert_eq!(job2.reg_addr, 16);
    assert_eq!(job2.count, 2);
    assert_eq!(job2.buffer_addr, 0x20 + 20); // D20

    // Simulate Slave 2 Response: Power = 3500 W, Voltage = 230 V
    let mut resp2 = [0x02, 0x03, 0x04, 0x0D, 0xAC, 0x00, 0xE6, 0x00, 0x00]; // 3500, 230
    let crc2 = compute_crc16(&resp2[..7]);
    let [c2_lo, c2_hi] = crc2.to_le_bytes();
    resp2[7] = c2_lo;
    resp2[8] = c2_hi;

    let dec_res2 = decode_rtu_response(&job2, &resp2, &mut vm.word_mem, &mut vm.bit_mem);
    assert!(dec_res2.is_ok());
    assert_eq!(vm.word_mem[0x20 + 20], 3500, "D20 should contain 3500");
    assert_eq!(vm.word_mem[0x20 + 21], 230, "D21 should contain 230");

    vm.mb_in_result = Some(ModbusJobResult {
        job_id: job2.job_id,
        instance_id: job2.instance_id,
        success: true,
        error_code: MB_ERR_OK,
    });

    // 8. Next scan: MB1 finishes, Step 3 activates (M_STEP3_VFD)
    vm.step_scan(10);
    assert_eq!(vm.bit_mem[0x41], 0, "M_STEP2_METER should be reset");
    assert_eq!(vm.bit_mem[0x42], 1, "M_STEP3_VFD should be active");

    // Preload D30 with 5000 (50.00 Hz)
    vm.word_mem[0x20 + 30] = 5000;

    // Scan: MB2 triggers Write to Slave 3 (Inverter)
    vm.step_scan(10);
    assert!(vm.mb_master[2].busy, "MB2 should be BUSY");
    let job3 = vm.mb_out_job.take().unwrap();
    assert_eq!(job3.slave_id, 3);
    assert_eq!(job3.function_code, MB_FC_WRITE_SINGLE_REGISTER);
    assert_eq!(job3.reg_addr, 8193);
    assert_eq!(job3.buffer_addr, 0x20 + 30); // D30

    // Verify Write Frame contains 5000 (0x1388)
    let mut tx_frame3 = [0u8; 16];
    let tx_len3 = encode_rtu_request(&job3, &vm.word_mem, &vm.bit_mem, &mut tx_frame3).unwrap();
    assert_eq!(tx_len3, 8);
    assert_eq!(&tx_frame3[..6], &[0x03, 0x06, 0x20, 0x01, 0x13, 0x88]);

    // Simulate Slave 3 echo response
    let dec_res3 = decode_rtu_response(&job3, &tx_frame3, &mut vm.word_mem, &mut vm.bit_mem);
    assert!(dec_res3.is_ok());

    vm.mb_in_result = Some(ModbusJobResult {
        job_id: job3.job_id,
        instance_id: job3.instance_id,
        success: true,
        error_code: MB_ERR_OK,
    });

    // 9. Next scan: MB2 completes, sequence resets cleanly
    vm.step_scan(10);
    assert_eq!(vm.bit_mem[0x42], 0, "M_STEP3_VFD should be reset");
    assert!(!vm.mb_master[2].busy);

    // Final checks: Verify values persisted in memory
    assert_eq!(vm.word_mem[0x20 + 10], 285);
    assert_eq!(vm.word_mem[0x20 + 20], 3500);
    assert_eq!(vm.word_mem[0x20 + 21], 230);
    assert_eq!(vm.word_mem[0x20 + 30], 5000);
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
#[test]
fn test_simulation_mode_transitions_and_safety() {
    let mut doc = Document::default();

    // 1. Initial State: Stopped
    assert!(doc.simulator.is_stopped());
    assert!(!doc.simulator.is_running());
    assert!(!doc.simulator.is_active());
    assert_eq!(doc.simulator.scan_count, 0);

    // 2. Transition: Stopped -> Running
    doc.start_simulation();
    assert!(doc.simulator.is_running());
    assert!(doc.simulator.is_active());
    assert!(!doc.simulator.is_stopped());
    // Initial scan executed immediately on start
    assert_eq!(doc.simulator.scan_count, 1);

    // Latch motor circuit: Start button X0 = true
    doc.simulator.set_input("X0", true);
    doc.step_simulation(10);
    assert_eq!(doc.simulator.get_var_value("Y0"), true);
    assert!(doc.program.rungs[0].is_energized);

    // Release X0: Y0 stays latched ON
    doc.simulator.set_input("X0", false);
    doc.step_simulation(10);
    assert_eq!(doc.simulator.get_var_value("Y0"), true);

    // 3. Transition: Running -> Stopped (Emergency / Safe Stop)
    // Standard PLC Safety Rule: All physical outputs MUST turn OFF immediately!
    doc.stop_simulation();
    assert!(doc.simulator.is_stopped());
    assert!(!doc.simulator.is_running());
    assert!(!doc.simulator.is_active());

    // Verify all physical outputs are safely de-energized
    assert_eq!(doc.simulator.get_var_value("Y0"), false);
    assert_eq!(doc.simulator.get_var_value("Y1"), false);
    assert!(!doc.program.rungs[0].is_energized);
    assert!(!doc.program.rungs[0].elements[0].is_energized);

    // 4. Transition: Stopped -> Step mode
    doc.single_step_simulation(10);
    assert!(doc.simulator.is_step());
    assert!(doc.simulator.is_active());
    assert!(!doc.simulator.is_running());

    // In step mode, toggling input immediately evaluates the current scan step
    let old_scans = doc.simulator.scan_count;
    doc.toggle_input("X0");
    assert_eq!(doc.simulator.get_var_value("X0"), true);
    // Since X0 is now true, motor Y0 energizes in this step
    assert_eq!(doc.simulator.get_var_value("Y0"), true);
    assert!(doc.program.rungs[0].is_energized);

    // Step scan count updated
    assert!(doc.simulator.scan_count >= old_scans);

    // 5. Transition: Step -> Running (Resume continuous)
    doc.toggle_simulation();
    assert!(doc.simulator.is_running());
    assert!(doc.simulator.is_active());

    // 6. Transition: Running -> Reset
    doc.reset_simulation();
    assert!(doc.simulator.is_stopped());
    assert!(!doc.simulator.is_active());
    assert_eq!(doc.simulator.scan_count, 0);
    assert_eq!(doc.simulator.simulation_time_ms, 0);
    assert_eq!(doc.simulator.get_var_value("Y0"), false);
    assert_eq!(doc.simulator.get_var_value("Y1"), false);
    assert!(!doc.program.rungs[0].is_energized);
}

#[test]
fn test_ld_move_st_inline_execution_and_roundtrip() {
    let il_script = r#"
PROGRAM
RUNG
    LD   X00
    MOVE V00 10
    ST   Y00
END
END
"#;

    let mut prog = IlParser::parse(il_script).expect("Should parse LD -> MOVE -> ST successfully");
    assert_eq!(prog.rungs.len(), 1);
    assert_eq!(prog.rungs[0].elements.len(), 3);

    // Element 0: Contact X00
    assert_eq!(prog.rungs[0].elements[0].element_type, TypeTag::Contact);
    assert_eq!(prog.rungs[0].elements[0].name, "X00");

    // Element 1: MOVE V00 10
    assert_eq!(prog.rungs[0].elements[1].element_type, TypeTag::Move);
    assert_eq!(prog.rungs[0].elements[1].name, "V00");
    assert_eq!(prog.rungs[0].elements[1].preset_value, 10);

    // Element 2: Coil Y00
    assert_eq!(prog.rungs[0].elements[2].element_type, TypeTag::Coil);
    assert_eq!(prog.rungs[0].elements[2].name, "Y00");

    // Test simulation execution
    let mut sim = my_canvas_core::PlcSimulator::default();
    sim.start(&mut prog);

    // Scan 1: X00 is false -> MOVE does not write, Y00 is false
    assert_eq!(*sim.registers.get("V00").unwrap_or(&0), 0);
    assert_eq!(sim.get_var_value("Y00"), false);
    assert!(!prog.rungs[0].is_energized);
    assert!(!prog.rungs[0].elements[1].is_energized);

    // Scan 2: Set X00 = true -> MOVE copies 10 to V00, power flows through ENO, Y00 energizes!
    sim.set_input("X00", true);
    sim.step_scan(&mut prog, 10);

    assert_eq!(*sim.registers.get("V00").unwrap_or(&0), 10);
    assert_eq!(sim.get_var_value("Y00"), true);
    assert!(prog.rungs[0].is_energized);
    assert!(prog.rungs[0].elements[1].is_energized); // Move block is energized
    assert!(prog.rungs[0].elements[2].is_energized); // Coil is energized

    // Test Export roundtrip
    let exported = IlParser::export_il(&prog);
    assert!(exported.contains("LD   X00") || exported.contains("LD\tX00"));
    assert!(exported.contains("MOVE V00 10") || exported.contains("MOVE\tV00 10"));
    assert!(exported.contains("ST   Y00") || exported.contains("ST\tY00"));
}

#[test]
fn test_all_iec_components_comprehensive_matrix() {
    let mut prog = LadderProgram::new_empty();
    prog.rungs.clear();

    // Rung 0: All Contact Modes into Coils
    let mut r0 = my_canvas_core::Rung::new(0);
    r0.elements.push(LadderElement::new_contact(1, "X0", "PB_NO", ContactMode::NormallyOpen));
    r0.elements.push(LadderElement::new_contact(2, "X1", "PB_NC", ContactMode::NormallyClosed));
    r0.elements.push(LadderElement::new_contact(3, "X2", "PB_P", ContactMode::PositiveTransition));
    r0.elements.push(LadderElement::new_contact(4, "X3", "PB_N", ContactMode::NegativeTransition));
    r0.elements.push(LadderElement::new_coil(5, "Y0", "OUT_NORM", CoilMode::Normal));
    prog.rungs.push(r0);

    // Rung 1: Set & Reset Latch Coils
    let mut r1 = my_canvas_core::Rung::new(1);
    r1.elements.push(LadderElement::new_contact(6, "X4", "SET_BTN", ContactMode::NormallyOpen));
    r1.elements.push(LadderElement::new_coil(7, "M0", "LATCH_M0", CoilMode::SetLatch));
    prog.rungs.push(r1);

    let mut r2 = my_canvas_core::Rung::new(2);
    r2.elements.push(LadderElement::new_contact(8, "X5", "RST_BTN", ContactMode::NormallyOpen));
    r2.elements.push(LadderElement::new_coil(9, "M0", "UNLATCH_M0", CoilMode::ResetUnlatch));
    prog.rungs.push(r2);

    // Rung 3: Timers (TON, TOF, TP)
    let mut r3 = my_canvas_core::Rung::new(3);
    r3.elements.push(LadderElement::new_contact(10, "X6", "TMR_EN", ContactMode::NormallyOpen));
    let mut ton_el = LadderElement::new_contact(11, "T0", "TIMER_ON", ContactMode::NormallyOpen);
    ton_el.element_type = TypeTag::Ton;
    ton_el.preset_value = 500;
    r3.elements.push(ton_el);
    r3.elements.push(LadderElement::new_coil(12, "Y1", "TMR_OUT", CoilMode::Normal));
    prog.rungs.push(r3);

    let mut r4 = my_canvas_core::Rung::new(4);
    r4.elements.push(LadderElement::new_contact(13, "X7", "TOF_EN", ContactMode::NormallyOpen));
    let mut tof_el = LadderElement::new_contact(14, "T1", "TIMER_OFF", ContactMode::NormallyOpen);
    tof_el.element_type = TypeTag::Tof;
    tof_el.preset_value = 1000;
    r4.elements.push(tof_el);
    r4.elements.push(LadderElement::new_coil(15, "Y2", "TOF_OUT", CoilMode::Normal));
    prog.rungs.push(r4);

    let mut r5 = my_canvas_core::Rung::new(5);
    r5.elements.push(LadderElement::new_contact(16, "X0", "TP_EN", ContactMode::NormallyOpen));
    let mut tp_el = LadderElement::new_contact(17, "T2", "TIMER_PULSE", ContactMode::NormallyOpen);
    tp_el.element_type = TypeTag::Tp;
    tp_el.preset_value = 250;
    r5.elements.push(tp_el);
    r5.elements.push(LadderElement::new_coil(18, "Y3", "TP_OUT", CoilMode::Normal));
    prog.rungs.push(r5);

    // Rung 6: Counters (CTU, CTD, CTUD)
    let mut r6 = my_canvas_core::Rung::new(6);
    r6.elements.push(LadderElement::new_contact(19, "X1", "PULSE_IN", ContactMode::NormallyOpen));
    let mut ctu_el = LadderElement::new_contact(20, "C0", "COUNT_UP", ContactMode::NormallyOpen);
    ctu_el.element_type = TypeTag::Ctu;
    ctu_el.preset_value = 5;
    r6.elements.push(ctu_el);
    r6.elements.push(LadderElement::new_coil(21, "Y4", "CTU_OUT", CoilMode::Normal));
    prog.rungs.push(r6);

    // Rung 7: Edge Detection Function Blocks (R_TRIG, F_TRIG)
    let mut r7 = my_canvas_core::Rung::new(7);
    r7.elements.push(LadderElement::new_contact(22, "X2", "TRIG_IN", ContactMode::NormallyOpen));
    let mut rtrig_el = LadderElement::new_contact(23, "RTRIG_1", "R_TRIG_1", ContactMode::NormallyOpen);
    rtrig_el.element_type = TypeTag::RTrig;
    r7.elements.push(rtrig_el);
    r7.elements.push(LadderElement::new_coil(24, "Y5", "PULSE_OUT", CoilMode::Normal));
    prog.rungs.push(r7);

    // Rung 8: Compare, Math & Move
    let mut r8 = my_canvas_core::Rung::new(8);
    r8.elements.push(LadderElement::new_contact(25, "X3", "CALC_EN", ContactMode::NormallyOpen));
    let mut cmp_el = LadderElement::new_contact(26, "V0", "CMP_V0", ContactMode::NormallyOpen);
    cmp_el.element_type = TypeTag::Compare;
    cmp_el.compare_op = ">=".to_string();
    cmp_el.preset_value = 10;
    r8.elements.push(cmp_el);

    let mut move_el = LadderElement::new_contact(27, "V1", "MOVE_V1", ContactMode::NormallyOpen);
    move_el.element_type = TypeTag::Move;
    move_el.preset_value = 99;
    r8.elements.push(move_el);
    r8.elements.push(LadderElement::new_coil(28, "Y6", "CALC_OUT", CoilMode::Normal));
    prog.rungs.push(r8);

    // Verify all 9 networks validate and export
    assert_eq!(prog.rungs.len(), 9);
    let il = IlParser::export_il(&prog);
    assert!(il.contains("PROGRAM"));
    assert!(il.contains("VAR"));
    assert!(il.contains("END_VAR"));

    // Verify simulation scan executes across all 9 IEC rungs without error
    let mut sim = my_canvas_core::PlcSimulator::default();
    sim.start(&mut prog);
    assert_eq!(sim.scan_count, 1);
    sim.step_scan(&mut prog, 50);
    assert_eq!(sim.scan_count, 2);
}

#[test]
fn test_bistable_sr_and_rs() {
    use my_canvas_core::{ContactMode, LadderElement, LadderProgram, PlcSimulator, Rung, TypeTag};

    // --- 1. Test SR (Set dominant) ---
    // Rung: Contact X0 (S1) -> SR (target_reg: "X1", name: "M0")
    let mut prog = LadderProgram::default();
    let mut r0 = Rung::new(0);
    r0.elements.push(LadderElement::new_contact(1, "X0", "SET", ContactMode::NormallyOpen));
    let mut sr_el = LadderElement::new_contact(2, "M0", "SR_FB", ContactMode::NormallyOpen);
    sr_el.element_type = TypeTag::Sr;
    sr_el.target_reg = "X1".to_string(); // Reset input
    r0.elements.push(sr_el);
    prog.rungs.push(r0);

    let mut sim = PlcSimulator::default();

    // S1=0, R=0 => Q=0
    sim.set_input("X0", false);
    sim.set_input("X1", false);
    sim.step_scan(&mut prog, 10);
    assert!(!sim.get_var_value("M0"));

    // S1=1, R=0 => Q=1 (Set)
    sim.set_input("X0", true);
    sim.step_scan(&mut prog, 10);
    assert!(sim.get_var_value("M0"));

    // S1=0, R=0 => Q=1 (Latched)
    sim.set_input("X0", false);
    sim.step_scan(&mut prog, 10);
    assert!(sim.get_var_value("M0"));

    // S1=0, R=1 => Q=0 (Reset)
    sim.set_input("X1", true);
    sim.step_scan(&mut prog, 10);
    assert!(!sim.get_var_value("M0"));

    // S1=1, R=1 => Q=1 (Set-dominant!)
    sim.set_input("X0", true);
    sim.set_input("X1", true);
    sim.step_scan(&mut prog, 10);
    assert!(sim.get_var_value("M0"), "SR must be Set-dominant when S1 and R are both TRUE");

    // --- 2. Test RS (Reset dominant) ---
    let mut prog_rs = LadderProgram::default();
    let mut r_rs = Rung::new(0);
    r_rs.elements.push(LadderElement::new_contact(10, "X2", "SET", ContactMode::NormallyOpen));
    let mut rs_el = LadderElement::new_contact(11, "M1", "RS_FB", ContactMode::NormallyOpen);
    rs_el.element_type = TypeTag::Rs;
    rs_el.target_reg = "X3".to_string(); // Reset input
    r_rs.elements.push(rs_el);
    prog_rs.rungs.push(r_rs);

    let mut sim_rs = PlcSimulator::default();

    // S=1, R1=0 => Q=1
    sim_rs.set_input("X2", true);
    sim_rs.set_input("X3", false);
    sim_rs.step_scan(&mut prog_rs, 10);
    assert!(sim_rs.get_var_value("M1"));

    // S=0, R1=0 => Q=1 (Latched)
    sim_rs.set_input("X2", false);
    sim_rs.step_scan(&mut prog_rs, 10);
    assert!(sim_rs.get_var_value("M1"));

    // S=1, R1=1 => Q=0 (Reset-dominant!)
    sim_rs.set_input("X2", true);
    sim_rs.set_input("X3", true);
    sim_rs.step_scan(&mut prog_rs, 10);
    assert!(!sim_rs.get_var_value("M1"), "RS must be Reset-dominant when S and R1 are both TRUE");
}

#[test]
fn test_jump_and_label_flow() {
    use my_canvas_core::{CoilMode, ContactMode, LadderElement, LadderProgram, PlcSimulator, Rung, TypeTag};

    let mut prog = LadderProgram::default();

    // Rung 0: If X0 is ON, JMP to LBL_SKIP
    let mut r0 = Rung::new(0);
    r0.elements.push(LadderElement::new_contact(1, "X0", "COND_JMP", ContactMode::NormallyOpen));
    let mut jmp = LadderElement::new_contact(2, "LBL_SKIP", "JMP", ContactMode::NormallyOpen);
    jmp.element_type = TypeTag::Jump;
    r0.elements.push(jmp);
    prog.rungs.push(r0);

    // Rung 1: Middle rung (should be skipped when X0 is ON)
    let mut r1 = Rung::new(1);
    r1.elements.push(LadderElement::new_contact(3, "X1", "ALWAYS_ON", ContactMode::NormallyOpen));
    r1.elements.push(LadderElement::new_coil(4, "Y1", "MIDDLE_COIL", CoilMode::Normal));
    prog.rungs.push(r1);

    // Rung 2: Destination Label LBL_SKIP, then turn on Y2
    let mut r2 = Rung::new(2);
    let mut lbl = LadderElement::new_contact(5, "LBL_SKIP", "LBL", ContactMode::NormallyOpen);
    lbl.element_type = TypeTag::Label;
    r2.elements.push(lbl);
    r2.elements.push(LadderElement::new_contact(6, "X2", "LANDING_IN", ContactMode::NormallyOpen));
    r2.elements.push(LadderElement::new_coil(7, "Y2", "LANDING_OUT", CoilMode::Normal));
    prog.rungs.push(r2);

    let mut sim = PlcSimulator::default();
    sim.set_input("X1", true);
    sim.set_input("X2", true);

    // Case A: X0 is FALSE -> No Jump -> Y1 turns ON, Y2 turns ON
    sim.set_input("X0", false);
    sim.step_scan(&mut prog, 10);
    assert!(sim.get_var_value("Y1"), "Y1 should be ON when Jump is not taken");
    assert!(sim.get_var_value("Y2"), "Y2 should be ON");

    // Case B: X0 is TRUE -> Jump taken -> Rung 1 skipped -> Y1 remains FALSE
    sim.set_var_value("Y1", false);
    sim.set_input("X0", true);
    sim.step_scan(&mut prog, 10);
    assert!(!sim.get_var_value("Y1"), "Y1 should NOT be energized when jumped over");
    assert!(sim.get_var_value("Y2"), "Landing rung Y2 should be executed");
}

#[test]
fn test_iec_il_sr_rs_jmp_roundtrip() {
    use my_canvas_core::{ContactMode, IlParser, LadderElement, LadderProgram, Rung, TypeTag};

    let mut prog = LadderProgram::new_empty();

    // Rung 0: SR
    let mut r0 = Rung::new(0);
    r0.elements.push(LadderElement::new_contact(1, "X0", "S_IN", ContactMode::NormallyOpen));
    let mut sr = LadderElement::new_contact(2, "SR_INST", "SR_FB", ContactMode::NormallyOpen);
    sr.element_type = TypeTag::Sr;
    sr.target_reg = "X1".to_string();
    r0.elements.push(sr);
    prog.rungs.push(r0);

    // Rung 1: JMP
    let mut r1 = Rung::new(1);
    r1.elements.push(LadderElement::new_contact(3, "X2", "J_COND", ContactMode::NormallyOpen));
    let mut jmp = LadderElement::new_contact(4, "END_LOOP", "JMP", ContactMode::NormallyOpen);
    jmp.element_type = TypeTag::Jump;
    r1.elements.push(jmp);
    prog.rungs.push(r1);

    // Rung 2: LBL
    let mut r2 = Rung::new(2);
    let mut lbl = LadderElement::new_contact(5, "END_LOOP", "LBL", ContactMode::NormallyOpen);
    lbl.element_type = TypeTag::Label;
    r2.elements.push(lbl);
    prog.rungs.push(r2);

    let exported = IlParser::export_il(&prog);
    assert!(exported.contains("SR_INST : SR;"));
    assert!(exported.contains("CAL  SR_INST(S1 := TRUE, R := X1)"));
    assert!(exported.contains("JMPC END_LOOP"));
    assert!(exported.contains("END_LOOP:"));

    let parsed = IlParser::parse(&exported).expect("parse exported IL");
    assert_eq!(parsed.rungs.len(), 3);
}

#[test]
fn test_label_left_rail_anchoring() {
    use my_canvas_core::{ContactMode, Document, InsertPosition, LadderElement, TypeTag};

    let mut doc = Document::default();
    doc.program.rungs.clear();
    doc.add_rung(); // Create empty Rung 0
    doc.selected_rung_idx = Some(0);

    // 1. Add normal contact X0 and coil Y0
    let c0 = LadderElement::new_contact(10, "X0", "START", ContactMode::NormallyOpen);
    let y0 = LadderElement::new_coil(11, "Y0", "MOTOR", my_canvas_core::CoilMode::Normal);
    doc.insert_element(c0, InsertPosition::AppendRung);
    doc.insert_element(y0, InsertPosition::AppendRung);

    assert_eq!(doc.program.rungs[0].elements.len(), 2);
    assert_eq!(doc.program.rungs[0].elements[0].name, "X0");

    // 2. Insert TypeTag::Label with AppendRung
    let mut lbl = LadderElement::new_contact(12, "TARGET_1", "LBL", ContactMode::NormallyOpen);
    lbl.element_type = TypeTag::Label;
    doc.insert_element(lbl, InsertPosition::AppendRung);

    // Hybrid Anchor Rule: Label MUST automatically anchor to index 0 (Left Power Rail)!
    assert_eq!(doc.program.rungs[0].elements.len(), 3);
    assert_eq!(doc.program.rungs[0].elements[0].element_type, TypeTag::Label);
    assert_eq!(doc.program.rungs[0].elements[0].name, "TARGET_1");
    assert_eq!(doc.program.rungs[0].elements[1].name, "X0");

    // 3. Verify ensure_label_at_start
    let mut rung_manual = my_canvas_core::Rung::new(1);
    rung_manual.elements.push(LadderElement::new_contact(20, "X1", "IN", ContactMode::NormallyOpen));
    let mut lbl2 = LadderElement::new_contact(21, "TARGET_2", "LBL", ContactMode::NormallyOpen);
    lbl2.element_type = TypeTag::Label;
    rung_manual.elements.push(lbl2);

    assert_eq!(rung_manual.elements[1].element_type, TypeTag::Label);
    rung_manual.ensure_label_at_start();
    assert_eq!(rung_manual.elements[0].element_type, TypeTag::Label);
    assert_eq!(rung_manual.elements[0].name, "TARGET_2");
}

#[test]
fn test_iec_address_aliases_and_simulation() {
    use my_canvas_core::{ContactMode, Document, LadderElement, LadderProgram, PlcSimulator, Rung};

    let mut sim = PlcSimulator::default();

    // 1. Digital Input alias: X0 <-> %IX0.0
    sim.set_input("%IX0.0", true);
    assert!(sim.get_var_value("X0"));
    assert!(sim.get_var_value("X00"));
    assert!(sim.get_var_value("%IX0.0"));

    sim.set_input("X0", false);
    assert!(!sim.get_var_value("%IX0.0"));
    assert!(!sim.get_var_value("X0"));

    // 2. Digital Output alias: Y0 <-> %QX0.0
    sim.set_var_value("%QX0.0", true);
    assert!(sim.get_var_value("Y0"));
    assert!(sim.get_var_value("Y00"));
    assert!(sim.get_var_value("%QX0.0"));

    sim.set_var_value("Y0", false);
    assert!(!sim.get_var_value("%QX0.0"));
    assert!(!sim.get_var_value("Y0"));

    // 3. Marker alias: M0 <-> %MX0.0
    sim.set_var_value("%MX0.0", true);
    assert!(sim.get_var_value("M0"));
    assert!(sim.get_var_value("R0"));
    assert!(sim.get_var_value("%MX0.0"));

    // 4. Test logic rung using pure IEC 61131-3 naming: [%IX0.0] -> (%QX0.0)
    let mut prog = LadderProgram::new_empty();
    let mut r0 = Rung::new(0);
    r0.elements.push(LadderElement::new_contact(1, "%IX0.0", "START_PB", ContactMode::NormallyOpen));
    r0.elements.push(LadderElement::new_coil(2, "%QX0.0", "MOTOR_RUN", my_canvas_core::CoilMode::Normal));
    prog.rungs = vec![r0];

    let mut doc = Document::new();
    doc.program = prog;
    doc.simulator.set_input("%IX0.0", true);
    doc.step_simulation(10);

    assert!(doc.simulator.get_var_value("%QX0.0"));
    assert!(doc.simulator.get_var_value("Y0")); // Legacy Y0 also receives the output!

    doc.simulator.set_input("%IX0.0", false);
    doc.step_simulation(10);
    assert!(!doc.simulator.get_var_value("%QX0.0"));
    assert!(!doc.simulator.get_var_value("Y0"));
}

#[test]
fn test_map_io_to_iec_var_table() {
    use my_canvas_core::{IlParser, LadderProgram};

    let prog = LadderProgram::new_demo_program();

    // 1. Structured Entries mapping
    let entries = prog.map_io_to_iec_var_entries(false);
    assert_eq!(entries.len(), prog.io_mappings.len());
    assert_eq!(entries[0].name, "X0");
    assert_eq!(entries[0].address, "%IX0.0");
    assert_eq!(entries[0].symbol, "START_PB");
    assert_eq!(entries[0].data_type, "BOOL");
    assert_eq!(entries[0].hw_pin, "D2");

    // 2. Symbolic entries mapping
    let sym_entries = prog.map_io_to_iec_var_entries(true);
    assert_eq!(sym_entries[0].name, "START_PB");
    assert_eq!(sym_entries[0].address, "%IX0.0");

    // 3. IEC VAR Table text generation
    let var_table = prog.map_io_to_iec_var_table();
    assert!(var_table.starts_with("VAR\n"));
    assert!(var_table.ends_with("END_VAR"));
    assert!(var_table.contains("X0 AT %IX0.0 : BOOL;"));
    assert!(var_table.contains("X1 AT %IX0.1 : BOOL;"));
    assert!(var_table.contains("Y0 AT %QX0.0 : BOOL;"));
    assert!(var_table.contains("Y1 AT %QX0.1 : BOOL;"));
    assert!(var_table.contains("T0 : TON;"));

    // 4. Symbolic VAR Table text generation
    let sym_var_table = prog.map_io_to_iec_var_table_with_options(true);
    assert!(sym_var_table.contains("START_PB AT %IX0.0 : BOOL;"));
    assert!(sym_var_table.contains("STOP_PB AT %IX0.1 : BOOL;"));
    assert!(sym_var_table.contains("MOTOR_RUN AT %QX0.0 : BOOL;"));

    // 5. Verification with IlParser export and parse
    let full_il = IlParser::export_il(&prog);
    assert!(full_il.contains(&var_table));
    let parsed = IlParser::parse(&full_il).expect("Failed to parse IL generated with VAR table");
    assert_eq!(parsed.io_mappings.len(), prog.io_mappings.len());

    // 6. Verification with custom edited IEC address
    let mut custom_prog = prog.clone();
    custom_prog.io_mappings[0].iec_address = "%IX2.7".to_string();
    let custom_table = custom_prog.map_io_to_iec_var_table();
    assert!(custom_table.contains("X0 AT %IX2.7 : BOOL;"));
}

#[test]
fn test_new_program_clear_state() {
    let mut doc = Document::default();
    doc.program.hardware = "IEC 61131-3 Standard PLC".to_string();
    doc.file_path = Some("example/01_motor_start_stop.il".to_string());
    doc.add_rung();
    doc.start_simulation();
    doc.simulator.set_input("%IX0.0", true);
    doc.step_simulation(10);

    assert!(doc.can_undo());
    assert!(doc.simulator.is_running());
    assert!(doc.simulator.get_var_value("%IX0.0"));
    assert!(doc.simulator.get_var_value("%QX0.0"));

    // Execute New Program (clear)
    doc.clear();

    assert_eq!(doc.program.rungs.len(), 1);
    assert!(doc.program.rungs[0].elements.is_empty());
    assert!(doc.program.io_mappings.is_empty());
    assert_eq!(doc.program.hardware, "IEC 61131-3 Standard PLC");
    assert!(doc.simulator.is_stopped());
    assert!(!doc.simulator.get_var_value("%IX0.0"));
    assert!(!doc.simulator.get_var_value("%QX0.0"));
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.selected_rung_idx, Some(0));
    assert!(doc.selected_element_id.is_none());
    assert!(doc.selected_element_ids.is_empty());
    assert!(doc.file_path.is_none());
    assert!(!doc.is_dirty);
}

#[test]
fn test_drop_to_new_rung() {
    let mut doc = Document::default();
    let initial_rungs = doc.program.rungs.len();

    // 1. Insert new element onto a new rung (appended at end)
    let elem_id = doc.program.alloc_id();
    let elem = my_canvas_core::LadderElement::new_contact(
        elem_id,
        "X100",
        "",
        my_canvas_core::ContactMode::NormallyOpen,
    );
    let new_rung_idx = doc.insert_element_to_new_rung(elem, None);
    assert_eq!(new_rung_idx, initial_rungs);
    assert_eq!(doc.program.rungs.len(), initial_rungs + 1);
    assert_eq!(doc.program.rungs[new_rung_idx].elements.len(), 1);
    assert_eq!(doc.program.rungs[new_rung_idx].elements[0].id, elem_id);
    assert_eq!(doc.selected_rung_idx, Some(new_rung_idx));

    // 2. Undo should remove the new rung and the element atomically in 1 step
    assert!(doc.can_undo());
    doc.undo();
    assert_eq!(doc.program.rungs.len(), initial_rungs);

    // 3. Redo should restore it
    assert!(doc.can_redo());
    doc.redo();
    assert_eq!(doc.program.rungs.len(), initial_rungs + 1);

    // 4. Move an existing element from Rung 0 to a newly created rung at the end
    let moving_elem_id = doc.program.rungs[0].elements[0].id;
    let moved = doc.move_element_to_new_rung(moving_elem_id, None);
    assert!(moved);
    let last_rung_idx = doc.program.rungs.len() - 1;
    assert_eq!(doc.selected_rung_idx, Some(last_rung_idx));
    assert!(doc.program.rungs[last_rung_idx].elements.iter().any(|e| e.id == moving_elem_id));
}

#[test]
fn test_online_monitoring_snapshot_and_conduction() {
    let mut doc = Document::default();
    doc.program = LadderProgram::new_empty();
    doc.program.rungs.clear();

    // Rung 0: LD X0 -> ST Y0
    let mut r0 = my_canvas_core::Rung::new(1);
    r0.elements.push(LadderElement::new_contact(1, "X0", "%IX0.0", ContactMode::NormallyOpen));
    r0.elements.push(LadderElement::new_coil(2, "Y0", "%QX0.0", CoilMode::Normal));
    doc.program.rungs.push(r0);

    // Initial state: not energized
    assert!(!doc.program.rungs[0].is_energized);
    assert!(!doc.program.rungs[0].elements[0].is_done);

    // Create a live snapshot from PLC: X0 = 1, Y0 = 1, Running = true
    let mut snapshot = my_canvas_core::PlcStatusSnapshot::default();
    snapshot.running = true;
    snapshot.scan_count = 555;
    snapshot.bit_mem[0] = 1;      // X0 = 1
    snapshot.bit_mem[0x20] = 1;   // Y0 = 1
    snapshot.acc_bool = true;

    // Apply snapshot to document
    doc.apply_online_snapshot(&snapshot);

    // Verify conduction states update immediately
    assert!(doc.simulator.is_active());
    assert!(doc.program.rungs[0].is_energized, "Rung 0 should be energized");
    assert!(doc.program.rungs[0].elements[0].is_done, "Contact X0 should be closed (ON)");
    assert!(doc.program.rungs[0].elements[0].is_energized);
    assert!(doc.program.rungs[0].elements[1].is_done, "Coil Y0 should be ON");
    assert!(doc.program.rungs[0].elements[1].is_energized);

    // Verify Process Image values in simulator maps
    assert_eq!(doc.simulator.inputs.get("X0"), Some(&true));
    assert_eq!(doc.simulator.inputs.get("%IX0.0"), Some(&true));
    assert_eq!(doc.simulator.outputs.get("Y0"), Some(&true));
    assert_eq!(doc.simulator.outputs.get("%QX0.0"), Some(&true));
    assert_eq!(doc.simulator.scan_count, 555);

    // Now send snapshot with X0 = 0
    snapshot.bit_mem[0] = 0;
    doc.apply_online_snapshot(&snapshot);
    assert!(!doc.program.rungs[0].is_energized, "Rung 0 should drop out");
    assert!(!doc.program.rungs[0].elements[0].is_done, "Contact X0 should be OPEN");
}

#[test]
fn test_online_monitoring_code_mismatch_suppresses_conduction() {
    let mut doc = Document::default();
    doc.program = LadderProgram::new_empty();
    doc.program.rungs.clear();

    // Rung 0: LD X0 -> ST Y0
    let mut r0 = my_canvas_core::Rung::new(1);
    r0.elements.push(LadderElement::new_contact(1, "X0", "%IX0.0", ContactMode::NormallyOpen));
    r0.elements.push(LadderElement::new_coil(2, "Y0", "%QX0.0", CoilMode::Normal));
    doc.program.rungs.push(r0);

    // Initial state: not energized
    assert!(!doc.simulator.is_active());

    // 1. Simulate matching snapshot: X0 = 1, Y0 = 1
    let mut snapshot = my_canvas_core::PlcStatusSnapshot::default();
    snapshot.running = true;
    snapshot.bit_mem[0] = 1;
    snapshot.bit_mem[0x20] = 1;

    let bc = doc.compile_to_bytecode();
    let (prog_len, prog_crc) = bc.program_signature();
    snapshot.prog_len = prog_len;
    snapshot.prog_crc = prog_crc;
    assert!(snapshot.matches_program(&bc));

    // When matched: live monitoring is active
    doc.set_online_mode(true);
    doc.apply_online_snapshot(&snapshot);
    assert!(doc.simulator.is_active());
    assert!(doc.program.rungs[0].is_energized);
    assert!(doc.program.rungs[0].elements[0].is_done);

    // 2. Now simulate CODE MISMATCH (e.g. PLC runs a different program with different CRC16)
    snapshot.prog_crc ^= 0x1234;
    assert!(!snapshot.matches_program(&bc), "Snapshot must report code mismatch");

    // When mismatched: simulation / conduction MUST NOT be shown
    let is_matched = snapshot.matches_program(&bc);
    if !is_matched {
        doc.set_online_mode(false);
        doc.clear_conduction();
    }

    assert!(!doc.simulator.is_active(), "Simulator should NOT be active on code mismatch");
    assert!(!doc.program.rungs[0].is_energized, "Rung should NOT be energized");
    assert!(!doc.program.rungs[0].elements[0].is_energized, "Contact should NOT be energized");
    assert!(!doc.program.rungs[0].elements[0].is_done, "Contact done flag should be cleared");
    assert!(!doc.program.rungs[0].elements[1].is_energized, "Coil should NOT be energized");
    assert!(!doc.program.rungs[0].elements[1].is_done, "Coil done flag should be cleared");
}

#[test]
fn test_document_force_io_and_write_registers() {
    let mut doc = Document::default();
    // Program: LD %IX0.0; ST %QX0.0
    let il = "LD %IX0.0\nST %QX0.0\n";
    doc.program = IlParser::parse(il).expect("Parse IL failed");

    // Start simulation
    doc.simulator.start(&mut doc.program);

    // Initial state: X0 is false -> Y0 is false
    assert!(!doc.simulator.get_var_value("%QX0.0"));
    assert!(!doc.simulator.get_var_value("%IX0.0"));

    // 1. Force %IX0.0 ON (addr 0x00)
    doc.force_bit(0, true);
    assert_eq!(doc.is_bit_forced(0), Some(true));
    doc.simulator.step_scan(&mut doc.program, 10);
    assert!(doc.simulator.get_var_value("%QX0.0"), "Y0 should be ON when X0 is forced ON");

    // 2. Force %QX0.0 OFF (addr 0x20)
    doc.force_bit(0x20, false);
    assert_eq!(doc.is_bit_forced(0x20), Some(false));
    doc.simulator.step_scan(&mut doc.program, 10);
    assert!(!doc.simulator.get_var_value("%QX0.0"), "Y0 should be OFF when forced OFF");

    // 3. Write Word Register %MW5 = 12345
    doc.write_word_register(5, 12345);
    assert_eq!(doc.simulator.registers.get("%MW5").copied(), Some(12345));

    // 4. Unforce %QX0.0
    doc.unforce_bit(0x20);
    assert_eq!(doc.is_bit_forced(0x20), None);
    doc.simulator.step_scan(&mut doc.program, 10);
    assert!(doc.simulator.get_var_value("%QX0.0"), "Y0 should be ON again after unforcing");

    // 5. Clear all forces
    doc.clear_all_forces();
    assert_eq!(doc.is_bit_forced(0), None);
}

#[test]
fn test_special_system_markers_and_registers() {
    use my_canvas_core::bytecode::{BytecodeCompiler, MemoryRegion};

    // 1. Verify Compiler parses %SM0..%SM31, M8000..M8031, %SD0..%SD31, D8000..D8031
    let (addr, canon, region) = BytecodeCompiler::parse_direct_address("%SM0", false).unwrap();
    assert_eq!(addr, 0xA0);
    assert_eq!(canon, "SM0");
    assert_eq!(region, MemoryRegion::InternalRelayM);

    let (addr, _, _) = BytecodeCompiler::parse_direct_address("M8000", false).unwrap();
    assert_eq!(addr, 0xA0);

    let (addr, canon, _) = BytecodeCompiler::parse_direct_address("%SM2", false).unwrap();
    assert_eq!(addr, 0xA2);
    assert_eq!(canon, "SM2");

    let (addr, canon, region) = BytecodeCompiler::parse_direct_address("%SD0", true).unwrap();
    assert_eq!(addr, 0xA0);
    assert_eq!(canon, "SD0");
    assert_eq!(region, MemoryRegion::DataRegisterD);

    let (addr, _, _) = BytecodeCompiler::parse_direct_address("D8000", true).unwrap();
    assert_eq!(addr, 0xA0);

    // 2. Ladder Logic:
    // Rung 1: LD %SM0 -> ST %QX0.0 (Always ON)
    // Rung 2: LD %SM2 -> ST %QX0.1 (First scan pulse only)
    let il = "LD %SM0\nST %QX0.0\nLD %SM2\nST %QX0.1\n";
    let mut doc = Document::default();
    doc.program = IlParser::parse(il).expect("Parse IL failed");

    // Scan cycle 1: First scan
    doc.simulator.step_scan(&mut doc.program, 10);
    assert!(doc.simulator.get_var_value("%SM0"), "%SM0 must be Always ON");
    assert!(doc.simulator.get_var_value("M8000"), "M8000 must alias %SM0");
    assert!(doc.simulator.get_var_value("%SM2"), "%SM2 must be true on First Scan");
    assert!(doc.simulator.get_var_value("%QX0.0"), "Y0 must be energised by %SM0");
    assert!(doc.simulator.get_var_value("%QX0.1"), "Y1 must be energised by %SM2 on first scan");

    // Special registers on Scan 1
    assert_eq!(doc.simulator.registers.get("%SD0").copied(), Some(100)); // Watchdog preset
    assert_eq!(doc.simulator.registers.get("%SD4").copied(), Some(1));   // Scan count = 1

    // Scan cycle 2: Second scan
    doc.simulator.step_scan(&mut doc.program, 10);
    assert!(doc.simulator.get_var_value("%SM0"), "%SM0 remains Always ON");
    assert!(!doc.simulator.get_var_value("%SM2"), "%SM2 must be false after first scan");
    assert!(doc.simulator.get_var_value("%QX0.0"), "Y0 remains ON");
    assert!(!doc.simulator.get_var_value("%QX0.1"), "Y1 must turn OFF after first scan");
    assert_eq!(doc.simulator.registers.get("%SD4").copied(), Some(2));   // Scan count = 2
}



