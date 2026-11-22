---
title: "OpenPLC Ladder Diagram — Drawing & Validation Rules for AI"
document_id: "OPENPLC-LD-AI-RULES"
version: "1.3.0"
language: "th"
status: "proposed-project-specification"
last_updated: "2026-11-21"
intended_audience:
  - "AI coding agents"
  - "Ladder Editor developers"
  - "PLC logic reviewers"
rule_profile: "STRICT_EDITOR"
target_platform: "OpenPLC Editor / OpenPLC Runtime (verify installed versions)"
reference_standard: "IEC 61131-3:2025 (Edition 4) — LD language; actual OpenPLC implementation support is version-dependent"
standards_context:
  - "IEC 61131-3:2025 Ladder Diagram (LD) — reference, not a certification of OpenPLC/this editor"
  - "IEC 61131-10:2019 XML exchange format — optional for interoperable project export"
vendor_specific_validation_required: true
---

# OpenPLC — กฎการวาดและตรวจสอบ Ladder Diagram สำหรับ AI

> **วัตถุประสงค์:** ใช้เป็นเอกสารอ้างอิงสำหรับ AI ที่สร้าง/แก้ไข Ladder Diagram, พัฒนา Ladder Editor ที่มีเป้าหมาย OpenPLC, ออกแบบระบบ Validation หรือช่วยตรวจโครงสร้าง Ladder Logic
>
> **สถานะ:** เป็น **ข้อกำหนดของโปรเจกต์ (Project Policy)** โดยใช้โหมด `STRICT_EDITOR` **ไม่ใช่ข้อกำหนดบังคับทั้งหมดของ IEC 61131-3 หรือ OpenPLC ทุกเวอร์ชัน** ความสามารถจริงขึ้นกับ OpenPLC Editor/Runtime/Compiler และเวอร์ชันที่ใช้งาน
>
> **ชื่อมาตรฐานที่ถูกต้อง:** Ladder Diagram ใช้มาตรฐาน **IEC 61131-3** (ไม่ใช่ IEEE) ปัจจุบัน IEC เผยแพร่ฉบับ **IEC 61131-3:2025 Edition 4** แล้ว แต่ไม่ควรสรุปว่า OpenPLC ทุกเวอร์ชันรองรับฟีเจอร์ฉบับนี้ทั้งหมด

## 1. บทบาทและลำดับความสำคัญของคำสั่งสำหรับ AI

AI ที่ใช้เอกสารนี้ **ต้อง**:

1. ใช้รหัสกฎ (`R01`–`R42`) เพื่อสร้าง ตรวจสอบ และรายงานผลของ Ladder Diagram
2. แยก **Drawing/Topology Validation** (โครงสร้างการวาด) ออกจาก **Logic/Semantic Validation** (พฤติกรรมและความหมาย)
3. ประเมินจาก **การเชื่อมต่อจริงของโหนด/สาย (graph topology)** ไม่ใช่ดูเฉพาะพิกัดสัญลักษณ์บนหน้าจอ
4. รายงานรหัสกฎ ระดับความรุนแรง ตำแหน่ง ข้อความปัญหา และแนวทางแก้ไข
5. หากข้อมูลที่จำเป็นยังไม่ทราบ (เช่น target PLC, ชนิด Tag, รูปแบบ Function Block) ให้รายงานว่า **ตรวจสอบไม่ได้/ต้องยืนยัน** แทนการเดา
6. หากข้อกำหนดโปรเจกต์นี้ขัดกับ Vendor Profile ที่ผู้ใช้ระบุ ให้ **แจ้งความขัดแย้ง** และใช้ข้อกำหนดตามโหมดที่ตั้งค่าไว้ ห้ามอ้างว่ากฎโปรเจกต์เป็นกฎสากล
7. ไม่ถือว่าการผ่าน Drawing Validation แปลว่า Logic ปลอดภัยหรือพร้อมดาวน์โหลดเข้า PLC จริง

### 1.1 ค่าตั้งต้นของโปรเจกต์

```yaml
ladder_editor_policy:
  mode: STRICT_EDITOR
  direction: LEFT_TO_RIGHT
  contact_in_series: true
  contact_in_parallel: true
  nested_branches: true
  parallel_coils: true
  coil_must_terminate_branch: true
  coil_in_series: false
  coil_followed_by_downstream_instruction: false
  coil_in_rejoining_logic_branch: false
  bypass_path_without_output_in_contact_coil_network: false
  contact_after_coil: false
  branch_after_coil: false
  dangling_wires: false
  disconnected_elements: false
  cycles_or_backward_logic_paths: false
  duplicate_normal_output_coil: WARNING
  set_reset_on_same_tag: WARNING
  network_execution_order: TOP_TO_BOTTOM
  validation_required_before_compile: true
  allow_incomplete_drafts: true
  validate_during_edit: true
  prevent_invalid_connection_commit: true
  allow_save_when_invalid: true
  block_generate_compile_export_on_error: true
  graph_crossings_require_explicit_junction: true
  multiwriter_physical_output: ERROR
  multiwriter_internal_boolean: WARNING
  require_symbol_table: true
  validate_iec_xml_export_and_roundtrip: true
  vendor: OpenPLC
  editor_version: UNCONFIRMED
  runtime_version: UNCONFIRMED
  compiler_version: UNCONFIRMED
  openplc_compatibility_check_required: true
```

**ศัพท์:** `Contact` = หน้าสัมผัส (เช่น NO/NC), `Coil` = คำสั่งเอาต์พุตบูลีนแบบปกติ, `Branch` = แขนงขนาน, `Network/Rung` = กลุ่มลอจิก, `Tag` = ตัวแปรหรือที่อยู่ I/O, `Terminal Output` = คำสั่งปลายทางที่กฎของ target ยอมรับ

## 2. Rule Catalog (กฎหลัก R01–R42)

