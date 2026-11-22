use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::ladder::{CoilMode, LadderElement, LadderProgram, Rung, TypeTag};

/// Validation profile for the Ladder Diagram validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ValidationPolicy {
    /// Project default policy (`STRICT_EDITOR` from OpenPLC_IEC61131-3_Ladder_Rules_AI.md).
    #[default]
    StrictEditor,
}

/// Severity level for a validation finding.
/// Ordered by severity rank: `Error > Warning > Unknown > Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
    Unknown,
}

impl ValidationSeverity {
    #[inline]
    pub fn rank(self) -> u8 {
        match self {
            ValidationSeverity::Info => 0,
            ValidationSeverity::Unknown => 1,
            ValidationSeverity::Warning => 2,
            ValidationSeverity::Error => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ValidationSeverity::Error => "ERROR",
            ValidationSeverity::Warning => "WARNING",
            ValidationSeverity::Unknown => "UNKNOWN",
            ValidationSeverity::Info => "INFO",
        }
    }
}

impl PartialOrd for ValidationSeverity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ValidationSeverity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

/// A single structured validation finding referencing a rule ID (`R01`–`R20`, `L01`–`L09`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationIssue {
    pub rule_id: &'static str,
    pub severity: ValidationSeverity,
    pub rung_idx: usize,
    pub element_ids: Vec<u64>,
    pub message: String,
    pub suggestion: String,
}

/// Returns `true` if `rule_id` is a structural/topology rule enforced transactionally by `R30`.
pub fn is_structural_topology_rule(rule_id: &str) -> bool {
    matches!(
        rule_id,
        "R01" | "R06" | "R08" | "R09" | "R10" | "R11" | "R12" | "R21" | "R25"
    )
}

/// Full program validation report returned by [`validate_program`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgramValidationReport {
    pub policy: ValidationPolicy,
    pub issues: Vec<ValidationIssue>,
}

impl ProgramValidationReport {
    /// R20 Gate: Program can compile only when there are zero `Error` issues.
    pub fn can_compile(&self) -> bool {
        !self
            .issues
            .iter()
            .any(|i| i.severity == ValidationSeverity::Error)
    }

    /// Returns all structural/topology `Error` issues (used by `R30` transactional edits).
    pub fn structural_errors(&self) -> Vec<&ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| {
                i.severity == ValidationSeverity::Error && is_structural_topology_rule(i.rule_id)
            })
            .collect()
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Warning)
            .count()
    }

    pub fn issues_for_rung(&self, rung_idx: usize) -> Vec<&ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.rung_idx == rung_idx)
            .collect()
    }

    pub fn issues_for_element(&self, elem_id: u64) -> Vec<&ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.element_ids.contains(&elem_id))
            .collect()
    }

    pub fn highest_severity_for_rung(&self, rung_idx: usize) -> Option<ValidationSeverity> {
        self.issues
            .iter()
            .filter(|i| i.rung_idx == rung_idx)
            .map(|i| i.severity)
            .max()
    }

    pub fn highest_severity_for_element(&self, elem_id: u64) -> Option<ValidationSeverity> {
        self.issues
            .iter()
            .filter(|i| i.element_ids.contains(&elem_id))
            .map(|i| i.severity)
            .max()
    }

    pub fn first_error_rung(&self) -> Option<usize> {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Error)
            .map(|i| i.rung_idx)
            .min()
    }

    pub fn first_issue_rung(&self) -> Option<usize> {
        self.issues
            .iter()
            .filter(|i| i.severity >= ValidationSeverity::Warning)
            .map(|i| i.rung_idx)
            .min()
            .or_else(|| self.issues.iter().map(|i| i.rung_idx).min())
    }
}

/// Returns `true` when `elem` is a `Branch` whose every path is non-empty and ends
/// in a terminal output (`is_right_aligned()` or a nested parallel output branch).
pub fn is_parallel_output_branch(elem: &LadderElement) -> bool {
    if elem.element_type != TypeTag::Branch || elem.branch_paths.is_empty() {
        return false;
    }
    elem.branch_paths.iter().all(|path| {
        path.last()
            .map(|last| last.element_type.is_right_aligned() || is_parallel_output_branch(last))
            .unwrap_or(false)
    })
}

/// Returns `true` if `elem` is a `Coil`, or a `Branch` that recursively contains any `Coil`.
pub fn branch_contains_any_coil(elem: &LadderElement) -> bool {
    if elem.element_type == TypeTag::Coil {
        return true;
    }
    if elem.element_type == TypeTag::Branch {
        return elem
            .branch_paths
            .iter()
            .any(|path| path.iter().any(branch_contains_any_coil));
    }
    false
}

fn is_normal_coil(el: &LadderElement) -> bool {
    el.element_type == TypeTag::Coil
        && matches!(el.coil_mode, CoilMode::Normal | CoilMode::Negated)
}

fn branch_all_paths_end_in_normal_coil(elem: &LadderElement) -> bool {
    if elem.element_type != TypeTag::Branch || elem.branch_paths.is_empty() {
        return false;
    }
    elem.branch_paths.iter().all(|path| {
        path.last()
            .map(|last| is_normal_coil(last) || branch_all_paths_end_in_normal_coil(last))
            .unwrap_or(false)
    })
}

fn is_word_only_address(name: &str) -> bool {
    let s = name.trim().to_ascii_uppercase();
    if s.is_empty() || s.contains('.') {
        return false;
    }
    for prefix in ["%MW", "%MD", "%ML", "%IW", "%ID", "%QW", "%QD"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    for prefix in ["AI", "AO"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    for prefix in ['D', 'V'] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

fn is_digits_or_bit_suffix(rest: &str) -> bool {
    if rest.is_empty() {
        return false;
    }
    let mut parts = rest.split('.');
    let first = match parts.next() {
        Some(p) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => p,
        _ => return false,
    };
    let _ = first;
    match parts.next() {
        None => true,
        Some(second) => {
            if parts.next().is_some() {
                return false;
            }
            !second.is_empty()
                && (second.chars().all(|c| c.is_ascii_digit())
                    || matches!(second, "Q" | "DN" | "ET" | "CV" | "IN" | "QU" | "QD"))
        }
    }
}

fn is_physical_input_address(name: &str) -> bool {
    let raw = name.trim().to_ascii_uppercase();
    if raw.is_empty() {
        return false;
    }
    let canon = LadderProgram::canonical_address(&raw).to_ascii_uppercase();
    for s in [&raw, &canon] {
        for prefix in ["%IX", "%IW", "%ID", "%IB", "AI"] {
            if let Some(rest) = s.strip_prefix(prefix) {
                if is_digits_or_bit_suffix(rest) {
                    return true;
                }
            }
        }
        for prefix in ['X', 'I'] {
            if let Some(rest) = s.strip_prefix(prefix) {
                if is_digits_or_bit_suffix(rest) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_physical_output_address(canon: &str) -> bool {
    let s = canon.trim().to_ascii_uppercase();
    if s.is_empty() {
        return false;
    }
    for prefix in ["%QX", "%QW", "%QD", "%QB", "AO"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if is_digits_or_bit_suffix(rest) {
                return true;
            }
        }
    }
    for prefix in ['Y', 'Q'] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if is_digits_or_bit_suffix(rest) {
                return true;
            }
        }
    }
    false
}

fn is_reserved_iec_keyword(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_uppercase().as_str(),
        "LD" | "LDN"
            | "AND"
            | "ANDN"
            | "OR"
            | "ORN"
            | "XOR"
            | "XORN"
            | "NOT"
            | "ST"
            | "STN"
            | "S"
            | "R"
            | "CAL"
            | "CALC"
            | "CALCN"
            | "RET"
            | "RETC"
            | "RETCN"
            | "JMP"
            | "JMPC"
            | "JMPCN"
            | "VAR"
            | "END_VAR"
            | "PROGRAM"
            | "END_PROGRAM"
            | "FUNCTION"
            | "END_FUNCTION"
            | "FUNCTION_BLOCK"
            | "END_FUNCTION_BLOCK"
            | "BOOL"
            | "INT"
            | "DINT"
            | "REAL"
            | "TIME"
            | "STRING"
            | "TRUE"
            | "FALSE"
            | "IF"
            | "THEN"
            | "ELSE"
            | "END_IF"
            | "FOR"
            | "WHILE"
            | "CASE"
    )
}

fn is_direct_plc_address(name: &str) -> bool {
    let s = name.trim().to_ascii_uppercase();
    if s.is_empty() {
        return false;
    }
    if matches!(
        s.as_str(),
        "ALWAYS_ON" | "ALWAYS_OFF" | "FIRST_SCAN" | "CLK_1S" | "CLK_100MS"
    ) {
        return true;
    }
    for prefix in [
        "%IX", "%QX", "%MX", "%MW", "%MD", "%ML", "%IW", "%ID", "%QW", "%QD", "%IB", "%QB", "%MB",
        "%SM", "%SD", "CTUD", "RTRIG", "FTRIG", "TON", "TOF", "CTU", "CTD", "TP", "SR", "RS", "SM",
        "SD", "AI", "AO", "MB",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if is_digits_or_bit_suffix(rest) {
                return true;
            }
        }
    }
    for prefix in ['X', 'Y', 'M', 'R', 'T', 'C', 'D', 'V', 'I', 'Q', 'S', 'U'] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if is_digits_or_bit_suffix(rest) {
                return true;
            }
        }
    }
    false
}

fn is_symbol_declared(program: &LadderProgram, name: &str) -> bool {
    let n = name.trim();
    if program
        .symbols
        .records
        .iter()
        .any(|r| r.symbol.eq_ignore_ascii_case(n) || r.address.eq_ignore_ascii_case(n))
    {
        return true;
    }
    program.io_mappings.iter().any(|io| {
        io.symbol.eq_ignore_ascii_case(n)
            || io.plc_address.eq_ignore_ascii_case(n)
            || io.iec_address.eq_ignore_ascii_case(n)
    })
}

fn count_rung_elements(elements: &[LadderElement]) -> usize {
    let mut total = 0;
    for el in elements {
        if el.element_type == TypeTag::Branch {
            for path in &el.branch_paths {
                total += count_rung_elements(path);
            }
        } else {
            total += 1;
        }
    }
    total
}

fn path_is_pure_wire(path: &[LadderElement]) -> bool {
    !path.is_empty() && path.iter().all(|e| e.element_type == TypeTag::Wire)
}

fn path_has_condition_logic(path: &[LadderElement]) -> bool {
    path.iter().any(|e| match e.element_type {
        TypeTag::Contact
        | TypeTag::Compare
        | TypeTag::InRange
        | TypeTag::RTrig
        | TypeTag::FTrig
        | TypeTag::UartRecv => true,
        TypeTag::Branch => e.branch_paths.iter().any(|p| path_has_condition_logic(p)),
        _ => false,
    })
}