| ID | หมวด | กฎสำหรับ `STRICT_EDITOR` | ระดับเริ่มต้น |
|---|---|---|---|
| **R01** | Direction | การวาดและเส้นทางลอจิกหลักดำเนินจากซ้ายไปขวา | ERROR |
| **R02** | Network | Network ที่ไม่ว่างต้องมีเส้นทางลอจิกสมบูรณ์จากแหล่งจ่ายลอจิกไปยังคำสั่งปลายทางที่ได้รับอนุญาต | ERROR |
| **R03** | Contact | อนุญาตให้ Contact ต่ออนุกรม (AND) | ALLOW |
| **R04** | Contact | อนุญาตให้ Contact ต่อขนาน (OR) | ALLOW |
| **R05** | Contact | อนุญาตให้ผสมอนุกรม/ขนานและใช้แขนงซ้อน เมื่อการเชื่อมต่อถูกต้อง | ALLOW |
| **R06** | Coil | Coil ปกติต้องเป็นคำสั่งสุดท้ายของ **แขนงที่มี Coil นั้น** | ERROR |
| **R07** | Coil | Coil หลายตัวต่อขนานได้ **เฉพาะในแขนง Output ที่แยกกันและไม่ต่อผ่านกัน** | ALLOW |
| **R08** | Coil | ห้าม Coil ปกติสองตัวหรือมากกว่าต่ออนุกรมบนเส้นทางเดียวกัน | ERROR |
| **R09** | Coil | ห้าม Contact หรือคำสั่งที่ต้องส่งลอจิกต่อ อยู่หลัง Coil บนเส้นทางเดียวกัน | ERROR |
| **R10** | Coil | ห้ามแตก Branch จากปลายด้านออกของ Coil | ERROR |
| **R11** | Connectivity | ห้ามสายปลายเปิด สายลอย หรือองค์ประกอบที่ไม่เชื่อมต่อใน Network ที่จะ Compile | ERROR |
| **R12** | Branch | จุดเริ่มต้น/สิ้นสุดแขนงต้องเชื่อมต่อเป็นเส้นทางที่สมบูรณ์ | ERROR |
| **R13** | Topology | ห้าม **wire topology cycle**; การใช้ Contact อ้างอิงค่า Coil เดิมแบบ Self-hold ไม่ถือเป็น wire cycle | ERROR |
| **R14** | Logic Quality | ตรวจพบสาย Bypass ข้ามเงื่อนไขที่อาจไม่ตั้งใจ (ต้องพิจารณาเจตนาผู้ออกแบบ) | WARNING |
| **R15** | Output | ตรวจ Coil ปกติที่เขียน Tag เดียวกันซ้ำในหลายตำแหน่ง | WARNING |
| **R16** | Data Type | Contact และ Coil ปกติต้องเชื่อมกับตัวแปร/ที่อยู่ซึ่งรองรับชนิด BOOL ตาม target | ERROR |
| **R17** | Function Block | ต้องตรวจจำนวนขา ชนิดข้อมูล และพารามิเตอร์บังคับตาม signature ของคำสั่ง/บล็อก | ERROR / UNKNOWN |
| **R18** | Set/Reset | อนุญาต Set/Reset แต่ให้เตือนเมื่อเขียน Tag เดียวกันโดยมีเงื่อนไขหรือลำดับที่อาจขัดแย้ง | WARNING |
| **R19** | Execution | ต้องมีลำดับประมวลผล Network ที่แน่นอนตามโปรไฟล์ (ค่าเริ่มต้นบนลงล่าง) | ERROR / UNKNOWN |
| **R20** | Gate | ต้องผ่าน ERROR ทั้งหมดก่อน Compile/Generate/Download; แต่อนุญาตให้ Save Draft ที่ยังมี Error ได้ | ERROR |
| **R21** | Coil / Branch Topology | **ห้ามวาง Coil อยู่กลางเส้นทางที่มีการรวมแขนงแล้วไหลต่อไปยังคำสั่งอื่น**; หลัง Coil อนุญาตเฉพาะสายไปถึง Right Rail หรือ Output Bus ปลายทางที่ไม่มีคำสั่งต่อท้าย | ERROR |
| **R22** | Wire Crossing | เส้นสายตัดกันไม่ถือว่าเชื่อมต่อโดยอัตโนมัติ; ต้องมี Junction/Connection Node ที่ชัดเจน | ERROR ถ้าเกิดการเชื่อมผิด — P0 |
| **R23** | Port Compatibility | ห้ามเชื่อม Port ผิดประเภท/ทิศทาง หรือป้อนสัญญาณหลายแหล่งเข้าขาที่รับได้เพียงหนึ่งแหล่ง | ERROR — P0 |
| **R24** | Branch Integrity | จุด Split/Merge ต้องอ้างอิง Node และ Edge ที่มีอยู่จริง; ไม่มีสายภาพซ้อนที่ขาดการเชื่อมทางข้อมูล | ERROR — P0 |
| **R25** | Output Bypass | Network แบบ Contact–Normal Coil ต้องไม่มีเส้นทางสมบูรณ์จาก Left Rail ถึง Right Rail ที่ข้าม Coil ปลายทางทั้งหมด | ERROR — P0 |
| **R26** | Write Permission | ห้ามเขียนค่าไปยัง Read-only VAR_INPUT หรือ Physical Input ที่ Target ไม่อนุญาตให้เขียน | ERROR — P0 |
| **R27** | I/O Location | ตรวจตำแหน่ง Physical I/O ซ้ำหรือช่วง Address ทับซ้อน โดยแยก Alias ที่ตั้งใจออกจากการขับ Output ชนกัน | ERROR/WARNING — P0 |
| **R28** | FB Instance & Signature | ตรวจว่า Function Block มี Instance ที่ประกาศแล้ว, พอร์ตถูกชนิด, Required Parameter ครบ และทางเชื่อมเป็นไปตาม Signature | ERROR/UNKNOWN — P0 |
| **R29** | Multiple Writers | วิเคราะห์การเขียน Tag เดียวกันข้าม Coil/POU/Task/ภาษา; Physical Output ที่มีผู้เขียนหลายจุดให้ Block ใน STRICT_EDITOR | ERROR/WARNING — P0 |
| **R30** | Edit Transactions | Move/Delete/Paste/Undo/Redo/Auto-route ต้องคง Graph Integrity; การเชื่อมที่ผิดต้องไม่ Commit | ERROR เมื่อ Graph ผิด — P0 |
| **R31** | Symbol Resolution | ตรวจ Undefined Tag, Duplicate Declaration, Scope, Reserved Name และ Reference ที่ชี้ไปยังตัวแปร/สมาชิกที่ไม่มีอยู่ | ERROR — P0 |
| **R32** | Timer/Counter Configuration | ตรวจ TON/TOF/TP, CTU/CTD/CTUD ตาม Signature ของ Target, ชนิด PT/PV, Instance State และ Value Range | ERROR/UNKNOWN — P1 |
| **R33** | Coil Variants | แยกกฎของ Normal/Negated/Set/Reset/Edge Coil; ห้ามแปลงชนิดโดยรักษาแค่ภาพสัญลักษณ์ | ERROR/UNKNOWN — P1 |
| **R34** | Feedback Semantics | แยกการอ้างอิง Tag ตัวเองผ่าน Contact (เช่น Self-hold) จากสาย Graph ที่ย้อนกลับจนเป็น Cycle | ALLOW/WARNING — P0 |
| **R35** | Task & Scan Order | ตรวจ Task/Program Instance ที่รันจริง, ลำดับการประมวลผล, Shared-State และการเขียนข้าม Task | ERROR/WARNING/UNKNOWN — P1 |
| **R36** | XML & Code Generation | เมื่อต้อง Export ให้ตรวจ Schema/Local IDs/References/Port Connections และทดสอบ Import–Export Round-trip | ERROR/UNKNOWN — P1 |
| **R37** | Draft vs Compile | เก็บแบบร่างที่ยังไม่สมบูรณ์ได้ แต่ห้าม Compile/Deploy หากมี ERROR หรือ UNKNOWN ที่จำเป็นต่อ Target | POLICY — P0 |
| **R38** | Complexity Limits | จำกัดความซับซ้อนเพื่อความอ่านง่าย เช่นเกิน 32 Elements/Rung แจ้ง WARNING ตาม Policy ไม่ใช่ข้อห้ามสากล | WARNING — P2 |
| **R39** | Semantic-Preserving Edits | Auto-fix/Auto-layout ห้ามเปลี่ยน AND/OR, Coil Type, Tag, Priorities หรือ Scan Order โดยไม่ยืนยัน | ERROR/REVIEW — P0 |
| **R40** | Safety Review | สถานะ PASS ของ Drawing/Compiler ไม่ใช่หลักฐานรับรอง E-Stop, Guard, Interlock หรือ Fail-safe | REQUIRE_REVIEW — P1 |
| **R41** | Target Capability | หาก Version/Instruction/FB/Pins ไม่ทราบ ให้ UNKNOWN; ห้าม AI สมมติว่า Export ได้หรือรองรับอัตโนมัติ | UNKNOWN/BLOCK — P0 |
| **R42** | Test Evidence | ทุกกฎบังคับควรมี PASS/FAIL Test, mutation tests และผล Compile จริงสำหรับ Target ที่อ้างว่ารองรับ | RELEASE_GATE — P1 |

### R21 — Coil ต้องเป็น Terminal Output ห้าม Coil อยู่ก่อนจุดรวมแขนงที่มี Logic ต่อท้าย