fn rung_has_guard_before_jump(elements: &[LadderElement], jmp_id: u64) -> bool {
    fn walk(elements: &[LadderElement], jmp_id: u64, saw_guard: bool) -> Option<bool> {
        let mut guarded = saw_guard;
        for el in elements {
            if el.id == jmp_id {
                return Some(guarded);
            }
            match el.element_type {
                TypeTag::Contact
                | TypeTag::Compare
                | TypeTag::InRange
                | TypeTag::RTrig
                | TypeTag::FTrig
                | TypeTag::UartRecv => {
                    guarded = true;
                }
                TypeTag::Branch => {
                    for path in &el.branch_paths {
                        if let Some(res) = walk(path, jmp_id, guarded) {
                            return Some(res);
                        }
                    }
                    if el.branch_paths.iter().any(|p| path_has_condition_logic(p)) {
                        guarded = true;
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(elements, jmp_id, false).unwrap_or(false)
}

fn collect_jumps(elements: &[LadderElement], out: &mut Vec<(u64, String)>) {
    for el in elements {
        if el.element_type == TypeTag::Jump {
            out.push((el.id, el.name.trim().to_ascii_uppercase()));
        } else if el.element_type == TypeTag::Branch {
            for path in &el.branch_paths {
                collect_jumps(path, out);
            }
        }
    }
}

struct CoilRegistry {
    normal: HashMap<String, Vec<(usize, u64)>>,
    set_latch: HashMap<String, Vec<(usize, u64)>>,
    reset_unlatch: HashMap<String, Vec<(usize, u64)>>,
}

impl CoilRegistry {
    fn new() -> Self {
        Self {
            normal: HashMap::new(),
            set_latch: HashMap::new(),
            reset_unlatch: HashMap::new(),
        }
    }

    fn record(&mut self, rung_idx: usize, el: &LadderElement) {
        if el.element_type != TypeTag::Coil {
            return;
        }
        let raw = el.name.trim();
        if raw.is_empty() {
            return;
        }
        let canon = LadderProgram::canonical_address(raw).to_ascii_uppercase();
        match el.coil_mode {
            CoilMode::Normal | CoilMode::Negated => {
                self.normal.entry(canon).or_default().push((rung_idx, el.id));
            }
            CoilMode::SetLatch => {
                self.set_latch
                    .entry(canon)
                    .or_default()
                    .push((rung_idx, el.id));
            }
            CoilMode::ResetUnlatch => {
                self.reset_unlatch
                    .entry(canon)
                    .or_default()
                    .push((rung_idx, el.id));
            }
            CoilMode::PositiveTransition | CoilMode::NegativeTransition => {}
        }
    }
}

fn validate_element_semantics(
    program: &LadderProgram,
    rung_idx: usize,
    el: &LadderElement,
    issues: &mut Vec<ValidationIssue>,
) {
    // R31: Symbol resolution & reserved IEC keyword checks on variable-referencing elements
    if matches!(
        el.element_type,
        TypeTag::Contact
            | TypeTag::Coil
            | TypeTag::Ton
            | TypeTag::Tof
            | TypeTag::Tp
            | TypeTag::Ctu
            | TypeTag::Ctd
            | TypeTag::Ctud
            | TypeTag::Compare
    ) {
        let n = el.name.trim();
        if !n.is_empty() {
            if is_reserved_iec_keyword(n) {
                issues.push(ValidationIssue {
                    rule_id: "R31",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: format!("Tag '{}' uses a reserved IEC 61131-3 keyword", n),
                    suggestion: "Rename the tag to a non-reserved identifier or valid PLC address".to_string(),
                });
            } else if !is_direct_plc_address(n)
                && !is_symbol_declared(program, n)
                && !(el.element_type == TypeTag::Compare
                    && crate::ladder::parse_int_literal(n).is_some())
            {
                issues.push(ValidationIssue {
                    rule_id: "R31",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: format!(
                        "Undefined symbol '{}': not declared in Symbol Table or I/O Mapping",
                        n
                    ),
                    suggestion: "Declare the symbol in the Global Symbol Table / I/O Mapping or use a direct PLC address (e.g. %IX0.0, %QX0.0, %MX0.0)".to_string(),
                });
            }
        }
    }

    match el.element_type {
        TypeTag::Contact | TypeTag::Coil => {
            if el.name.trim().is_empty() {
                issues.push(ValidationIssue {
                    rule_id: "R16",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: "Missing tag/address on Contact/Coil".to_string(),
                    suggestion: "Assign a valid BOOL tag or bit address (e.g. %IX0.0, %QX0.0, %MX0.0)".to_string(),
                });
            } else {
                // R26: Write permission check on Coils (physical inputs are read-only)
                if el.element_type == TypeTag::Coil && is_physical_input_address(&el.name) {
                    issues.push(ValidationIssue {
                        rule_id: "R26",
                        severity: ValidationSeverity::Error,
                        rung_idx,
                        element_ids: vec![el.id],
                        message: format!(
                            "Cannot write to read-only physical input '{}'",
                            el.name.trim()
                        ),
                        suggestion: "Use a physical output (%QX0.0 / Y0) or internal bit (%MX0.0 / M0)".to_string(),
                    });
                }
                if is_word_only_address(&el.name) {
                    issues.push(ValidationIssue {
                        rule_id: "R16",
                        severity: ValidationSeverity::Error,
                        rung_idx,
                        element_ids: vec![el.id],
                        message: format!(
                            "Contact/Coil '{}' requires a BOOL tag/address, not a word register",
                            el.name.trim()
                        ),
                        suggestion: "Use a BOOL address (e.g. %IX0.0, %QX0.0, %MX0.0) or bit-indexed register (e.g. %MW0.0)".to_string(),
                    });
                }
            }
        }
        TypeTag::Ton | TypeTag::Tof | TypeTag::Tp | TypeTag::Ctu | TypeTag::Ctd | TypeTag::Ctud => {
            if el.name.trim().is_empty() || el.preset_value < 0 {
                issues.push(ValidationIssue {
                    rule_id: "R17",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: "Timer/Counter block missing required name or has negative preset".to_string(),
                    suggestion: "Provide a valid timer/counter address and a non-negative preset value".to_string(),
                });
            }
        }
        TypeTag::Compare => {
            if el.name.trim().is_empty() {
                issues.push(ValidationIssue {
                    rule_id: "R17",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: "Compare block missing required operand address".to_string(),
                    suggestion: "Specify a valid register or variable address to compare".to_string(),
                });
            }
            let op = el.compare_op.trim();
            if !matches!(op, "==" | "!=" | ">" | "<" | ">=" | "<=" | "=" | "<>") {
                issues.push(ValidationIssue {
                    rule_id: "L08",
                    severity: ValidationSeverity::Unknown,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: format!("Unknown Compare operator signature '{}'", op),
                    suggestion: "Use a standard comparison operator (==, !=, >, <, >=, <=)".to_string(),
                });
            }
        }
        TypeTag::Math => {
            if el.target_reg.trim().is_empty() {
                issues.push(ValidationIssue {
                    rule_id: "R17",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: "Math block missing required target register".to_string(),
                    suggestion: "Assign a destination word register (e.g. %MW0 or D0)".to_string(),
                });
            }
            let op = el.math_op.trim().to_ascii_uppercase();
            if !matches!(
                op.as_str(),
                "+" | "-" | "*" | "/" | "%" | "ADD" | "SUB" | "MUL" | "DIV" | "MOD"
            ) {
                issues.push(ValidationIssue {
                    rule_id: "L08",
                    severity: ValidationSeverity::Unknown,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: format!("Unknown Math operator signature '{}'", el.math_op.trim()),
                    suggestion: "Use a standard math operator (+, -, *, /, %)".to_string(),
                });
            }
        }
        TypeTag::Bitwise => {
            if el.target_reg.trim().is_empty() {
                issues.push(ValidationIssue {
                    rule_id: "R17",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: "Bitwise block missing required target register".to_string(),
                    suggestion: "Assign a destination word register (e.g. %MW0 or D0)".to_string(),
                });
            }
            let op = el.math_op.trim().to_ascii_uppercase();
            if !op.is_empty()
                && !matches!(
                    op.as_str(),
                    "AND" | "OR" | "XOR" | "NOT" | "SHL" | "SHR" | "&" | "|" | "^" | "~" | "<<" | ">>"
                )
            {
                issues.push(ValidationIssue {
                    rule_id: "L08",
                    severity: ValidationSeverity::Unknown,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: format!("Unknown Bitwise operator signature '{}'", el.math_op.trim()),
                    suggestion: "Use a standard bitwise operator (AND, OR, XOR, NOT, SHL, SHR)".to_string(),
                });
            }
        }
        TypeTag::Move | TypeTag::Scale | TypeTag::Limit | TypeTag::Pid | TypeTag::Select => {
            if el.target_reg.trim().is_empty() {
                issues.push(ValidationIssue {
                    rule_id: "R17",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: format!(
                        "{} block missing required target register",
                        el.element_type.display_name()
                    ),
                    suggestion: "Assign a destination word register (e.g. %MW0 or D0)".to_string(),
                });
            }
        }
        TypeTag::Jump | TypeTag::Label => {
            if el.name.trim().is_empty() {
                issues.push(ValidationIssue {
                    rule_id: "R17",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: "JMP/LBL instruction missing target label name".to_string(),
                    suggestion: "Specify a non-empty label identifier".to_string(),
                });
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn walk_sequence(
    program: &LadderProgram,
    rung_idx: usize,
    elements: &[LadderElement],
    mut seen_normal_coil: Option<u64>,
    depth: usize,
    is_top_level: bool,
    downstream_after_branch: Option<u64>,
    coils: &mut CoilRegistry,
    issues: &mut Vec<ValidationIssue>,
) {
    for (idx, el) in elements.iter().enumerate() {
        let has_more = idx + 1 < elements.len();

        // Check R08 / R09 / R10 when a normal coil was already seen earlier in series
        if let Some(prev_id) = seen_normal_coil {
            if el.element_type == TypeTag::Branch {
                issues.push(ValidationIssue {
                    rule_id: "R10",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![prev_id, el.id],
                    message: "Branch cannot start from the output side of a Coil".to_string(),
                    suggestion: "Move the branch point before all Coils".to_string(),
                });
            } else if is_normal_coil(el) {
                issues.push(ValidationIssue {
                    rule_id: "R08",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![prev_id, el.id],
                    message: "Normal Coils cannot be placed in series on the same path".to_string(),
                    suggestion: "Place Coils in parallel output branches or separate networks".to_string(),
                });
            } else {
                issues.push(ValidationIssue {
                    rule_id: "R09",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![prev_id, el.id],
                    message: "Elements cannot be placed after a Normal Coil".to_string(),
                    suggestion: "Move input/logic elements before the Coil".to_string(),
                });
            }
        }

        // R01: Unreachable elements after JMP/RET
        if (el.element_type == TypeTag::Jump || el.element_type == TypeTag::Return) && has_more {
            issues.push(ValidationIssue {
                rule_id: "R01",
                severity: ValidationSeverity::Error,
                rung_idx,
                element_ids: vec![el.id, elements[idx + 1].id],
                message: "Unreachable elements after JMP/RET".to_string(),
                suggestion: "Remove elements after JMP/RET or move them to another network".to_string(),
            });
        }

        // R01: Label must be at index 0 of top-level rung
        if el.element_type == TypeTag::Label && (!is_top_level || idx != 0) {
            issues.push(ValidationIssue {
                rule_id: "R01",
                severity: ValidationSeverity::Error,
                rung_idx,
                element_ids: vec![el.id],
                message: "LBL must be placed at the start of the network".to_string(),
                suggestion: "Move LBL to the left rail at the beginning of the network".to_string(),
            });
        }

        if el.element_type == TypeTag::Branch {
            if el.branch_paths.is_empty() {
                issues.push(ValidationIssue {
                    rule_id: "R12",
                    severity: ValidationSeverity::Error,
                    rung_idx,
                    element_ids: vec![el.id],
                    message: "Branch has no paths".to_string(),
                    suggestion: "Add at least one valid branch path or delete the empty branch".to_string(),
                });
            } else {
                if depth + 1 > 3 {
                    issues.push(ValidationIssue {
                        rule_id: "L09",
                        severity: ValidationSeverity::Warning,
                        rung_idx,
                        element_ids: vec![el.id],
                        message: "Deeply nested branch (> 3 levels)".to_string(),
                        suggestion: "Simplify branch nesting or split into multiple networks".to_string(),
                    });
                }

                // R14 / L03: Bypass wire shorting across parallel conditions
                if el.branch_paths.len() >= 2
                    && el.branch_paths.iter().any(|p| path_is_pure_wire(p))
                    && el.branch_paths.iter().any(|p| path_has_condition_logic(p))
                {
                    issues.push(ValidationIssue {
                        rule_id: "R14",
                        severity: ValidationSeverity::Warning,
                        rung_idx,
                        element_ids: vec![el.id],
                        message: "Bypass wire shorts across parallel conditions".to_string(),
                        suggestion: "Remove the bypass wire if unintentional, or verify always-true intent".to_string(),
                    });
                }

                let next_downstream = if has_more {
                    Some(elements[idx + 1].id)
                } else {
                    downstream_after_branch
                };

                // R21.3 & R25: Terminal branch containing any Coil must have every path end in a terminal output
                if downstream_after_branch.is_none() && !has_more && branch_contains_any_coil(el) {
                    for path in &el.branch_paths {
                        if let Some(last) = path.last() {
                            if !(last.element_type.is_right_aligned()
                                || is_parallel_output_branch(last))
                            {
                                if path_is_pure_wire(path) {
                                    issues.push(ValidationIssue {
                                        rule_id: "R25",
                                        severity: ValidationSeverity::Error,
                                        rung_idx,
                                        element_ids: vec![last.id],
                                        message: "Coil-free bypass wire reaches right rail without passing through any Coil".to_string(),
                                        suggestion: "Remove the bypass wire or add a terminal Coil/output to this branch path".to_string(),
                                    });
                                } else {
                                    issues.push(ValidationIssue {
                                        rule_id: "R21",
                                        severity: ValidationSeverity::Error,
                                        rung_idx,
                                        element_ids: vec![last.id],
                                        message: "Parallel output branch path must terminate in an output connected to the right rail".to_string(),
                                        suggestion: "Add a terminal Coil/output to this branch path or separate the branches".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }

                for path in &el.branch_paths {
                    if path.is_empty() {
                        issues.push(ValidationIssue {
                            rule_id: "R11",
                            severity: ValidationSeverity::Error,
                            rung_idx,
                            element_ids: vec![el.id],
                            message: "Empty/dangling branch path".to_string(),
                            suggestion: "Add elements to the branch path or remove the empty path".to_string(),
                        });
                    } else {
                        // Each branch path gets its own copy of `seen_normal_coil` (R07 allows parallel output coils)
                        walk_sequence(
                            program,
                            rung_idx,
                            path,
                            seen_normal_coil,
                            depth + 1,
                            false,
                            next_downstream,
                            coils,
                            issues,
                        );
                    }
                }

                // If every path of this branch ended in a normal coil, nothing may follow it in series (R06/R09)
                if branch_all_paths_end_in_normal_coil(el) && has_more {
                    seen_normal_coil = Some(el.id);
                }
            }
        } else if el.element_type == TypeTag::OpenCircuit {
            issues.push(ValidationIssue {
                rule_id: "R11",
                severity: ValidationSeverity::Error,
                rung_idx,
                element_ids: vec![el.id],
                message: "Open circuit creates a disconnected/dangling path".to_string(),
                suggestion: "Replace the open circuit with a valid element or remove the disconnected path".to_string(),
            });
        } else {
            validate_element_semantics(program, rung_idx, el, issues);
            coils.record(rung_idx, el);
            // R21.1 / R21.4: Any Coil (all modes) inside a branch that merges into downstream elements is forbidden
            if el.element_type == TypeTag::Coil {
                if let Some(after_id) = downstream_after_branch {
                    issues.push(ValidationIssue {
                        rule_id: "R21",
                        severity: ValidationSeverity::Error,
                        rung_idx,
                        element_ids: vec![el.id, after_id],
                        message: format!(
                            "Coil '{}' cannot be placed before a branch merge with downstream elements",
                            el.name.trim()
                        ),
                        suggestion: "Move the Coil to a terminal parallel output branch or separate network connecting directly to the right rail".to_string(),
                    });
                }
            }
            if is_normal_coil(el) {
                seen_normal_coil = Some(el.id);
            }
        }
    }
}

fn rung_has_terminal_output(rung: &Rung) -> bool {
    rung.elements
        .iter()
        .any(|el| el.element_type.is_right_aligned() || is_parallel_output_branch(el))
}

/// Validate an entire [`LadderProgram`] under `STRICT_EDITOR` policy (R01–R20, L01–L09).
pub fn validate_program(program: &LadderProgram) -> ProgramValidationReport {
    let mut issues = Vec::new();
    let mut coils = CoilRegistry::new();
    let mut label_rung: HashMap<String, usize> = HashMap::new();

    // First pass: record label definitions (top-to-bottom, R19)
    for (r_idx, rung) in program.rungs.iter().enumerate() {
        for el in &rung.elements {
            if el.element_type == TypeTag::Label {
                let lbl = el.name.trim().to_ascii_uppercase();
                if !lbl.is_empty() {
                    label_rung.entry(lbl).or_insert(r_idx);
                }
            }
        }
    }

    // Second pass: per-rung structural & semantic checks (top-to-bottom, R19)
    for (r_idx, rung) in program.rungs.iter().enumerate() {
        if rung.elements.is_empty() {
            issues.push(ValidationIssue {
                rule_id: "L04",
                severity: ValidationSeverity::Info,
                rung_idx: r_idx,
                element_ids: Vec::new(),
                message: "Empty network".to_string(),
                suggestion: "Add logic elements or remove unused network".to_string(),
            });
            continue;
        }

        let is_comment_or_label_only = rung
            .elements
            .iter()
            .all(|e| matches!(e.element_type, TypeTag::Comment | TypeTag::Label));
        if is_comment_or_label_only {
            for (idx, el) in rung.elements.iter().enumerate() {
                if el.element_type == TypeTag::Label {
                    if idx != 0 {
                        issues.push(ValidationIssue {
                            rule_id: "R01",
                            severity: ValidationSeverity::Error,
                            rung_idx: r_idx,
                            element_ids: vec![el.id],
                            message: "LBL must be placed at the start of the network".to_string(),
                            suggestion: "Move LBL to the left rail at the beginning of the network".to_string(),
                        });
                    }
                    validate_element_semantics(program, r_idx, el, &mut issues);
                }
            }
            continue;
        }

        // R38: Network complexity limit (recommended <= 32 elements)
        let elem_count = count_rung_elements(&rung.elements);
        if elem_count > 32 {
            issues.push(ValidationIssue {
                rule_id: "R38",
                severity: ValidationSeverity::Warning,
                rung_idx: r_idx,
                element_ids: Vec::new(),
                message: format!("Network has {} elements (recommended limit: 32)", elem_count),
                suggestion: "Split complex logic into multiple networks for readability".to_string(),
            });
        }

        // R02: Non-empty logic rung must have a terminal output instruction
        if !rung_has_terminal_output(rung) {
            let ids: Vec<u64> = rung.elements.iter().map(|e| e.id).collect();
            issues.push(ValidationIssue {
                rule_id: "R02",
                severity: ValidationSeverity::Error,
                rung_idx: r_idx,
                element_ids: ids,
                message: "Network has no terminal output instruction".to_string(),
                suggestion: "Add a Coil or output instruction at the end of the network".to_string(),
            });
        }

        walk_sequence(
            program,
            r_idx,
            &rung.elements,
            None,
            0,
            true,
            None,
            &mut coils,
            &mut issues,
        );

        // R13: Unconditional backward jump cycle check
        let mut jumps = Vec::new();
        collect_jumps(&rung.elements, &mut jumps);
        for (jmp_id, target_lbl) in jumps {
            if let Some(&target_rung) = label_rung.get(&target_lbl) {
                if target_rung <= r_idx && !rung_has_guard_before_jump(&rung.elements, jmp_id) {
                    issues.push(ValidationIssue {
                        rule_id: "R13",
                        severity: ValidationSeverity::Error,
                        rung_idx: r_idx,
                        element_ids: vec![jmp_id],
                        message: format!(
                            "Unconditional backward jump to '{}' creates an infinite cycle",
                            target_lbl
                        ),
                        suggestion: "Add a guarding condition before JMP or jump forward only".to_string(),
                    });
                }
            }
        }
    }

    // Program-wide check R27: I/O Location Overlap
    let mut io_by_addr: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, io) in program.io_mappings.iter().enumerate() {
        let raw_addr = if !io.iec_address.trim().is_empty() {
            io.iec_address.trim()
        } else {
            io.plc_address.trim()
        };
        if !raw_addr.is_empty() {
            let canon = LadderProgram::canonical_address(raw_addr).to_ascii_uppercase();
            io_by_addr.entry(canon).or_default().push(idx);
        }
    }
    for (canon, indices) in io_by_addr {
        if indices.len() >= 2 {
            if is_physical_output_address(&canon) {
                issues.push(ValidationIssue {
                    rule_id: "R27",
                    severity: ValidationSeverity::Error,
                    rung_idx: 0,
                    element_ids: Vec::new(),
                    message: format!(
                        "I/O location overlap: physical output '{}' is mapped {} times",
                        canon,
                        indices.len()
                    ),
                    suggestion: "Assign a unique physical output address and hardware pin to each I/O mapping".to_string(),
                });
            } else {
                issues.push(ValidationIssue {
                    rule_id: "R27",
                    severity: ValidationSeverity::Warning,
                    rung_idx: 0,
                    element_ids: Vec::new(),
                    message: format!(
                        "I/O location overlap: physical input '{}' is mapped {} times",
                        canon,
                        indices.len()
                    ),
                    suggestion: "Verify shared input mapping or assign unique input addresses".to_string(),
                });
            }
        }
    }

    // Program-wide check R31: Duplicate symbol names mapped to different canonical addresses
    let mut sym_to_addrs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rec in &program.symbols.records {
        let sym = rec.symbol.trim().to_ascii_uppercase();
        if sym.is_empty() {
            continue;
        }
        let canon = LadderProgram::canonical_address(&rec.address).to_ascii_uppercase();
        let list = sym_to_addrs.entry(sym).or_default();
        if !list.contains(&canon) {
            list.push(canon);
        }
    }
    for (sym, addrs) in sym_to_addrs {
        if addrs.len() >= 2 {
            issues.push(ValidationIssue {
                rule_id: "R31",
                severity: ValidationSeverity::Error,
                rung_idx: 0,
                element_ids: Vec::new(),
                message: format!(
                    "Duplicate symbol '{}' mapped to multiple addresses ({})",
                    sym,
                    addrs.join(", ")
                ),
                suggestion: "Ensure each symbol name maps to a single unique PLC address".to_string(),
            });
        }
    }

    // Program-wide check R29 (Physical Output ERROR) / R15 (Internal Bit WARNING): Duplicate Normal Coil
    for (tag, occs) in &coils.normal {
        if occs.len() >= 2 {
            let is_phys_out = is_physical_output_address(tag);
            let mut by_rung: BTreeMap<usize, Vec<u64>> = BTreeMap::new();
            for &(r_idx, id) in occs {
                by_rung.entry(r_idx).or_default().push(id);
            }
            for (r_idx, ids) in by_rung {
                if is_phys_out {
                    issues.push(ValidationIssue {
                        rule_id: "R29",
                        severity: ValidationSeverity::Error,
                        rung_idx: r_idx,
                        element_ids: ids,
                        message: format!(
                            "Physical output '{}' has multiple writers ({} occurrences)",
                            tag,
                            occs.len()
                        ),
                        suggestion: "Drive each physical output from a single Coil; combine logic with OR branches or internal %MX bits".to_string(),
                    });
                } else {
                    issues.push(ValidationIssue {
                        rule_id: "R15",
                        severity: ValidationSeverity::Warning,
                        rung_idx: r_idx,
                        element_ids: ids,
                        message: format!(
                            "Duplicate normal Coil writing '{}' ({} occurrences across program)",
                            tag,
                            occs.len()
                        ),
                        suggestion: "Combine conditions into a single Coil or use Set/Reset coils".to_string(),
                    });
                }
            }
        }
    }

    // Program-wide check R29 (Physical Output mixed Normal+S/R ERROR) / R18 (Set/Reset conflict WARNING)
    let mut sr_tags: Vec<String> = coils
        .set_latch
        .keys()
        .chain(coils.reset_unlatch.keys())
        .cloned()
        .collect();
    sr_tags.sort();
    sr_tags.dedup();

    for tag in sr_tags {
        let sets = coils.set_latch.get(&tag).map(Vec::as_slice).unwrap_or(&[]);
        let resets = coils
            .reset_unlatch
            .get(&tag)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let normals = coils.normal.get(&tag).map(Vec::as_slice).unwrap_or(&[]);

        let has_sr_pair = !sets.is_empty() && !resets.is_empty();
        let has_mixed_normal = (!sets.is_empty() || !resets.is_empty()) && !normals.is_empty();
        if has_sr_pair || has_mixed_normal {
            let is_phys_out = is_physical_output_address(&tag);
            let mut by_rung: BTreeMap<usize, Vec<u64>> = BTreeMap::new();
            for &(r_idx, id) in sets.iter().chain(resets.iter()).chain(normals.iter()) {
                by_rung.entry(r_idx).or_default().push(id);
            }
            if has_mixed_normal && is_phys_out {
                for (r_idx, ids) in by_rung {
                    issues.push(ValidationIssue {
                        rule_id: "R29",
                        severity: ValidationSeverity::Error,
                        rung_idx: r_idx,
                        element_ids: ids,
                        message: format!(
                            "Physical output '{}' mixes Normal Coil with Set/Reset latch",
                            tag
                        ),
                        suggestion: "Use either a single Normal Coil or a dedicated Set/Reset pair on a physical output".to_string(),
                    });
                }
            } else {
                let msg = if has_mixed_normal {
                    format!(
                        "Tag '{}' mixes Set/Reset latch with a Normal Coil",
                        tag
                    )
                } else {
                    format!(
                        "Tag '{}' has both Set (S) and Reset (R) coils — verify scan order precedence",
                        tag
                    )
                };
                for (r_idx, ids) in by_rung {
                    issues.push(ValidationIssue {
                        rule_id: "R18",
                        severity: ValidationSeverity::Warning,
                        rung_idx: r_idx,
                        element_ids: ids,
                        message: msg.clone(),
                        suggestion: "Review Set/Reset precedence and avoid mixing Normal Coils with S/R on the same tag".to_string(),
                    });
                }
            }
        }
    }

    ProgramValidationReport {
        policy: ValidationPolicy::StrictEditor,
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ladder::ContactMode;

    fn prog_with_rungs(rungs: Vec<Rung>) -> LadderProgram {
        let mut p = LadderProgram::new_empty();
        p.rungs = rungs;
        p
    }

    fn rung_with(id: usize, elements: Vec<LadderElement>) -> Rung {
        let mut r = Rung::new(id);
        r.elements = elements;
        r
    }

    #[test]
    fn tc01_series_contacts_to_coil_passes() {
        // |----[ A ]----[ B ]----( Q1 )----|
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                LadderElement::new_contact(2, "%IX0.1", "B", ContactMode::NormallyOpen),
                LadderElement::new_coil(3, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(report.can_compile());
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.warning_count(), 0);
    }

    #[test]
    fn tc02_parallel_contacts_to_coil_passes() {
        // |----+--[ A ]--+--( Q1 )----|
        //      +--[ B ]--+
        let br = LadderElement::new_branch(
            10,
            vec![
                vec![LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen)],
                vec![LadderElement::new_contact(2, "%IX0.1", "B", ContactMode::NormallyOpen)],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                br,
                LadderElement::new_coil(3, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(report.can_compile());
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn tc03_parallel_output_coils_passes() {
        // |----[ A ]----+----( Q1 )----|
        //               +----( Q2 )----|
        let out_br = LadderElement::new_branch(
            10,
            vec![
                vec![LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal)],
                vec![LadderElement::new_coil(3, "%QX0.1", "Q2", CoilMode::Normal)],
            ],
        );
        assert!(is_parallel_output_branch(&out_br));
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                out_br,
            ],
        )]);
        let report = validate_program(&p);
        assert!(report.can_compile());
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn tc04_series_coils_fails_r08() {
        // |----[ A ]----( Q1 )----( Q2 )----|
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal),
                LadderElement::new_coil(3, "%QX0.1", "Q2", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R08" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn tc05_contact_after_coil_fails_r09() {
        // |----[ A ]----( Q1 )----[ B ]----|
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal),
                LadderElement::new_contact(3, "%IX0.1", "B", ContactMode::NormallyOpen),
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R09" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn tc06_dangling_empty_branch_fails_r11_r12() {
        let br_empty_path = LadderElement::new_branch(
            10,
            vec![
                vec![LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen)],
                vec![],
            ],
        );
        let p1 = prog_with_rungs(vec![rung_with(
            0,
            vec![
                br_empty_path,
                LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let r1 = validate_program(&p1);
        assert!(!r1.can_compile());
        assert!(r1
            .issues
            .iter()
            .any(|i| i.rule_id == "R11" && i.severity == ValidationSeverity::Error));

        let br_no_paths = LadderElement::new_branch(11, vec![]);
        let p2 = prog_with_rungs(vec![rung_with(
            0,
            vec![
                br_no_paths,
                LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let r2 = validate_program(&p2);
        assert!(!r2.can_compile());
        assert!(r2
            .issues
            .iter()
            .any(|i| i.rule_id == "R12" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn tc07_branch_after_coil_fails_r10() {
        // |----[ A ]----( Q1 )----+----( Q2 )----|
        //                         +----( Q3 )----|
        let out_br = LadderElement::new_branch(
            10,
            vec![
                vec![LadderElement::new_coil(3, "%QX0.1", "Q2", CoilMode::Normal)],
                vec![LadderElement::new_coil(4, "%QX0.2", "Q3", CoilMode::Normal)],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal),
                out_br,
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R10" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn tc08_no_and_nc_contact_to_coil_passes() {
        // |----[ A ]----[/STOP]----( MOTOR )----|
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                LadderElement::new_contact(2, "%IX0.1", "STOP", ContactMode::NormallyClosed),
                LadderElement::new_coil(3, "%QX0.0", "MOTOR", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(report.can_compile());
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn tc09_unconditional_backward_jump_cycle_fails_r13() {
        let lbl = LadderElement {
            id: 1,
            element_type: TypeTag::Label,
            name: "LOOP_START".to_string(),
            ..Default::default()
        };
        let jmp = LadderElement {
            id: 3,
            element_type: TypeTag::Jump,
            name: "LOOP_START".to_string(),
            ..Default::default()
        };
        let p = prog_with_rungs(vec![
            rung_with(
                0,
                vec![
                    lbl,
                    LadderElement::new_contact(2, "%IX0.0", "A", ContactMode::NormallyOpen),
                    LadderElement::new_coil(4, "%QX0.0", "Q1", CoilMode::Normal),
                ],
            ),
            rung_with(1, vec![jmp]),
        ]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R13" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn tc10_duplicate_normal_coil_warns_r15() {
        let p = prog_with_rungs(vec![
            rung_with(
                0,
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                    LadderElement::new_coil(2, "%MX0.0", "M0", CoilMode::Normal),
                ],
            ),
            rung_with(
                1,
                vec![
                    LadderElement::new_contact(3, "%IX0.1", "B", ContactMode::NormallyOpen),
                    LadderElement::new_coil(4, "M0", "M0", CoilMode::Normal), // canonicalizes to %MX0.0
                ],
            ),
        ]);
        let report = validate_program(&p);
        assert!(report.can_compile()); // Warning only for internal bit
        assert_eq!(report.warning_count(), 2);
        assert!(report
            .issues
            .iter()
            .all(|i| i.rule_id == "R15" && i.severity == ValidationSeverity::Warning));
    }

    #[test]
    fn tc11_contact_with_word_register_fails_r16() {
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%MW0", "INT_TAG", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R16" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn tc12_function_block_missing_required_param_fails_r17() {
        let mut math_el = LadderElement::default();
        math_el.id = 2;
        math_el.element_type = TypeTag::Math;
        math_el.name = "D0".to_string();
        math_el.target_reg = String::new(); // missing required target_reg
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                math_el,
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R17" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn tc13_unknown_function_block_signature_reports_unknown_l08() {
        let mut cmp_el = LadderElement::default();
        cmp_el.id = 1;
        cmp_el.element_type = TypeTag::Compare;
        cmp_el.name = "%MW0".to_string();
        cmp_el.compare_op = "???".to_string();
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                cmp_el,
                LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "L08" && i.severity == ValidationSeverity::Unknown));
    }

    #[test]
    fn tc14_bypass_wire_across_condition_warns_r14() {
        let br = LadderElement::new_branch(
            10,
            vec![
                vec![LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen)],
                vec![LadderElement::new_wire(2)],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                br,
                LadderElement::new_coil(3, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R14" && i.severity == ValidationSeverity::Warning));
    }

    #[test]
    fn tc15_open_circuit_disconnected_element_fails_r11() {
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_open(1),
                LadderElement::new_coil(2, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R11" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn tc16_nested_contact_branches_to_single_coil_passes() {
        let inner_br = LadderElement::new_branch(
            11,
            vec![
                vec![LadderElement::new_contact(2, "%IX0.1", "B", ContactMode::NormallyOpen)],
                vec![LadderElement::new_contact(3, "%IX0.2", "C", ContactMode::NormallyOpen)],
            ],
        );
        let outer_br = LadderElement::new_branch(
            10,
            vec![
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                    inner_br,
                ],
                vec![LadderElement::new_contact(4, "%IX0.3", "D", ContactMode::NormallyOpen)],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                outer_br,
                LadderElement::new_coil(5, "%QX0.0", "Q1", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(report.can_compile());
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.warning_count(), 0);
    }

    #[test]
    fn demo_program_compiles_cleanly() {
        let p = LadderProgram::new_demo_program();
        let report = validate_program(&p);
        assert!(
            report.can_compile(),
            "Demo program should have 0 errors, got: {:?}",
            report.issues
        );
    }

    #[test]
    fn r21_coil_before_branch_merge_with_trailing_coil_fails() {
        // |----+----[ IX0.0 ]----( QX0.1 )----+----( QX0.0 )----|
        //      |                              |
        //      +----[ IX0.0 ]-----------------+
        let br = LadderElement::new_branch(
            10,
            vec![
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                    LadderElement::new_coil(2, "%QX0.1", "Q1", CoilMode::Normal),
                ],
                vec![LadderElement::new_contact(
                    3,
                    "%IX0.0",
                    "A",
                    ContactMode::NormallyOpen,
                )],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                br,
                LadderElement::new_coil(4, "%QX0.0", "Q0", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        let r21 = report
            .issues
            .iter()
            .find(|i| i.rule_id == "R21" && i.severity == ValidationSeverity::Error)
            .expect("expected R21 ERROR");
        assert!(r21.element_ids.contains(&2));
        assert!(r21.element_ids.contains(&4));
    }

    #[test]
    fn r21_coil_before_branch_merge_with_trailing_contact_fails() {
        let br = LadderElement::new_branch(
            10,
            vec![
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                    LadderElement::new_coil(2, "%QX0.1", "Q1", CoilMode::SetLatch),
                ],
                vec![LadderElement::new_contact(
                    3,
                    "%IX0.1",
                    "B",
                    ContactMode::NormallyOpen,
                )],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                br,
                LadderElement::new_contact(4, "%IX0.2", "C", ContactMode::NormallyOpen),
                LadderElement::new_coil(5, "%QX0.0", "Q0", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R21" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn r21_nested_branch_coil_before_outer_merge_fails() {
        let inner = LadderElement::new_branch(
            11,
            vec![
                vec![LadderElement::new_coil(2, "%QX0.1", "Q1", CoilMode::Normal)],
                vec![LadderElement::new_contact(
                    3,
                    "%IX0.1",
                    "B",
                    ContactMode::NormallyOpen,
                )],
            ],
        );
        let outer = LadderElement::new_branch(
            10,
            vec![
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                    inner,
                ],
                vec![LadderElement::new_contact(
                    4,
                    "%IX0.2",
                    "C",
                    ContactMode::NormallyOpen,
                )],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                outer,
                LadderElement::new_coil(5, "%QX0.0", "Q0", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        let r21 = report
            .issues
            .iter()
            .find(|i| i.rule_id == "R21" && i.severity == ValidationSeverity::Error)
            .expect("expected R21 ERROR on nested branch coil");
        assert!(r21.element_ids.contains(&2));
        assert!(r21.element_ids.contains(&5));
    }

    #[test]
    fn r21_parallel_coils_to_right_rail_passes() {
        let out_br = LadderElement::new_branch(
            10,
            vec![
                vec![
                    LadderElement::new_contact(2, "%IX0.1", "B", ContactMode::NormallyOpen),
                    LadderElement::new_coil(3, "%QX0.0", "Q0", CoilMode::Normal),
                ],
                vec![
                    LadderElement::new_contact(4, "%IX0.2", "C", ContactMode::NormallyOpen),
                    LadderElement::new_coil(5, "%QX0.1", "Q1", CoilMode::Normal),
                ],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                out_br,
            ],
        )]);
        let report = validate_program(&p);
        assert!(report.can_compile(), "unexpected errors: {:?}", report.issues);
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn r21_mixed_terminal_branch_missing_output_fails() {
        // Terminal branch: top path ends in Coil, bottom path ends in Contact (missing terminal output)
        let mixed_br = LadderElement::new_branch(
            10,
            vec![
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                    LadderElement::new_coil(2, "%QX0.1", "Q1", CoilMode::Normal),
                ],
                vec![LadderElement::new_contact(
                    3,
                    "%IX0.1",
                    "B",
                    ContactMode::NormallyOpen,
                )],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(0, vec![mixed_br])]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R21" && i.severity == ValidationSeverity::Error && i.element_ids.contains(&3)));
    }

    #[test]
    fn r25_coil_free_bypass_wire_to_right_rail_fails() {
        let br = LadderElement::new_branch(
            10,
            vec![
                vec![LadderElement::new_coil(2, "%QX0.0", "Q0", CoilMode::Normal)],
                vec![LadderElement::new_wire(3)],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                br,
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R25" && i.severity == ValidationSeverity::Error && i.element_ids.contains(&3)));
        assert!(!report.structural_errors().is_empty());
    }

    #[test]
    fn r26_coil_writing_physical_input_fails() {
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%IX0.1", "BAD_IN", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R26" && i.severity == ValidationSeverity::Error && i.element_ids.contains(&2)));
    }

    #[test]
    fn r27_io_location_overlap_errors_on_output_warns_on_input() {
        use crate::ladder::IOPortMapping;
        let mut p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%QX0.0", "Q0", CoilMode::Normal),
            ],
        )]);
        p.io_mappings = vec![
            IOPortMapping {
                plc_address: "Y0".to_string(),
                iec_address: "%QX0.0".to_string(),
                symbol: "MOTOR_A".to_string(),
                hw_pin: "D8".to_string(),
                description: String::new(),
                is_input: false,
            },
            IOPortMapping {
                plc_address: "Y0".to_string(),
                iec_address: "%QX0.0".to_string(),
                symbol: "MOTOR_B".to_string(),
                hw_pin: "D9".to_string(),
                description: String::new(),
                is_input: false,
            },
            IOPortMapping {
                plc_address: "X0".to_string(),
                iec_address: "%IX0.0".to_string(),
                symbol: "IN_A".to_string(),
                hw_pin: "D2".to_string(),
                description: String::new(),
                is_input: true,
            },
            IOPortMapping {
                plc_address: "X0".to_string(),
                iec_address: "%IX0.0".to_string(),
                symbol: "IN_B".to_string(),
                hw_pin: "D3".to_string(),
                description: String::new(),
                is_input: true,
            },
        ];
        let report = validate_program(&p);
        assert!(!report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R27" && i.severity == ValidationSeverity::Error));
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R27" && i.severity == ValidationSeverity::Warning));
    }

    #[test]
    fn r29_physical_output_multiple_writers_and_sr_pair_policy() {
        // Duplicate Normal Coil on %QX0.0 -> R29 ERROR
        let p_dup = prog_with_rungs(vec![
            rung_with(
                0,
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                    LadderElement::new_coil(2, "%QX0.0", "Q0", CoilMode::Normal),
                ],
            ),
            rung_with(
                1,
                vec![
                    LadderElement::new_contact(3, "%IX0.1", "B", ContactMode::NormallyOpen),
                    LadderElement::new_coil(4, "Y0", "Q0", CoilMode::Normal),
                ],
            ),
        ]);
        let r_dup = validate_program(&p_dup);
        assert!(!r_dup.can_compile());
        assert!(r_dup
            .issues
            .iter()
            .any(|i| i.rule_id == "R29" && i.severity == ValidationSeverity::Error));

        // Pure SetLatch + ResetUnlatch pair on %QX0.0 -> R18 WARNING (allowed to compile!)
        let p_sr = prog_with_rungs(vec![
            rung_with(
                0,
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "START", ContactMode::NormallyOpen),
                    LadderElement::new_coil(2, "%QX0.0", "MOTOR", CoilMode::SetLatch),
                ],
            ),
            rung_with(
                1,
                vec![
                    LadderElement::new_contact(3, "%IX0.1", "STOP", ContactMode::NormallyOpen),
                    LadderElement::new_coil(4, "%QX0.0", "MOTOR", CoilMode::ResetUnlatch),
                ],
            ),
        ]);
        let r_sr = validate_program(&p_sr);
        assert!(r_sr.can_compile(), "pure S/R pair on %QX0.0 should warn (R18), not error");
        assert!(r_sr
            .issues
            .iter()
            .any(|i| i.rule_id == "R18" && i.severity == ValidationSeverity::Warning));

        // Mixing Normal Coil + SetLatch on %QX0.0 -> R29 ERROR
        let p_mix = prog_with_rungs(vec![
            rung_with(
                0,
                vec![
                    LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                    LadderElement::new_coil(2, "%QX0.0", "Q0", CoilMode::Normal),
                ],
            ),
            rung_with(
                1,
                vec![
                    LadderElement::new_contact(3, "%IX0.1", "B", ContactMode::NormallyOpen),
                    LadderElement::new_coil(4, "%QX0.0", "Q0", CoilMode::SetLatch),
                ],
            ),
        ]);
        let r_mix = validate_program(&p_mix);
        assert!(!r_mix.can_compile());
        assert!(r_mix
            .issues
            .iter()
            .any(|i| i.rule_id == "R29" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn r31_symbol_resolution_and_reserved_keywords() {
        // Undeclared symbol -> R31 ERROR
        let p_undef = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "MY_UNDECLARED_BTN", "BTN", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%QX0.0", "Q0", CoilMode::Normal),
            ],
        )]);
        let r_undef = validate_program(&p_undef);
        assert!(!r_undef.can_compile());
        assert!(r_undef
            .issues
            .iter()
            .any(|i| i.rule_id == "R31" && i.severity == ValidationSeverity::Error));

        // Declared symbol in GlobalSymbolTable -> PASS
        let mut p_decl = p_undef.clone();
        p_decl.symbols.set_symbol("%IX0.0", "MY_UNDECLARED_BTN", None);
        let r_decl = validate_program(&p_decl);
        assert!(r_decl.can_compile(), "declared symbol should pass: {:?}", r_decl.issues);

        // Reserved IEC keyword -> R31 ERROR
        let p_kw = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "AND", "AND", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%QX0.0", "Q0", CoilMode::Normal),
            ],
        )]);
        let r_kw = validate_program(&p_kw);
        assert!(!r_kw.can_compile());
        assert!(r_kw
            .issues
            .iter()
            .any(|i| i.rule_id == "R31" && i.severity == ValidationSeverity::Error));

        // Duplicate symbol mapped to two different addresses -> R31 ERROR
        let mut p_dup_sym = prog_with_rungs(vec![rung_with(
            0,
            vec![
                LadderElement::new_contact(1, "%IX0.0", "A", ContactMode::NormallyOpen),
                LadderElement::new_coil(2, "%QX0.0", "Q0", CoilMode::Normal),
            ],
        )]);
        p_dup_sym.symbols.set_symbol("%IX0.0", "DUP_TAG", None);
        p_dup_sym.symbols.set_symbol("%IX0.1", "DUP_TAG", None);
        let r_dup_sym = validate_program(&p_dup_sym);
        assert!(!r_dup_sym.can_compile());
        assert!(r_dup_sym
            .issues
            .iter()
            .any(|i| i.rule_id == "R31" && i.severity == ValidationSeverity::Error));
    }

    #[test]
    fn r34_self_hold_seal_in_feedback_passes_cleanly() {
        // |----+--[ START ]--+--[/STOP]--( MOTOR )----|
        //      +--[ MOTOR ]--+
        let latch_br = LadderElement::new_branch(
            10,
            vec![
                vec![LadderElement::new_contact(1, "%IX0.0", "START", ContactMode::NormallyOpen)],
                vec![LadderElement::new_contact(2, "%QX0.0", "MOTOR", ContactMode::NormallyOpen)],
            ],
        );
        let p = prog_with_rungs(vec![rung_with(
            0,
            vec![
                latch_br,
                LadderElement::new_contact(3, "%IX0.1", "STOP", ContactMode::NormallyClosed),
                LadderElement::new_coil(4, "%QX0.0", "MOTOR", CoilMode::Normal),
            ],
        )]);
        let report = validate_program(&p);
        assert!(report.can_compile(), "self-hold must pass cleanly: {:?}", report.issues);
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.warning_count(), 0);
    }

    #[test]
    fn r38_complexity_limit_over_32_elements_warns() {
        let mut elems = Vec::new();
        for i in 1..=33u64 {
            elems.push(LadderElement::new_contact(
                i,
                "%IX0.0",
                "A",
                ContactMode::NormallyOpen,
            ));
        }
        elems.push(LadderElement::new_coil(100, "%QX0.0", "Q0", CoilMode::Normal));
        let p = prog_with_rungs(vec![rung_with(0, elems)]);
        let report = validate_program(&p);
        assert!(report.can_compile());
        assert!(report
            .issues
            .iter()
            .any(|i| i.rule_id == "R38" && i.severity == ValidationSeverity::Warning));
    }
}