```yaml
rule_id: R21
name: COIL_MUST_NOT_FEED_DOWNSTREAM_LOGIC
severity: ERROR
scope: NORMAL_COIL
mode: STRICT_EDITOR
reject_when:
  - "มี Contact, Coil หรือคำสั่ง Logic ใด ๆ ที่เข้าถึงได้จาก coil.output โดยไม่ผ่าน Right Rail"
  - "Coil อยู่ก่อนจุดรวมแขนง (rejoin) และหลังจุดรวมนั้นยังมีคำสั่ง Logic อยู่"
allow_when:
  - "coil.output ต่อสายตรงถึง Right Rail"
  - "coil.output ต่อผ่าน Output Bus/จุดรวมปลายทางที่ไม่มี Logic instruction ต่อท้าย แล้วถึง Right Rail"
validation_method: DIRECTED_GRAPH_REACHABILITY
error_code: COIL_BEFORE_REJOIN_WITH_DOWNSTREAM_LOGIC
message_th: "Coil ต้องอยู่ปลายแขนง: พบการรวมแขนงหลัง Coil แล้วมีคำสั่งต่อท้าย"
compile_policy: BLOCK
```

**ขั้นตอนตรวจสอบสำหรับ AI/Validator**

1. สร้าง Directed Graph จาก Input → Contact → Junction → Coil → Right Rail ตามสายจริง ไม่พิจารณาจากตำแหน่งภาพอย่างเดียว
2. เริ่มจากพอร์ต **output ของ Coil ทุกตัว** แล้วเดินตามเส้นสายไปข้างหน้า โดยข้าม Junction และ Wire ได้
3. หากพบ Instruction ชนิดใดก็ตามก่อนถึง Right Rail ให้ `FAIL R21` (ถ้าเป็น Coil อีกตัว ให้รายงาน R08 เพิ่มได้)
4. หากพบเพียง Wire/Junction → Right Rail และไม่มีคำสั่งต่อท้าย ให้ผ่าน; Coil หลายตัวอาจใช้ Terminal Output Bus ร่วมกันได้ตามกฎ R07
5. สำหรับวงจรที่มีแต่ Contact/Coil ให้ตรวจทุกเส้นทางจาก Left Rail ถึง Right Rail ว่าจบด้วย Coil เพียงตัวเดียว ไม่ผ่าน Coil ซ้ำหรือ bypass Coil ทั้งหมด
6. บังคับกฎนี้ระหว่าง `onConnect`, `onMove`, `onBranchMerge`, `onPaste`, `Validate`, และ `Compile`; ป้องกันการเชื่อมต่อผิดตั้งแต่ก่อนบันทึก

**สำคัญ:** จุดรวม Branch ไม่ผิดด้วยตัวมันเอง; จะผิดเมื่อ **มี Coil ก่อนจุดรวม และมี Logic Instruction ต่อท้ายจุดรวมนั้น** การวาง Coil ขนานที่ต่างแขนงกันและต่อถึง Right Rail โดยไม่มีคำสั่งต่อท้าย **ยังอนุญาต**

**หมายเหตุสำคัญ:** `ALLOW` หมายถึงรองรับรูปแบบนั้น **เมื่อกฎที่เกี่ยวข้องทั้งหมดผ่าน** ไม่ได้หมายความว่าตรรกะของวงจรปลอดภัยหรือถูกต้องตามความต้องการใช้งาน

## 2.2 รายละเอียดบังคับของ Validator รุ่น 1.3 (R22–R42)

> **สถานะ:** กฎใหม่เป็น *นโยบายของ Editor เป้าหมาย* ที่จัดทำเพิ่มเติมโดยโครงการ ไม่ใช่การอ้างว่ากฎทั้งหมดเป็นข้อบังคับของ IEC 61131-3. ความแตกต่างเชิงเครื่องมือจะตรวจตาม `target_profile` ที่ยืนยันแล้วเท่านั้น

### 2.2.1 Wiring & Branch — R22–R25

**R22: Crossing != Connection.** หากสายสองเส้นตัดกันทางเรขาคณิต แต่ไม่มี Graph Node แบบ `JUNCTION` หรือการผูก port/endpoint จริง ห้ามสร้างการเชื่อมทางไฟฟ้าลอจิกจากการทับภาพเพียงอย่างเดียว หากการตัดกันเป็นแบบตั้งใจไม่เชื่อม ให้แสดงด้วยรูปแบบที่แยกได้ (เช่น ไม่มีจุดทึบ); ไม่ต้อง Error เพราะเส้นตัดกันเฉย ๆ หากไม่มี Graph Error

**R23: Typed Port Compatibility.** พอร์ต Logic Bool, พอร์ตข้อมูล INT/TIME/REAL, พอร์ต `VAR_IN_OUT` และพอร์ต `EN/ENO` (ถ้ามี) ต้องตรวจชนิด/ทิศทาง/จำนวนการต่อสายตาม Signature; อย่าใช้กฎรวมแบบ 'ทุกพอร์ตรับสองสายได้'. Branch Junction ที่รวม Bool Power Flow สามารถทำ OR ได้ **เมื่อ** target อนุญาต ไม่ใช่การป้อนสายหลายเส้นตรงเข้าพอร์ตข้อมูล FB โดยพลการ

**R24: Graph-backed Junction.** สำหรับแต่ละ Split/Merge ให้ตรวจว่า `node_id` ที่ทุก edge อ้างอิงมีอยู่จริง, ไม่มี edge ไปหา port ที่ถูกลบ, และไม่มี junction โดดเดี่ยว; สำหรับการวาดที่เกิดจากการลากผ่านสาย ต้องสร้าง junction จริงหรือยกเลิกการเชื่อม

**R25: No Coil-Free Bypass.** จำกัดเฉพาะ Network ที่ประกอบด้วย `Contact + Normal Coil` และกำหนดให้ทุกเส้นทางจาก Left Rail ถึง Right Rail ผ่าน `exactly 1` Coil ปลายทาง (กฎ R06–R09/R21). Network ที่ใช้ FB หรือ instruction ปลายทางพิเศษต้องใช้ `target_profile` ไม่ใช่บังคับ Coil โดยอัตโนมัติ

### 2.2.2 Variables, FB & Writes — R26–R33

**R26:** ตรวจ `writable` จาก symbol class + mapping + target. ตัวแปร `VAR_INPUT` ภายใน POU โดยทั่วไปต้องถือ Read-only; Physical Input `%IX...` ต้องไม่ถูกปฏิบัติเป็น Coil Output สำหรับโปรไฟล์ที่ห้ามเขียนข้อมูลอินพุต. **ห้ามอนุมานว่าตัวแปรที่แมป `%QX` ต้องเป็น Class=Output** เพราะใน OpenPLC มีตัวอย่าง Class=Local ที่ตั้ง Location เป็น `%QX0.0` ได้

**R27:** ตรวจการใช้ `%IX/%QX/%IW/%QW/%MX/...` ตามขนาดข้อมูล, alignment และแผนที่ target จริง. ตรวจการอ้างอิงที่อยู่อย่างทับซ้อน เช่น `%QX0.0` กับ `%QB0`/`%QW0` โดยไม่ถือว่าผิดทั้งหมดหากมี alias ที่ตั้งใจและ target รองรับ; แต่ถ้าผู้เขียนหลายจุดไปยัง Output เดียวกันและไม่มี owner ชัดเจน ให้ ERROR ใน `STRICT_EDITOR`

**R28:** ตรวจการประกาศ FB Instance (ตัวอย่าง `TON0 : TON`), signature ของ FB, required inputs, member access (`TON0.Q`, `TON0.ET`) และดาตาไทป์. อย่าบังคับพอร์ต optional ทุกขาให้ต่อครบ; ใช้ metadata ที่ยืนยันแล้ว

**R29:** รวบรวมรายการผู้เขียนตัวแปรจาก Coil, Set/Reset, POU, ST, FB output mapping และ I/O alias. ห้ามนับเฉพาะ normal coil เพื่อป้องกัน silent overwrite. สำหรับ Physical Output ที่มีหลาย writers ตั้ง `ERROR` ใน strict mode; internal state อาจ `WARNING` เมื่อชัดเจนว่าเป็น design intent แต่ต้องรายงานลำดับการเขียนและโปรไฟล์ Scan

**R31:** Symbol name ต้อง resolve ได้จริงตาม Scope/Namespace รวมทั้ง case rules และ reserved keywords ของ compiler; บล็อกการอ้างอิง Tag ที่ไม่มีอยู่หรือชื่อ instance เดียวกันชนกัน

**R32:** สำหรับ TON/TOF/TP ตรวจ `IN:BOOL`, `PT:TIME` และ `Q/ET` ตามชนิดที่ target กำหนด; CTU/CTD/CTUD ตรวจ `CU/CD/R/LD/PV/CV/Q` ตาม signature จริง. PT=0/negative/เกินช่วง ให้ ERROR เฉพาะกรณี target ห้าม มิฉะนั้น WARNING และทดสอบพฤติกรรม อย่ากำหนดกฎค่าคงที่แบบเดาสุ่ม

**R33:** Normal, negated, Set, Reset, rising/falling-edge coil อาจมี Semantics และ state ต่างกัน; หากระบบไม่รองรับให้ `UNSUPPORTED_TARGET`, อย่าเปลี่ยนเป็น Normal Coil โดยอัตโนมัติ. ต้องทดสอบ latch/unlatch priority เฉพาะ target

### 2.2.3 Editor Integrity & Execution — R30, R34–R42

**R30: Transactional Edits.** ทุกคำสั่ง `onConnect`, `onDisconnect`, `onMove`, `onDelete`, `onPaste`, `onDuplicate`, `onUndo`, `onRedo`, `autoRoute` ต้องทำงานบน temporary graph, Validate, แล้วจึง commit เมื่อเป็น Edit ที่กำหนดให้ถูกต้อง; ถ้าเกิด ERROR ที่เป็นผลจากคำสั่งนั้น ให้ Reject โดยไม่ทำให้ Graph เดิมเสีย. `undo/redo` ต้องได้ state เดิมครบทั้ง node, edge, port, and IDs. สำหรับ Draft ที่มี ERROR อยู่ก่อน อนุญาตแก้ไข/ลบ/undo เพื่อกู้คืนได้ ไม่ล็อกทุกการแก้ไข

**R34: Self-hold is not a wire cycle.** ตัวอย่าง `START || MOTOR_CONTACT` ต่อร่วมกับ `STOP` ไป Coil `MOTOR`: Contact อ่าน Tag `MOTOR` ที่ Coil เขียน เป็น *data/state dependency across scans* ไม่ใช่วงจร Wire ที่ย้อนกลับจาก output port ไป input port. ต้อง PASS หาก topology ถูกต้อง; ห้ามส่ง R13 เพียงเพราะ Tag อ่าน-เขียนตัวเอง

**R35:** ตรวจความสอดคล้องของ Task, Program instance, scheduling, priority, scan timing และ Ownership ของ shared/global variables. `top-to-bottom` เพียงอย่างเดียวไม่อธิบายโปรแกรมหลาย Tasks; ถ้าพิสูจน์ลำดับไม่ได้ ให้ `UNKNOWN` และทำ concurrency review

**R36:** หากส่งออก PLCopen XML / IEC 61131-10 ให้ตรวจ XML schema ที่ target เลือกใช้, local IDs ไม่ซ้ำใน scope, `refLocalId` ชี้เป้าหมายจริง, port connection ครบ, semantic consistency และ **round-trip** (export→import→compile). ไม่อ้างว่าส่งออกสำเร็จแปลว่าเข้ากันได้โดยอัตโนมัติ

**R37:** `EDIT/DRAFT` อนุญาตให้บันทึก Network ว่างหรือเส้นค้างได้พร้อม Error markers; `COMPILE/GENERATE/DEPLOY` ต้อง Block เมื่อมี `ERROR` หรือ `UNKNOWN` ที่ส่งผลต่อการสร้างโปรแกรมบน Target. ห้าม AI แก้วงจรแทนผู้ใช้โดยไม่ได้ยืนยัน semantic change

**R38:** เตือนเมื่อ Rung มีองค์ประกอบมากเกิน threshold ที่กำหนด เช่น 32 ตามตัวอย่าง PLCopen Coding Guidelines L6. สามารถกำหนด threshold/disabled ตามทีม และเป็น `WARNING` เริ่มต้น ไม่ใช่ Error จาก IEC

**R39:** ทุก Auto-layout/Auto-fix ต้องรักษาความหมายเดิมของ Logic. การขยับ Branch Split ผ่าน Contact จะเปลี่ยน AND/OR ได้; การแยก Coil กลาง Network เป็นสอง Network อาจเปลี่ยน scan order. หากพิสูจน์ semantic equivalence ไม่ได้ ต้องขอการยืนยันก่อนเปลี่ยน

**R40:** สำหรับ Emergency Stop, Interlock, Guard, machine protection และอุปกรณ์อันตราย ให้แสดง `REQUIRES_SAFETY_REVIEW`; ห้ามบอกว่า validation ผ่าน = ได้รับการรับรองความปลอดภัย และห้าม deploy จริงอัตโนมัติ

**R41:** Target Support Matrix กำหนด `SUPPORTED`, `UNSUPPORTED`, `UNVERIFIED` สำหรับ coil variants, branch topology, FB pins, XML และ compiler behavior โดยยึด Version จริง; สถานะ `UNVERIFIED` ต้องไม่แอบผ่านขั้น Generate เมื่อ feature นั้นจำเป็น

**R42:** เพิ่ม Unit Tests (Graph), Mutation Tests (แก้สาย/ย้าย element), Property-based Tests (valid graph invariants) และ golden tests (compiler result) โดยอ้างผลจริงตาม target/version. กฎที่ไม่มีหลักฐาน test ห้ามถือว่าทดสอบผ่าน

### 2.2.4 Source of Truth สำหรับ AI

```yaml
validation_pipeline:
  - parse_editor_document
  - validate_schema_and_identifiers
  - build_typed_port_graph
  - validate_endpoints_junctions_and_wire_crossings
  - detect_wire_cycles_and_unreachable_nodes
  - validate_normal_coil_terminal_property
  - validate_no_coil_free_bypass_for_contact_coil_network
  - resolve_symbols_scopes_read_write_permissions
  - validate_io_locations_and_overlaps
  - validate_function_block_signatures_and_instances
  - analyze_variable_writers_across_pous_tasks
  - validate_target_features_and_export_format
  - run_compiler_and_simulation_tests_if_available
  - emit_diagnostics_and_evidence
phase_policy:
  EDIT:
    allow_invalid_draft_save: true
    reject_new_invalid_connection: true
    allow_recovery_actions_from_invalid_draft: true
  COMPILE:
    block_if_error: true
    block_if_required_target_feature_unverified: true
  DEPLOY:
    requires_explicit_user_action: true
    requires_target_compiler_success: true
    requires_hardware_safety_review_if_applicable: true
severity_order:
  - ERROR
  - UNKNOWN
  - WARNING
  - INFO
```

### 2.2.5 ข้อควรระวังเกี่ยวกับกฎเดิม

- R14 ตรวจ bypass เป็นเพียง `WARNING` ของคุณภาพลอจิก ไม่ใช่ยืนยันว่า short circuit ผิดทุกกรณี; แต่ R25 เป็น `ERROR` ในเงื่อนไขที่แคบกว่า (Contact–Normal Coil Network ที่มี power path ไป Right Rail โดยไม่ผ่าน terminal output)
- R15 เดิมเป็น `WARNING` สำหรับ duplicate normal coil; R29 ใหม่เพิ่มการตรวจ **ตัวแปรเดียวกันถูกเขียนจากหลายกลไก** และยกระดับเป็น `ERROR` สำหรับ physical output ที่มี multiwriter ภายใต้ strict policy
- R17 ต้องตรวจเฉพาะ **Required** Inputs ตาม signature จริง ไม่ใช่บังคับต่อทุกพอร์ตโดยอัตโนมัติ
- R19 ไม่ใช่ global scan order ที่ยืนยันได้ทุก target: R35 เพิ่ม Task/POU scheduling
- R20 ไม่ควรห้าม Save Draft ที่มี Error; R37 แยก `EDIT` จาก `COMPILE`/`DEPLOY`
- R21/R25 ใช้กับ Normal Coil เป็นหลัก; Function Block/EN/ENO และ Coil variants มีข้อยกเว้นที่ต้องยืนยันจาก target

## 3. ตัวอย่าง Ladder Diagram (ASCII)

สัญลักษณ์: `[ A ]` = NO Contact, `[/A]` = NC Contact, `( Q1 )` = Normal Coil, `+` = จุดแยก/รวมแขนง; เส้น `-` = การเชื่อมต่อ ตัวอย่างนี้เป็น schematic ไม่ใช่ไฟล์โค้ดที่ Compile ได้โดยตรง

### TC01 — Contact ต่ออนุกรม: PASS

```text
|----[ A ]----[ B ]----------------( Q1 )----|
```

ความหมาย: `Q1 = A AND B`  
กฎที่เกี่ยวข้อง: R03, R06

### TC02 — Contact ต่อขนาน: PASS

```text
|----+----[ A ]----+--------------( Q1 )----|
     |             |
     +----[ B ]----+
```

ความหมาย: `Q1 = A OR B`  
กฎที่เกี่ยวข้อง: R04, R06, R12

### TC03 — Coil ต่อขนาน: PASS (STRICT_EDITOR)

```text
|----[ A ]----+-------------------( Q1 )----|
              |
              +-------------------( Q2 )----|
```

ความหมาย: `Q1 = A`, `Q2 = A`  
**เงื่อนไข:** สอง Coil อยู่ในแขนงเอาต์พุตแยกกัน และไม่มีเส้นทางไหลผ่าน Coil แรกไปยัง Coil ที่สอง (R07)

### TC04 — Coil ต่ออนุกรม: FAIL (R08)

```text
|----[ A ]--------( Q1 )--------( Q2 )------|
```

**ข้อผิดพลาด:** `R08 ERROR — ห้ามต่อ Coil อนุกรมในเส้นทางเดียวกัน`  
**การแก้:** แยกเป็นแขนง Output ขนานหรือคนละ Network

### TC05 — Contact หลัง Coil: FAIL (R09)

```text
|----[ A ]--------( Q1 )--------[ B ]-------|
```

**ข้อผิดพลาด:** `R09 ERROR — Coil ต้องจบแขนง; ห้ามวาง Contact หลัง Coil`  
**การแก้:** ย้าย Contact ไปอยู่ก่อน Coil หรือแยก Network ให้สอดคล้องกับเจตนาลอจิก

### TC06 — Branch ปลายเปิด: FAIL (R11, R12)

```text
|----+----[ A ]------------------( Q1 )----|
     |
     +----[ B ]----     <-- dangling end
```

**ข้อผิดพลาด:** `R11/R12 ERROR — แขนงไม่สมบูรณ์`  
**การแก้:** ต่อแขนงให้ครบตาม topology หรือเอาสายส่วนเกินออก

### TC07 — Coil ขนานโดยแตกแขนงหลัง Coil: FAIL (R10)

```text
|----[ A ]----( Q1 )----+----( Q2 )----|
                        |
                        +----( Q3 )----|
```

**ข้อผิดพลาด:** `R10 ERROR — แตกแขนงจากด้านออกของ Coil`  
**การแก้:** ย้ายจุดแตกแขนงให้อยู่ก่อน Coil ทุกตัว

### TC08 — Contact แบบ NC: PASS

```text
|----[ A ]----[/STOP]------------( MOTOR )----|
```

ความหมาย: `MOTOR = A AND NOT STOP` โดยตีความ `STOP` เป็นตัวแปรลอจิก มิใช่ข้อยืนยันด้านความปลอดภัยของวงจรหยุดเครื่อง

### TC09 — Coil อยู่ในแขนงก่อนจุดรวม แล้วมี Coil อีกตัวหลังจุดรวม: FAIL (R21)

**รูปแบบผิดตรงกับกรณีจากภาพที่ส่งมา:**

```text
|----+----[ %IX0.0 ]----( %QX0.1 )----+----( %QX0.0 )----|
     |                                |
     +----[ %IX0.0 ]------------------+
```

- แขนงบนผ่าน Coil `%QX0.1` แล้วกลับมารวมกับแขนงล่าง
- จากจุดรวม มีเส้นต่อไปยัง Coil `%QX0.0` อีกตัว
- Coil `%QX0.1` จึง **ไม่ใช่ Output ปลายทาง** แต่กลายเป็นองค์ประกอบกลางเส้นทาง (mid-rung coil)
- **ห้ามอนุญาตเพียงเพราะมีแขนงขนาน**: ต้องตรวจว่าปลายด้านออกของ Coil ไปถึงคำสั่งอื่นผ่านเส้นสายหรือจุดรวมแขนงหรือไม่

**ERROR:** `R21 — COIL_BEFORE_REJOIN_WITH_DOWNSTREAM_LOGIC`  
**วิธีแก้:** ย้าย Coil ทั้งสองเป็น Output Branch ที่ปลาย Network โดยแยกแขนงหลังกลุ่ม Contact หรือแยกเป็นคนละ Network ให้ตรงตามตรรกะที่ตั้งใจ

**ตัวอย่างที่ถูกต้อง: Coil เป็น Output Branch แยกกัน (PASS)**

```text
|----+----[ %IX0.0 ]----+----+----( %QX0.1 )----|
     |                  |    |
     +----[ %IX0.0 ]----+    +----( %QX0.0 )----|
```

ในตัวอย่างที่ถูกต้อง สายหลัง Coil แต่ละตัวต่อถึง Right Rail โดยไม่มี Contact, Coil, Function Block หรือจุดรวมที่นำไปสู่คำสั่งอื่น

> **ขอบเขต:** R21 เป็น `STRICT_EDITOR` project policy สำหรับ Coil ปกติ; ไม่ใช้ตัดสิน Output Function Block/คำสั่ง vendor-specific โดยไม่ตรวจ signature ของคำสั่งนั้นก่อน

## 4. วิธีตรวจสอบเชิงโครงสร้าง (Graph Validation)

ใช้โครงสร้างภายในประมาณนี้:

```text
Network = {
  id,
  elements: [{id, kind, tag, input_ports, output_ports, position}],
  wires: [{from_element, from_port, to_element, to_port}],
  target_profile
}
```

**อัลกอริทึมแนะนำ**

1. ตรวจ schema, `id`, พอร์ต และชนิดองค์ประกอบให้ถูกต้อง
2. สร้าง **directed graph** ของการต่อสายจากด้านเข้าไปด้านออกขององค์ประกอบ
3. ตรวจสายปลายเปิด, typed port ที่ผิด, graph-backed junction, crossing, จุดที่ไปไม่ถึงจากแหล่งลอจิก, และองค์ประกอบที่ไม่มีเส้นทางไปยัง terminal ที่ยอมรับ (R02, R11, R12, R22–R24)
4. ตรวจ **wire graph** ย้อนกลับ/กราฟมี cycle (R01, R13) โดยไม่เข้าใจผิดว่า Self-hold ที่ใช้ Contact อ้างอิง Coil เดียวกันคือสายวนกลับ (R34)
5. ตรวจคุณสมบัติ **ทุกเส้นทางจากต้นทางถึงปลายทาง** ที่มี Coil ปกติ ว่าไม่ผ่าน Coil ปกติเกิน 1 ตัว (R08); ในระบบจริงควรตรวจด้วย DAG/reachability หรือ dynamic programming ไม่จำเป็นต้อง enumerate เส้นทางจำนวนมหาศาล
6. สำหรับ **ทุก Coil ปกติ** ตรวจเส้นทางด้านออก `coil.output` โดยเดินผ่าน Wire/Junction ไปทางขวา: อนุญาตให้ถึง Right Rail หรือ Output Bus ปลายทางเท่านั้น; ถ้าพบ Contact/Coil/Function Block อื่น **แม้ผ่านจุดรวม Branch แล้ว** ให้แจ้ง `R21` ทันที; ตรวจข้อห้าม R06, R09, R10 ด้วย
7. หากมี Coil หลายตัว ตรวจว่าเป็น **คนละแขนง Output** โดยไม่ผ่านกันแบบอนุกรม (R07); สำหรับ Network ที่ประกอบด้วย Contact + Coil ปกติเท่านั้น ให้ตรวจว่าทุกเส้นทางลอจิกที่สมบูรณ์ถึง Right Rail ผ่าน Coil ปลายทางหนึ่งตัวพอดี (ไม่ใช่ศูนย์หรือสองตัว)
8. ตรวจ Tag, data type, FB signatures, I/O alias, write permissions, multiwriters, task/scan order (R16–R19, R26–R29, R31–R35)
9. ทำ lint เพื่อแจ้ง bypass, duplicate coil และความซับซ้อนที่ควรทบทวน (R14, R15, R18)
10. อนุญาต Save Draft ที่ไม่สมบูรณ์ แต่ Block Compile/Deploy เมื่อมี `ERROR` หรือ `UNKNOWN` ที่เป็นข้อกำหนดจำเป็นของ Target (R20, R37, R41)

**ข้อควรระวัง:** ห้ามใช้วิธีง่าย ๆ เช่น `coil.x > contact.x` เพียงอย่างเดียวในการตัดสินถูกผิด เพราะตำแหน่งภาพไม่รับประกันการต่อสายจริง; ต้องตรวจ connectivity ด้วย

## 5. Additional Validation / Lint

| Code | การตรวจ | Severity | หมายเหตุ |
|---|---|---|---|
| L01 | Duplicate normal output Coil เขียน Tag เดียวกัน | WARNING | อาจเกิดการเขียนทับตาม scan order |
| L02 | Set และ Reset Tag เดียวกันในหลายจุด | WARNING | วิเคราะห์ลำดับและ precedence ของ target |
| L03 | Bypass ข้ามเงื่อนไขทั้งหมด | WARNING | บางครั้งเป็นการออกแบบ OR/always-true โดยตั้งใจ |
| L04 | Network ว่าง | INFO หรือ ERROR | เป็นนโยบาย Editor; อนุญาต placeholder ได้ในระหว่างแก้ไข |
| L05 | Tag ไม่มีการประกาศ | ERROR | เมื่อระบบจำเป็นต้องใช้ Symbol Table |
| L06 | Tag data type ไม่ตรงกับ instruction | ERROR | ตรวจตาม signature จริง |
| L07 | Function Block ขาด required inputs | ERROR | ตรวจตาม target profile |
| L08 | Function Block ไม่ทราบ signature | UNKNOWN | ห้ามเดาชนิดพอร์ต |
| L09 | แขนงซ้อนหลายชั้นเกินเกณฑ์ทีม | INFO / WARNING | เป็นเรื่อง readability ไม่ใช่ข้อผิดพลาดสากล |
| L10 | ไม่มี comment หรือ Tag name ไม่สื่อความหมาย | INFO | ข้อแนะนำด้านการบำรุงรักษา |
| L11 | Reset/Stop/Safety interlock ไม่ครบตามความต้องการ | REQUIRE_REVIEW | ต้องอาศัย requirement และการประเมินความเสี่ยง |

## 6. รูปแบบรายงาน Validation ที่ AI ควรส่งกลับ

```json
{
  "network_id": "N004",
  "policy": "STRICT_EDITOR",
  "target_vendor": "OpenPLC",
  "editor_version": "UNCONFIRMED",
  "result": "FAIL",
  "issues": [
    {
      "rule_id": "R08",
      "severity": "ERROR",
      "element_ids": ["Q1", "Q2"],
      "path_id": "P01",
      "message_th": "พบ Coil สองตัวต่ออนุกรมในเส้นทางเดียวกัน",
      "suggestion_th": "แยก Coil เป็นแขนง Output ขนานหรือแยก Network"
    }
  ],
  "can_compile": false,
  "requires_vendor_confirmation": true
}
```

**ค่า `result`:** `PASS` (ไม่มี ERROR), `FAIL` (มี ERROR), `UNKNOWN` (ตรวจไม่ครบเพราะข้อมูลไม่พอ). เมื่อมี `UNKNOWN` ที่กระทบการ Compile จริง ให้หยุดและร้องขอข้อมูลที่ขาด แทนการอ้างว่าผ่านแล้ว

## 7. Acceptance Tests ที่ต้องผ่านก่อนปล่อย Validator

| Test ID | กรณีทดสอบ | ผลคาดหวัง |
|---|---|---|
| TC01 | Contact A และ B อนุกรม แล้วจบด้วย Coil Q1 | PASS |
| TC02 | Contact A/B ขนาน แล้วจบด้วย Coil Q1 | PASS |
| TC03 | Contact A ควบคุม Q1/Q2 ที่เป็น Output Branch แยกกัน | PASS |
| TC04 | Q1 และ Q2 ต่ออนุกรม | FAIL R08 |
| TC05 | Contact B ต่อหลัง Q1 | FAIL R09 |
| TC06 | Branch ที่ยังมีสายปลายเปิด | FAIL R11/R12 |
| TC07 | แตกแขนงออกจากด้านหลัง Q1 | FAIL R10 |
| TC08 | Contact NO ตามด้วย Contact NC และ Coil | PASS |
| TC09 | มีวงจรสายย้อนกลับเป็น Cycle | FAIL R13 |
| TC10 | Coil สองตัวเขียน Tag เดียวกันคนละ Network | WARNING R15/L01 |
| TC11 | Contact บังคับอ่าน Tag ชนิด INT โดย target ไม่อนุญาต | FAIL R16 |
| TC12 | Function Block ไม่มี required input | FAIL R17/L07 |
| TC13 | Function Block ไม่ทราบ signature | UNKNOWN L08 |
| TC14 | มีสาย bypass ตัดเงื่อนไข | WARNING R14/L03; ต้องตรวจเจตนา |
| TC15 | องค์ประกอบลอย ไม่เชื่อมกับแหล่งลอจิก | FAIL R11 |
| TC16 | แยกแขนง Contact ซ้อนกันอย่างถูกต้อง แล้วจบ Coil เดียว | PASS |
| TC17 | Coil Q1 อยู่แขนงบนก่อนรวมกับแขนงล่าง แล้วต่อไปยัง Coil Q2 | FAIL R21 (และ R08 ตามเส้นทางที่พบ) |
| TC18 | Contact A/B ขนาน แล้วแตก Output Branch ไป Coil Q1/Q2 ปลายทางโดยไม่มีคำสั่งต่อท้าย Coil | PASS |

### Acceptance Tests เพิ่มเติม (TC19–TC40)

| Test ID | กรณีทดสอบ | ผลที่คาดหวัง |
|---|---|---|
| TC19 | สายไขว้กันที่ไม่มี Junction แต่ Validator เชื่อมให้เอง | FAIL R22 เมื่อเกิด logical false connection |
| TC20 | สายสองเส้นไขว้กันแต่ไม่มี Junction และไม่อ้างว่าเชื่อมต่อ | PASS R22 |
| TC21 | ต่อ TIME Output เข้าขา BOOL โดยไม่มี Conversion ที่ target รองรับ | FAIL R23 |
| TC22 | ย้าย Branch Split แล้ว Wire ชี้ node_id ที่ลบไปแล้ว | FAIL R24/R30 |
| TC23 | Branch ตรงจาก Left Rail ไป Right Rail ข้าม Normal Coil | FAIL R25 (Contact–Coil mode) |
| TC24 | Contact อ่าน Physical Input `%IX0.0` | PASS R26 |
| TC25 | Normal Coil เขียน Physical Input `%IX0.0` ใน target ที่เป็น read-only | FAIL R26 |
| TC26 | ตัวแปรสองตัวเขียน Physical Output `%QX0.0` เดียวกัน | FAIL R27/R29 ใน strict mode |
| TC27 | FB TON0:TON อ่าน TON0.Q ที่ประกาศถูกต้อง | PASS R28/R32 |
| TC28 | ใช้ `TON0.Q` แต่ไม่มีการประกาศ TON0 | FAIL R28/R31 |
| TC29 | POU หลายตัวเขียน Physical Output เดียวกัน | FAIL R29 |
| TC30 | Delete Coil แล้วเหลือ Wire/Edge ชี้ element ที่ไม่มีอยู่ | FAIL R30 |
| TC31 | Start/Stop Self-hold ใช้ Contact ของ Coil เดิม | PASS R13/R34 (ถ้า Wire ไม่มี Cycle) |
| TC32 | มี wire ย้อนกลับจาก Coil ไป upstream node เป็น directed cycle | FAIL R13/R34 |
| TC33 | Network ยังวาดไม่เสร็จ กด Save Draft | ALLOW R37 + รายงาน ERROR ไม่ใช่ Compile |
| TC34 | Network เดิมไม่สมบูรณ์ กด Compile | BLOCK R20/R37 |
| TC35 | โครงสร้างเกิน 32 elements/rung เมื่อ threshold ตั้งไว้ 32 | WARNING R38 |
| TC36 | Auto-fix ย้าย branch แล้วเปลี่ยน OR เป็น AND | REJECT R39 หากไม่มี semantic approval |
| TC37 | Export XML ที่มี refLocalId ไป Node ที่หายไป | FAIL R36 |
| TC38 | OpenPLC target ไม่ทราบว่า support coil subtype | UNKNOWN/BLOCK R41 เมื่อ feature จำเป็น |
| TC39 | Initial value/reset behavior ของ FB ถูกคาดเดาโดยไม่มี target evidence | UNKNOWN R32/R41 |
| TC40 | Undo/Redo ทำให้ Graph กลับสู่ original state และ IDs ถูกต้อง | PASS R30 |

## 8. ขอบเขตตามมาตรฐานและข้อแตกต่างระหว่าง PLC

- **IEC 61131-3:** ใช้อ้างอิงแนวคิดภาษา Ladder Diagram เท่านั้น การรองรับรูปแบบกราฟิกและตำแหน่ง Output Instruction อาจต่างกันตามผู้ผลิต
- **Coil อนุกรม:** `R08` เป็นการ **ตั้งข้อห้ามของ Editor นี้** โดยตั้งใจ ไม่ควรบอกว่า PLC ทุกยี่ห้อห้ามแบบนี้; บางแพลตฟอร์มมี Output Instructions ที่อนุกรมได้
- **Coil ขนาน:** `R07` เป็นรูปแบบที่ Editor นี้อนุญาต แต่ต้องตรวจ target จริงก่อน Compile/Export
- **Output และ Function Block:** Timer, Counter, Compare, Set/Reset, Jump, Return, function/function block และ instruction ที่มี enable/enable output มีข้อกำหนดต่างจาก Normal Coil; อย่าใช้ R06–R10 ตัดสินทั้งหมดโดยอัตโนมัติ
- **ลำดับการประมวลผล:** ค่า default เป็น `TOP_TO_BOTTOM` แต่ต้องยืนยันด้วย target compiler/runtime โดยเฉพาะเมื่อมีการเขียนตัวแปรซ้ำ
- **Safety:** วงจรหยุดฉุกเฉิน ระบบ interlock ที่เกี่ยวข้องกับความปลอดภัย และ safety PLC ต้องออกแบบ/ตรวจตามมาตรฐานและฮาร์ดแวร์ที่เกี่ยวข้อง ไม่ใช้การผ่านกฎการวาดเป็นหลักฐานรับรองความปลอดภัย

## 9. Vendor Profile / Extension (สำหรับพัฒนาต่อ)

เมื่อระบุ PLC/IDE ให้เพิ่ม `target_profile` เช่น:

```yaml
target_profile:
  vendor: OpenPLC
  plc_family: "OpenPLC Runtime (version pending)"
  ide_name: "OpenPLC Editor"
  ide_version: UNCONFIRMED
  runtime_version: UNCONFIRMED
  compiler_version: UNCONFIRMED
  firmware: NOT_APPLICABLE_OR_UNCONFIRMED
  coil_placement_rules: PENDING_VERIFICATION
  allowed_output_instruction_topologies: PENDING_VERIFICATION
  function_block_signatures: PENDING_VERIFICATION
  scan_order: PENDING_VERIFICATION
```

### หลักการ override

1. **โหมด `STRICT_EDITOR`:** R01–R42 ของเอกสารนี้เป็นกฎของโปรเจกต์ แม้บาง PLC จะรองรับมากกว่า
2. **โหมด `VENDOR_COMPATIBLE`:** ให้ใช้กฎ target ที่ผ่านการยืนยันจากเอกสารผู้ผลิต/Compiler แทนกฎเฉพาะของโปรเจกต์ในประเด็นที่แตกต่าง พร้อมบันทึกว่า override กฎใดและเพราะเหตุใด
3. หากไม่ทราบ vendor อย่าอ้างว่า Ladder Diagram ที่ผ่าน `STRICT_EDITOR` จะนำไป Compile บน PLC ทุกยี่ห้อได้

## 10. OpenPLC-specific Implementation Profile (ต้องอ่านก่อนใช้กับโปรเจกต์ OpenPLC)

### 10.1 ข้อเท็จจริงและข้อกำหนดของโปรเจกต์

| ประเด็น | วิธีปฏิบัติของ AI |
|---|---|
| Standard | ใช้แนวคิดภาษา `LD` ตาม IEC 61131-3; **อย่าเรียก IEEE** |
| Edition | IEC 61131-3:2025 เป็นฉบับปัจจุบัน **แต่** ไม่อนุมานว่า OpenPLC ที่ติดตั้งรองรับคุณสมบัติทุกอย่างในฉบับนี้ |
| Editor | ต้องระบุรุ่น/เวอร์ชันของ OpenPLC Editor ที่ใช้งานจริง (เช่น Desktop รุ่นเดิมกับรุ่นใหม่อาจแตกต่างกัน) |
| Runtime/Compiler | ต้องตรวจผลโดยใช้ Compiler/Runtime เวอร์ชันเป้าหมาย; การวาดได้ใน Editor ไม่เท่ากับ Compile ผ่าน |
| Output Coil | ในโหมด `STRICT_EDITOR` ต้องวางเป็น terminal ของแขนง ห้าม Coil อนุกรมหรือมี Contact ตามหลัง Coil; ถือเป็น **Project Policy** |
| Parallel Coils | อนุญาตในรูปแบบกราฟของโปรเจกต์เท่านั้น จนกว่าจะยืนยันกับ OpenPLC เวอร์ชันจริงว่า Editor และ Compiler รองรับ |
| IEC XML | หากต้องการแลกเปลี่ยนโปรเจกต์ ให้พิจารณา IEC 61131-10:2019/PLCopen XML และทดสอบ round-trip import/export จริง |
| Physical I/O | ตรวจ `BOOL` และ mapping ของ input/output กับเป้าหมาย ไม่สมมติ address หรือ pin จากตัวอย่าง |
| Safety | การผ่าน Validation ไม่ใช่การรับรองความปลอดภัยของเครื่องจักร; งาน Safety ต้องใช้วงจรและการประเมินเฉพาะทาง |

### 10.2 Flow สำหรับ AI เมื่อเขียน/แก้ไข OpenPLC Ladder

1. อ่านเอกสารนี้และเลือก `STRICT_EDITOR` เป็นค่าเริ่มต้น
2. รับ `OpenPLC Editor version`, `Runtime version`, `Compiler version` (หากไม่ทราบ ให้ระบุ `UNCONFIRMED`)
3. สร้าง Graph ของ Network จาก `elements` + `wires` พร้อมตรวจ R01–R42
4. ตรวจ logic ความหมายและ variable types โดยอิง symbol table ของโปรเจกต์
5. แสดง `ERROR`, `WARNING`, `UNKNOWN` พร้อมตัวอย่างการแก้ไข
6. **เมื่อมีเครื่องมือจริง** ให้ทดสอบใน OpenPLC Editor และคอมไพเลอร์เป้าหมาย; ไม่กล่าวอ้างว่าทดสอบแล้วหากยังไม่ได้ทำ
7. เก็บผล compile/import/export เป็น artifact ทดสอบ และอย่าสั่ง Download/Deploy ไปยัง PLC จริงโดยอัตโนมัติ

### 10.3 Feature flags สำหรับ OpenPLC

```yaml
openplc_profile:
  target: OpenPLC
  editor_version: UNCONFIRMED
  runtime_version: UNCONFIRMED
  compiler_version: UNCONFIRMED
  reference_standard: IEC_61131_3_2025
  project_policy: STRICT_EDITOR
  verify_actual_target_support: true
  parallel_normal_coils:
    project_allowed: true
    target_support: UNVERIFIED
  series_normal_coils:
    project_allowed: false
  contact_after_normal_coil:
    project_allowed: false
  output_coil_terminal_only: true
  reject_coil_before_rejoin_with_downstream_logic: true
  allow_terminal_output_merge_without_downstream_instructions: true
  plcopen_xml_exchange:
    planned: OPTIONAL
    target_support: UNVERIFIED
```

### 10.4 แหล่งอ้างอิงทางเทคนิค (URLs ตรวจสอบได้)

- IEC 61131-3:2025 (Edition 4) — https://webstore.iec.ch/en/publication/68533
- IEC 61131-10:2019 (XML exchange) — https://webstore.iec.ch/en/publication/33034
- Autonomy/OpenPLC Editor overview — https://edge.autonomylogic.com/docs/openplc-editor/overview/
- OpenPLC Editor / POU documentation — https://edge.autonomylogic.com/docs/openplc-editor/iec-concepts/pous/
- OpenPLC Editor / XML export — https://edge.autonomylogic.com/docs/openplc-editor/building-deploying/xml-export/
- Legacy OpenPLC Editor source (archive; **ไม่ใช่หลักฐานว่ารุ่นใหม่มีพฤติกรรมเหมือนเดิม**) — https://github.com/thiagoralves/OpenPLC_Editor
- PLCopen Coding Guidelines V1.0, L5 (Coil after Contact) / L6 (Rung complexity) — https://plcopen.org/system/files/downloads/plcopen_coding_guidelines_version_1.0.pdf
- Autonomy Edge Variables & Data Types — https://edge.autonomylogic.com/docs/openplc-editor/iec-concepts/variables-datatypes/
- Autonomy Edge Tasks & Instances — https://edge.autonomylogic.com/docs/openplc-editor/iec-concepts/tasks-instances/
- Autonomy Edge Worked LD TON/TOF Example — https://edge.autonomylogic.com/docs/openplc-editor/examples/blink/
- PLCopen IEC 61131-10 XML exchange — https://www.plcopen.org/standards/logic/iec-61131-10/

> **ความหมายของการอ้างอิงมาตรฐาน:** เอกสารนี้สรุปหลักการและตั้งนโยบายโปรเจกต์เอง ไม่ใช่การคัดลอกฉบับมาตรฐาน IEC และไม่ใช่ใบรับรอง compliance

## 11. คำสั่งย่อสำหรับ AI Agent

> ใช้เอกสาร `OpenPLC_IEC61131-3_Ladder_Rules_AI.md` เป็น Project Specification หลักของ Ladder Editor สำหรับ OpenPLC โดยใช้ `STRICT_EDITOR` เป็นค่าเริ่มต้น ตรวจ Graph Topology ของ Network ตาม R01–R42 โดยเฉพาะ R21, R22–R30 และ R34 ห้าม Coil ก่อนจุดรวมแขนงที่มี Logic ต่อท้าย, แยก ERROR/WARNING/INFO/UNKNOWN, บังคับ Coil อยู่สุดปลายแขนง, อนุญาต Coil ขนานเป็นคนละ Output Branch, ไม่อนุญาต Coil อนุกรม, ไม่อนุญาต Contact หรือ Branch หลัง Coil, ตรวจสายลอย/วงจรย้อนกลับ/Tag และ Function Block, และแสดงข้อผิดพลาดพร้อม rule_id และวิธีแก้ ห้ามสมมติว่านี่เป็นกฎสากลสำหรับ PLC ทุกยี่ห้อ หากระบุ target ให้ตรวจ Vendor Profile เพิ่มเติมก่อน Compile/Export

---

**การใช้เอกสาร:** วางไฟล์นี้ใน repository เช่น `docs/OpenPLC_IEC61131-3_Ladder_Rules_AI.md` แล้วอ้างอิงในคำสั่งของ AI coding agent หรือไฟล์กฎของโปรเจกต์ (`AGENTS.md`, `CLAUDE.md` ฯลฯ) ทั้งนี้เอกสารฉบับนี้เป็น **Draft v1.3** ที่ควรทดสอบกับ Ladder Editor และ PLC เป้าหมายก่อนนำไปใช้จริง

## 12. Revision Notes — v1.3.0

- เพิ่ม R22–R42 เพื่อปิดช่องว่างการต่อสาย, junction, typed ports, การเขียน/อ่าน I/O, Function Blocks, task execution, target compatibility และ editor mutations
- เพิ่ม Acceptance Tests TC19–TC40
- ชี้แจงว่า `self-hold` ที่อ้างอิง Tag ไม่ใช่ wire-cycle (R13/R34)
- เพิ่ม `DRAFT/COMPILE/DEPLOY` gate แยกกัน
- ปรับเกณฑ์ `duplicate writers` ให้แยก physical output และ internal state
- เพิ่มหลักฐาน OpenPLC/PLCopen และย้ำว่า Strict Editor Rules เป็น Project Policy ไม่ใช่การรับรองมาตรฐาน IEC
- **ยังไม่ยืนยัน** ว่ากฎเฉพาะ target ทำงานกับ OpenPLC Editor/Runtime เวอร์ชันใด เพราะไม่ได้รับข้อมูลเวอร์ชันและยังไม่ได้รัน Compiler จริง
