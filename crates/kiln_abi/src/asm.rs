//! GNU inline-asm target rules (spec/abi.md §asm): constraint validation
//! and simplification, clobber register names, and the assembler-template
//! analysis that sema diagnoses and codegen lowers to LLVM inline asm.
//! Ports of clang 19's `TargetInfo::validateOutputConstraint`,
//! `validateInputConstraint`, the `AArch64` and X86 `validateAsmConstraint`,
//! `convertConstraint`, `isValidGCCRegisterName`,
//! `getNormalizedGCCRegisterName`, `GCCAsmStmt::AnalyzeAsmString` and
//! `SimplifyConstraint` (CGStmt.cpp).

use crate::target::Arch;

/// What a constraint allows (clang's `TargetInfo::ConstraintInfo`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ConstraintInfo {
    /// The constraint as written.
    pub constraint: String,
    /// The operand's symbolic name (`[name]`), empty for none.
    pub name: String,
    flags: u8,
    tied: Option<u32>,
    imm: Option<ImmRange>,
}

/// The values an immediate constraint accepts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImmRange {
    /// Any value (`n`).
    Any,
    /// `min..=max`.
    Range(i64, i64),
    /// One of a set of values.
    Set(&'static [i64]),
}

const READ_WRITE: u8 = 1;
const ALLOWS_REGISTER: u8 = 2;
const ALLOWS_MEMORY: u8 = 4;
const EARLY_CLOBBER: u8 = 8;
const MATCHING_INPUT: u8 = 16;

impl ConstraintInfo {
    /// A fresh info for `constraint` of the operand named `name`.
    pub fn new(constraint: &str, name: &str) -> Self {
        ConstraintInfo {
            constraint: constraint.to_owned(),
            name: name.to_owned(),
            ..Self::default()
        }
    }
    /// `+`: the output is also read.
    pub fn is_read_write(&self) -> bool {
        self.flags & READ_WRITE != 0
    }
    /// The operand may be in a register.
    pub fn allows_register(&self) -> bool {
        self.flags & ALLOWS_REGISTER != 0
    }
    /// The operand may be in memory.
    pub fn allows_memory(&self) -> bool {
        self.flags & ALLOWS_MEMORY != 0
    }
    /// `&`.
    pub fn early_clobber(&self) -> bool {
        self.flags & EARLY_CLOBBER != 0
    }
    /// An input is tied to this output.
    pub fn has_matching_input(&self) -> bool {
        self.flags & MATCHING_INPUT != 0
    }
    /// The output this input is tied to.
    pub fn tied_operand(&self) -> Option<u32> {
        self.tied
    }
    /// The immediate values this operand requires (`n`, x86 `I`..`O`).
    pub fn requires_immediate(&self) -> Option<ImmRange> {
        self.imm
    }
    /// True if `v` is acceptable for a [`requires_immediate`](Self::requires_immediate) operand.
    pub fn is_valid_immediate(&self, v: i128) -> bool {
        match self.imm {
            None | Some(ImmRange::Any) => true,
            Some(ImmRange::Range(lo, hi)) => v >= i128::from(lo) && v <= i128::from(hi),
            Some(ImmRange::Set(s)) => {
                i32::try_from(v).is_ok() && s.iter().any(|&x| i128::from(x) == i128::from(v as u32))
            }
        }
    }
    fn set(&mut self, f: u8) {
        self.flags |= f;
    }
    fn tie_to(&mut self, i: u32, outputs: &mut [ConstraintInfo]) {
        outputs[i as usize].set(MATCHING_INPUT);
        self.flags = outputs[i as usize].flags;
        self.tied = Some(i);
    }
}

/// The target's `matchAsmCCConstraint`: the length of the flag-output
/// constraint (`@cc<cond>`) that is all of `rest`, or 0.
fn match_asm_cc(arch: Arch, rest: &[u8]) -> usize {
    let Some(cond) = rest.strip_prefix(b"@cc") else { return 0 };
    let conds: &[&[u8]] = match arch {
        Arch::AArch64 => &[
            b"eq", b"ne", b"hs", b"cs", b"cc", b"lo", b"mi", b"pl", b"vs", b"vc", b"hi", b"ls",
            b"ge", b"lt", b"gt", b"le",
        ],
        Arch::X86_64 => &[
            b"a", b"ae", b"b", b"be", b"c", b"e", b"z", b"g", b"ge", b"l", b"le", b"na", b"nae",
            b"nb", b"nbe", b"nc", b"ne", b"nz", b"ng", b"nge", b"nl", b"nle", b"no", b"np", b"ns",
            b"o", b"p", b"s",
        ],
    };
    if conds.contains(&cond) { rest.len() } else { 0 }
}

/// `validateAsmConstraint` for the letter(s) at `c[*i..]`; may advance `i`
/// over a multi-letter constraint.
fn validate_target(arch: Arch, c: &[u8], i: &mut usize, info: &mut ConstraintInfo) -> bool {
    let at = |k: usize| c.get(*i + k).copied().unwrap_or(0);
    if at(0) == b'@' {
        // A condition-flag output (`=@cc<cond>`).
        let len = match_asm_cc(arch, &c[*i..]);
        if len == 0 {
            return false;
        }
        *i += len - 1;
        info.set(ALLOWS_REGISTER);
        return true;
    }
    match arch {
        Arch::AArch64 => match at(0) {
            b'w' | b'S' | b'z' | b'x' | b'y' => {
                info.set(ALLOWS_REGISTER);
                true
            }
            b'I' | b'J' | b'K' | b'L' | b'M' | b'N' | b'Y' | b'Z' => true,
            b'Q' => {
                info.set(ALLOWS_MEMORY);
                true
            }
            b'U' if (at(1) == b'p' && matches!(at(2), b'l' | b'a' | b'h'))
                || (at(1) == b'c' && matches!(at(2), b'i' | b'j')) =>
            {
                info.set(ALLOWS_REGISTER);
                *i += 2;
                true
            }
            _ => false,
        },
        Arch::X86_64 => match at(0) {
            b'e' | b'Z' | b'C' | b'G' => true,
            b'f' => {
                if info.constraint.starts_with('=') {
                    return false;
                }
                info.set(ALLOWS_REGISTER);
                true
            }
            b'a' | b'b' | b'c' | b'd' | b'S' | b'D' | b'A' | b't' | b'u' | b'y' | b'x' | b'v'
            | b'l' | b'q' | b'Q' | b'R' | b'k' => {
                info.set(ALLOWS_REGISTER);
                true
            }
            b'I' => imm(info, ImmRange::Range(0, 31)),
            b'J' => imm(info, ImmRange::Range(0, 63)),
            b'K' => imm(info, ImmRange::Range(-128, 127)),
            b'L' => imm(info, ImmRange::Set(&[0xff, 0xffff, 0xffff_ffff])),
            b'M' => imm(info, ImmRange::Range(0, 3)),
            b'N' => imm(info, ImmRange::Range(0, 255)),
            b'O' => imm(info, ImmRange::Range(0, 127)),
            b'W' if at(1) == b's' => {
                *i += 1;
                info.set(ALLOWS_REGISTER);
                true
            }
            b'Y' if matches!(at(1), b'z' | b'0' | b'i' | b't' | b'2' | b'm' | b'k' | b'd') => {
                *i += 1;
                info.set(ALLOWS_REGISTER);
                true
            }
            _ => false,
        },
    }
}

fn imm(info: &mut ConstraintInfo, r: ImmRange) -> bool {
    info.imm = Some(r);
    true
}

/// clang's `TargetInfo::validateOutputConstraint`.
pub fn validate_output_constraint(arch: Arch, info: &mut ConstraintInfo) -> bool {
    let c = info.constraint.clone().into_bytes();
    match c.first() {
        Some(b'=') => {}
        Some(b'+') => info.set(READ_WRITE),
        _ => return false,
    }
    let mut i = 1;
    while i < c.len() {
        match c[i] {
            b'&' => info.set(EARLY_CLOBBER),
            b'%' | b'?' | b'!' | b'*' | b'i' | b'n' | b'E' | b'F' => {}
            b'r' => info.set(ALLOWS_REGISTER),
            b'm' | b'o' | b'V' | b'<' | b'>' => info.set(ALLOWS_MEMORY),
            b'g' | b'X' => info.set(ALLOWS_REGISTER | ALLOWS_MEMORY),
            b',' => {
                if matches!(c.get(i + 1), Some(b'=' | b'+')) {
                    i += 1;
                }
            }
            b'#' => {
                while i + 1 < c.len() && c[i + 1] != b',' {
                    i += 1;
                }
            }
            _ => {
                if !validate_target(arch, &c, &mut i, info) {
                    return false;
                }
            }
        }
        i += 1;
    }
    if info.early_clobber() && info.is_read_write() && !info.allows_register() {
        return false;
    }
    info.allows_memory() || info.allows_register()
}

/// clang's `resolveSymbolicName` at `c[*i] == '['`: the output named
/// there; `i` ends at the `]`.
fn resolve_symbolic_name(c: &[u8], i: &mut usize, outputs: &[ConstraintInfo]) -> Option<u32> {
    let start = *i + 1;
    let end = start + c[start..].iter().position(|&b| b == b']')?;
    *i = end;
    let name = &c[start..end];
    outputs.iter().position(|o| o.name.as_bytes() == name).map(|p| p as u32)
}

/// clang's `TargetInfo::validateInputConstraint`.
pub fn validate_input_constraint(
    arch: Arch,
    outputs: &mut [ConstraintInfo],
    info: &mut ConstraintInfo,
) -> bool {
    let c = info.constraint.clone().into_bytes();
    if c.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            b'0'..=b'9' => {
                let start = i;
                while i + 1 < c.len() && c[i + 1].is_ascii_digit() {
                    i += 1;
                }
                let Some(n) =
                    std::str::from_utf8(&c[start..=i]).ok().and_then(|s| s.parse::<u32>().ok())
                else {
                    return false;
                };
                if n as usize >= outputs.len()
                    || outputs[n as usize].is_read_write()
                    || info.tied.is_some_and(|t| t != n)
                {
                    return false;
                }
                info.tie_to(n, outputs);
            }
            b'[' => {
                let Some(n) = resolve_symbolic_name(&c, &mut i, outputs) else { return false };
                if info.tied.is_some_and(|t| t != n) || outputs[n as usize].is_read_write() {
                    return false;
                }
                info.tie_to(n, outputs);
            }
            b'%' | b'i' | b'E' | b'F' | b'p' | b',' | b'?' | b'!' | b'*' => {}
            b'n' => info.imm = Some(ImmRange::Any),
            b'r' => info.set(ALLOWS_REGISTER),
            b'm' | b'o' | b'V' | b'<' | b'>' => info.set(ALLOWS_MEMORY),
            b'g' | b'X' => info.set(ALLOWS_REGISTER | ALLOWS_MEMORY),
            b'#' => {
                while i + 1 < c.len() && c[i + 1] != b',' {
                    i += 1;
                }
            }
            _ => {
                if !validate_target(arch, &c, &mut i, info) {
                    return false;
                }
            }
        }
        i += 1;
    }
    true
}

/// clang's `validateConstraintModifier`: `Err(suggestion)` when an operand
/// of `size` bits printed with `modifier` (0 for none) does not fit the
/// register class of `constraint`.
pub fn validate_constraint_modifier(
    arch: Arch,
    constraint: &str,
    modifier: u8,
    size: u64,
) -> Result<(), Option<&'static str>> {
    if arch != Arch::AArch64 {
        return Ok(());
    }
    let c = constraint.trim_start_matches(['=', '+', '&']);
    match c.as_bytes().first() {
        Some(b'z' | b'r') if !matches!(modifier, b'x' | b'w') && size != 64 => Err(Some("w")),
        _ => Ok(()),
    }
}

const AARCH64_REG_NAMES: &[&str] = &[
    "w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10", "w11", "w12", "w13", "w14",
    "w15", "w16", "w17", "w18", "w19", "w20", "w21", "w22", "w23", "w24", "w25", "w26", "w27",
    "w28", "w29", "w30", "wsp", "x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10",
    "x11", "x12", "x13", "x14", "x15", "x16", "x17", "x18", "x19", "x20", "x21", "x22", "x23",
    "x24", "x25", "x26", "x27", "x28", "fp", "lr", "sp", "s0", "s1", "s2", "s3", "s4", "s5", "s6",
    "s7", "s8", "s9", "s10", "s11", "s12", "s13", "s14", "s15", "s16", "s17", "s18", "s19", "s20",
    "s21", "s22", "s23", "s24", "s25", "s26", "s27", "s28", "s29", "s30", "s31", "d0", "d1", "d2",
    "d3", "d4", "d5", "d6", "d7", "d8", "d9", "d10", "d11", "d12", "d13", "d14", "d15", "d16",
    "d17", "d18", "d19", "d20", "d21", "d22", "d23", "d24", "d25", "d26", "d27", "d28", "d29",
    "d30", "d31", "v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12",
    "v13", "v14", "v15", "v16", "v17", "v18", "v19", "v20", "v21", "v22", "v23", "v24", "v25",
    "v26", "v27", "v28", "v29", "v30", "v31", "z0", "z1", "z2", "z3", "z4", "z5", "z6", "z7", "z8",
    "z9", "z10", "z11", "z12", "z13", "z14", "z15", "z16", "z17", "z18", "z19", "z20", "z21",
    "z22", "z23", "z24", "z25", "z26", "z27", "z28", "z29", "z30", "z31", "p0", "p1", "p2", "p3",
    "p4", "p5", "p6", "p7", "p8", "p9", "p10", "p11", "p12", "p13", "p14", "p15", "pn0", "pn1",
    "pn2", "pn3", "pn4", "pn5", "pn6", "pn7", "pn8", "pn9", "pn10", "pn11", "pn12", "pn13", "pn14",
    "pn15", "za", "zt0",
];

/// `AArch64` `GCCRegAliases`: `(alias, register)`.
fn aarch64_alias(name: &str) -> Option<&'static str> {
    Some(match name {
        "w31" => "wsp",
        "x31" => "sp",
        "r29" | "x29" => "fp",
        "r30" | "x30" => "lr",
        _ => {
            let n: u32 = name.strip_prefix('r')?.parse().ok()?;
            if n > 28 || name.len() > 3 || name.starts_with("r0") && name.len() > 2 {
                return None;
            }
            AARCH64_REG_NAMES[32 + n as usize]
        }
    })
}

const X86_REG_NAMES: &[&str] = &[
    "ax", "dx", "cx", "bx", "si", "di", "bp", "sp", "st", "st(1)", "st(2)", "st(3)", "st(4)",
    "st(5)", "st(6)", "st(7)", "argp", "flags", "fpcr", "fpsr", "dirflag", "frame", "xmm0", "xmm1",
    "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7", "mm0", "mm1", "mm2", "mm3", "mm4", "mm5",
    "mm6", "mm7", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15", "xmm8", "xmm9", "xmm10",
    "xmm11", "xmm12", "xmm13", "xmm14", "xmm15", "ymm0", "ymm1", "ymm2", "ymm3", "ymm4", "ymm5",
    "ymm6", "ymm7", "ymm8", "ymm9", "ymm10", "ymm11", "ymm12", "ymm13", "ymm14", "ymm15", "xmm16",
    "xmm17", "xmm18", "xmm19", "xmm20", "xmm21", "xmm22", "xmm23", "xmm24", "xmm25", "xmm26",
    "xmm27", "xmm28", "xmm29", "xmm30", "xmm31", "ymm16", "ymm17", "ymm18", "ymm19", "ymm20",
    "ymm21", "ymm22", "ymm23", "ymm24", "ymm25", "ymm26", "ymm27", "ymm28", "ymm29", "ymm30",
    "ymm31", "zmm0", "zmm1", "zmm2", "zmm3", "zmm4", "zmm5", "zmm6", "zmm7", "zmm8", "zmm9",
    "zmm10", "zmm11", "zmm12", "zmm13", "zmm14", "zmm15", "zmm16", "zmm17", "zmm18", "zmm19",
    "zmm20", "zmm21", "zmm22", "zmm23", "zmm24", "zmm25", "zmm26", "zmm27", "zmm28", "zmm29",
    "zmm30", "zmm31", "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "cr0", "cr2", "cr3", "cr4",
    "cr8", "dr0", "dr1", "dr2", "dr3", "dr6", "dr7", "bnd0", "bnd1", "bnd2", "bnd3", "tmm0",
    "tmm1", "tmm2", "tmm3", "tmm4", "tmm5", "tmm6", "tmm7", "r16", "r17", "r18", "r19", "r20",
    "r21", "r22", "r23", "r24", "r25", "r26", "r27", "r28", "r29", "r30", "r31",
];

/// X86 `AddlRegNames` (valid, kept as written by normalization).
fn x86_addl(name: &str) -> bool {
    const FIXED: &[&str] = &[
        "al", "ah", "eax", "rax", "bl", "bh", "ebx", "rbx", "cl", "ch", "ecx", "rcx", "dl", "dh",
        "edx", "rdx", "esi", "rsi", "edi", "rdi", "esp", "rsp", "ebp", "rbp",
    ];
    if FIXED.contains(&name) {
        return true;
    }
    // r8d/r8w/r8b .. r31d/r31w/r31b
    let Some(rest) = name.strip_prefix('r') else { return false };
    let (num, suf) = rest.split_at(rest.len().saturating_sub(1));
    matches!(suf, "d" | "w" | "b")
        && num.parse::<u32>().is_ok_and(|n| (8..=31).contains(&n) && !num.starts_with('0'))
}

fn reg_names(arch: Arch) -> &'static [&'static str] {
    match arch {
        Arch::AArch64 => AARCH64_REG_NAMES,
        Arch::X86_64 => X86_REG_NAMES,
    }
}

fn strip_reg_prefix(name: &str) -> &str {
    name.strip_prefix(['%', '#']).unwrap_or(name)
}

/// clang's `isValidGCCRegisterName`.
pub fn is_valid_register_name(arch: Arch, name: &str) -> bool {
    let name = strip_reg_prefix(name);
    if name.is_empty() {
        return false;
    }
    let names = reg_names(arch);
    if name.as_bytes()[0].is_ascii_digit()
        && let Some(n) = parse_c_integer(name)
    {
        return n < names.len() as u64;
    }
    if names.contains(&name) {
        return true;
    }
    match arch {
        Arch::AArch64 => aarch64_alias(name).is_some(),
        Arch::X86_64 => x86_addl(name),
    }
}

/// clang 19's `validateGlobalRegisterVariable` for a GNU global register
/// variable named `name` (as written, after [`is_valid_register_name`]
/// accepted it) of `size_bits` bits: `None` when the target cannot hold a
/// global variable in that register, else whether the sizes differ.
/// `AArch64`: `sp` and the `x` registers are 64 bits, the `w` registers 32;
/// `x86_64`: `rsp`/`rbp` are 64 bits, `esp`/`ebp` 32; nothing else.
pub fn global_register_size_mismatch(arch: Arch, name: &str, size_bits: u64) -> Option<bool> {
    let reg_bits = match arch {
        Arch::AArch64 if name == "sp" || name.starts_with('x') => 64,
        Arch::AArch64 if name.starts_with('w') => 32,
        Arch::X86_64 if name == "rsp" || name == "rbp" => 64,
        Arch::X86_64 if name == "esp" || name == "ebp" => 32,
        _ => return None,
    };
    Some(size_bits != reg_bits)
}

/// clang 22's `AArch64TargetInfo::validateGlobalRegisterVariable` also
/// requires an `x`/`w` register to be reserved: register 0 always, x18
/// only on targets that reserve it by default (none of kiln's Linux
/// targets), others only under `+reserve-xN` (which kiln does not accept).
/// clang 19 accepts every `x`/`w` register. `true` when `name` (already a
/// size-checked global register) may hold a variable at clang 22.
pub fn global_register_reserved_22(arch: Arch, name: &str) -> bool {
    match arch {
        Arch::AArch64 => name == "sp" || name == "x0" || name == "w0",
        Arch::X86_64 => true,
    }
}

/// `StringRef::getAsInteger(0, n)`: decimal, `0x` hex, `0b` binary or a
/// leading `0` octal.
fn parse_c_integer(s: &str) -> Option<u64> {
    let (radix, digits) = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (16, h)
    } else if let Some(b) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        (2, b)
    } else if s.len() > 1 && s.starts_with('0') {
        (8, &s[1..])
    } else {
        (10, s)
    };
    u64::from_str_radix(digits, radix).ok()
}

/// clang's `isValidClobber`.
pub fn is_valid_clobber(arch: Arch, name: &str) -> bool {
    is_valid_register_name(arch, name) || matches!(name, "memory" | "cc" | "unwind")
}

/// clang's `getNormalizedGCCRegisterName` (not canonical) of a valid
/// register name.
pub fn normalized_register_name(arch: Arch, name: &str) -> String {
    let name = strip_reg_prefix(name);
    let names = reg_names(arch);
    if name.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && let Some(n) = parse_c_integer(name)
        && let Some(r) = names.get(n as usize)
    {
        return (*r).to_owned();
    }
    if arch == Arch::AArch64
        && let Some(r) = aarch64_alias(name)
    {
        return r.to_owned();
    }
    name.to_owned()
}

/// The clobbers the target adds to every inline asm (clang's
/// `getClobbers`).
pub fn machine_clobbers(arch: Arch) -> &'static str {
    match arch {
        Arch::AArch64 => "",
        Arch::X86_64 => "~{dirflag},~{fpsr},~{flags}",
    }
}

/// True if the target's assembler has `{a|b}` dialect variants (clang's
/// `!hasNoAsmVariants`).
pub fn has_asm_variants(arch: Arch) -> bool {
    arch == Arch::X86_64
}

/// `convertConstraint` for the letter(s) at `c[*i..]`.
fn convert_constraint(arch: Arch, c: &[u8], i: &mut usize, out: &mut String) {
    let ch = c[*i];
    if ch == b'@' {
        let len = match_asm_cc(arch, &c[*i..]);
        if len == 0 {
            out.push('@');
        } else {
            out.push('{');
            out.push_str(&String::from_utf8_lossy(&c[*i..*i + len]));
            out.push('}');
            *i += len - 1;
        }
        return;
    }
    match arch {
        Arch::AArch64 => {
            if ch == b'U' && *i + 2 < c.len() {
                out.push_str("@3");
                out.push_str(&String::from_utf8_lossy(&c[*i..*i + 3]));
                *i += 2;
            } else if ch == b'p' {
                out.push('r');
            } else {
                out.push(ch as char);
            }
        }
        Arch::X86_64 => match ch {
            b'a' => out.push_str("{ax}"),
            b'b' => out.push_str("{bx}"),
            b'c' => out.push_str("{cx}"),
            b'd' => out.push_str("{dx}"),
            b'S' => out.push_str("{si}"),
            b'D' => out.push_str("{di}"),
            b't' => out.push_str("{st}"),
            b'u' => out.push_str("{st(1)}"),
            b'Y' if matches!(c.get(*i + 1), Some(b'k' | b'm' | b'i' | b't' | b'z' | b'2')) => {
                out.push('^');
                out.push('Y');
                out.push(c[*i + 1] as char);
                *i += 1;
            }
            _ => out.push(ch as char),
        },
    }
}

/// clang's `SimplifyConstraint` (CGStmt.cpp): the LLVM spelling of a
/// constraint (without its leading `=`/`+` for outputs). `[name]`
/// resolves against `outputs`.
pub fn simplify_constraint(arch: Arch, c: &str, outputs: &[ConstraintInfo]) -> String {
    let c = c.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            b'*' | b'?' | b'!' | b'=' | b'+' => {}
            b'#' => {
                while i + 1 < c.len() && c[i + 1] != b',' {
                    i += 1;
                }
            }
            b'&' | b'%' => {
                out.push(c[i] as char);
                while i + 1 < c.len() && c[i + 1] == c[i] {
                    i += 1;
                }
            }
            b',' => out.push('|'),
            b'g' => out.push_str("imr"),
            b'[' => {
                let n = resolve_symbolic_name(c, &mut i, outputs).unwrap_or(0);
                out.push_str(&n.to_string());
            }
            _ => convert_constraint(arch, c, &mut i, &mut out),
        }
        i += 1;
    }
    out
}

/// A piece of an analyzed assembler template.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AsmPiece {
    /// Literal text (already escaped for LLVM: `$` doubled, variants).
    Text(Vec<u8>),
    /// An operand reference `%[mod]N` / `%[mod][name]`.
    Operand {
        /// The operand number (LLVM numbering: outputs, inputs, then the
        /// input halves of `+` outputs).
        no: u32,
        /// The modifier letter, 0 for none.
        modifier: u8,
        /// Byte offset of the `%` in the template.
        percent: u32,
        /// Byte offset just past the reference.
        end: u32,
    },
}

/// Why a template is malformed (clang's `AnalyzeAsmString` diagnostics).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AsmStringError {
    /// `invalid % escape in inline assembly string`.
    InvalidEscape,
    /// `invalid operand number in inline asm string`.
    InvalidOperandNumber,
    /// `unterminated symbolic operand name in inline assembly string`.
    UnterminatedName,
    /// `empty symbolic operand name in inline assembly string`.
    EmptyName,
    /// `unknown symbolic operand name in inline assembly string`.
    UnknownName,
}

/// The operands a template may refer to.
#[derive(Clone, Copy, Debug)]
pub struct AsmOperandCounts<'a> {
    /// Outputs.
    pub outputs: u32,
    /// Outputs whose constraint starts with `+`.
    pub plus: u32,
    /// Inputs.
    pub inputs: u32,
    /// `asm goto` labels.
    pub labels: u32,
    /// The symbolic names of the outputs, inputs then labels (`""` for
    /// none).
    pub names: &'a [&'a str],
}

/// The operand named by `%[name]`, `p` just past the `[`; on success `p`
/// is advanced past the `]`.
fn named_operand(
    s: &[u8],
    p: &mut usize,
    ops: &AsmOperandCounts<'_>,
) -> Result<u32, (AsmStringError, u32)> {
    let off = (*p - 1) as u32;
    let Some(len) = s[*p..].iter().position(|&b| b == b']') else {
        return Err((AsmStringError::UnterminatedName, off));
    };
    if len == 0 {
        return Err((AsmStringError::EmptyName, off));
    }
    let name = &s[*p..*p + len];
    let Some(n) = ops.names.iter().position(|nm| !nm.is_empty() && nm.as_bytes() == name) else {
        return Err((AsmStringError::UnknownName, *p as u32));
    };
    *p += len + 1;
    // A label is numbered after the "+" operands' input halves too
    // (clang's `getNamedOperand`, as LLVM numbers `callbr` operands).
    let n = n as u32;
    Ok(if n >= ops.outputs + ops.inputs { n + ops.plus } else { n })
}

/// A simple asm string (no operands): only `$` is escaped, as `$$`.
fn escape_simple(s: &[u8]) -> Vec<u8> {
    let mut cur = Vec::with_capacity(s.len());
    for &b in s {
        if b == b'$' {
            cur.extend_from_slice(b"$$");
        } else {
            cur.push(b);
        }
    }
    cur
}

/// clang's `GCCAsmStmt::AnalyzeAsmString`: splits `s` into pieces, or the
/// error and the byte offset it is reported at. `simple` is a basic asm
/// (no `:`), where only `$` is escaped.
pub fn analyze_asm_string(
    arch: Arch,
    s: &[u8],
    simple: bool,
    ops: &AsmOperandCounts<'_>,
) -> Result<Vec<AsmPiece>, (AsmStringError, u32)> {
    if simple {
        return Ok(vec![AsmPiece::Text(escape_simple(s))]);
    }
    let mut pieces = Vec::new();
    let mut cur = Vec::new();
    let variants = has_asm_variants(arch);
    let mut p = 0usize;
    let end = s.len();
    loop {
        if p == end {
            if !cur.is_empty() {
                pieces.push(AsmPiece::Text(cur));
            }
            return Ok(pieces);
        }
        let ch = s[p];
        p += 1;
        match ch {
            b'$' => {
                cur.extend_from_slice("$$".as_bytes());
                continue;
            }
            b'{' => {
                cur.extend_from_slice(if variants { b"$(" } else { b"{" });
                continue;
            }
            b'|' => {
                cur.extend_from_slice(if variants { b"$|" } else { b"|" });
                continue;
            }
            b'}' => {
                cur.extend_from_slice(if variants { b"$)" } else { b"}" });
                continue;
            }
            b'%' => {}
            _ => {
                cur.push(ch);
                continue;
            }
        }
        if p == end {
            return Err((AsmStringError::InvalidEscape, (p - 1) as u32));
        }
        let mut esc = s[p];
        p += 1;
        match esc {
            b'%' | b'{' | b'}' => {
                cur.push(esc);
                continue;
            }
            b'=' => {
                cur.extend_from_slice("${:uid}".as_bytes());
                continue;
            }
            _ => {}
        }
        if !cur.is_empty() {
            pieces.push(AsmPiece::Text(std::mem::take(&mut cur)));
        }
        let percent = (p - 2) as u32;
        let mut modifier = 0u8;
        if esc.is_ascii_alphabetic() {
            if p == end {
                return Err((AsmStringError::InvalidEscape, (p - 1) as u32));
            }
            modifier = esc;
            esc = s[p];
            p += 1;
        }
        if esc.is_ascii_digit() {
            p -= 1;
            let mut n: u64 = 0;
            while p != end && s[p].is_ascii_digit() {
                n = n.saturating_mul(10).saturating_add(u64::from(s[p] - b'0'));
                p += 1;
            }
            let total = u64::from(ops.outputs + ops.plus + ops.inputs + ops.labels);
            if n >= total {
                return Err((AsmStringError::InvalidOperandNumber, (p - 1) as u32));
            }
            pieces.push(AsmPiece::Operand { no: n as u32, modifier, percent, end: p as u32 });
            continue;
        }
        if esc == b'[' {
            let n = named_operand(s, &mut p, ops)?;
            pieces.push(AsmPiece::Operand { no: n, modifier, percent, end: p as u32 });
            continue;
        }
        return Err((AsmStringError::InvalidEscape, (p - 1) as u32));
    }
}

/// clang's `generateAsmString`: the LLVM template of analyzed pieces.
pub fn generate_asm_string(pieces: &[AsmPiece]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in pieces {
        match p {
            AsmPiece::Text(t) => out.extend_from_slice(t),
            AsmPiece::Operand { no, modifier: 0, .. } => {
                out.push(b'$');
                out.extend_from_slice(no.to_string().as_bytes());
            }
            AsmPiece::Operand { no, modifier, .. } => {
                out.extend_from_slice(b"${");
                out.extend_from_slice(no.to_string().as_bytes());
                out.push(b':');
                out.push(*modifier);
                out.push(b'}');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(arch: Arch, c: &str) -> Option<ConstraintInfo> {
        let mut i = ConstraintInfo::new(c, "");
        validate_output_constraint(arch, &mut i).then_some(i)
    }

    #[test]
    fn output_constraints() {
        let i = out(Arch::AArch64, "+r,m").unwrap();
        assert!(i.is_read_write() && i.allows_register() && i.allows_memory());
        assert!(out(Arch::AArch64, "r").is_none());
        assert!(out(Arch::AArch64, "=").is_none());
        assert!(out(Arch::AArch64, "+&m").is_none());
        assert!(out(Arch::X86_64, "=a").unwrap().allows_register());
        assert!(out(Arch::X86_64, "=f").is_none());
    }

    #[test]
    fn input_constraints_tie() {
        let mut outs = vec![out(Arch::AArch64, "=r").unwrap()];
        outs[0].name = "x".into();
        let mut i = ConstraintInfo::new("0", "");
        assert!(validate_input_constraint(Arch::AArch64, &mut outs, &mut i));
        assert_eq!(i.tied_operand(), Some(0));
        assert!(outs[0].has_matching_input());
        let mut j = ConstraintInfo::new("[x]", "");
        assert!(validate_input_constraint(Arch::AArch64, &mut outs, &mut j));
        let mut k = ConstraintInfo::new("1", "");
        assert!(!validate_input_constraint(Arch::AArch64, &mut outs, &mut k));
        let mut e = ConstraintInfo::new("", "");
        assert!(!validate_input_constraint(Arch::AArch64, &mut outs, &mut e));
    }

    #[test]
    fn global_register_variables() {
        let f = global_register_size_mismatch;
        assert_eq!(f(Arch::AArch64, "x0", 64), Some(false));
        assert_eq!(f(Arch::AArch64, "w8", 64), Some(true));
        assert_eq!(f(Arch::AArch64, "sp", 32), Some(true));
        assert_eq!(f(Arch::AArch64, "lr", 64), None);
        assert_eq!(f(Arch::X86_64, "rsp", 64), Some(false));
        assert_eq!(f(Arch::X86_64, "ebp", 64), Some(true));
        assert_eq!(f(Arch::X86_64, "%rsp", 64), None);
        assert_eq!(f(Arch::X86_64, "rax", 64), None);
    }

    #[test]
    fn simplify() {
        let outs = [out(Arch::AArch64, "=r").unwrap()];
        assert_eq!(simplify_constraint(Arch::AArch64, "r,m", &outs), "r|m");
        assert_eq!(simplify_constraint(Arch::AArch64, "&&r", &outs), "&r");
        assert_eq!(simplify_constraint(Arch::AArch64, "g", &outs), "imr");
        assert_eq!(simplify_constraint(Arch::AArch64, "Upa", &outs), "@3Upa");
        assert_eq!(simplify_constraint(Arch::X86_64, "a", &outs), "{ax}");
    }

    #[test]
    fn clobbers() {
        let a = Arch::AArch64;
        for ok in ["x0", "w1", "x29", "fp", "lr", "r3", "sp", "v1", "d0", "0", "memory", "cc"] {
            assert!(is_valid_clobber(a, ok), "{ok}");
        }
        assert!(!is_valid_clobber(a, "rax"));
        assert_eq!(normalized_register_name(a, "x29"), "fp");
        assert_eq!(normalized_register_name(a, "r3"), "x3");
        assert_eq!(normalized_register_name(a, "0"), "w0");
        assert!(is_valid_clobber(Arch::X86_64, "rax"));
        assert!(is_valid_clobber(Arch::X86_64, "r8d"));
        assert_eq!(normalized_register_name(Arch::X86_64, "eax"), "eax");
    }

    #[test]
    fn template() {
        let names = ["", "nm"];
        let ops = AsmOperandCounts { outputs: 0, plus: 0, inputs: 2, labels: 0, names: &names };
        let p =
            analyze_asm_string(Arch::X86_64, b"{a|b} $1 %0 %= %% %c1 %[nm]", false, &ops).unwrap();
        assert_eq!(generate_asm_string(&p), b"$(a$|b$) $$1 $0 ${:uid} % ${1:c} $1");
        let p = analyze_asm_string(Arch::AArch64, b"{a|b}", false, &ops).unwrap();
        assert_eq!(generate_asm_string(&p), b"{a|b}");
        let e = |s: &[u8]| analyze_asm_string(Arch::AArch64, s, false, &ops).unwrap_err();
        assert_eq!(e(b"%cc1"), (AsmStringError::InvalidEscape, 2));
        assert_eq!(e(b"ab %"), (AsmStringError::InvalidEscape, 3));
        assert_eq!(e(b"%2"), (AsmStringError::InvalidOperandNumber, 1));
        assert_eq!(e(b"%[x]"), (AsmStringError::UnknownName, 2));
        assert_eq!(e(b"%[x"), (AsmStringError::UnterminatedName, 1));
        assert_eq!(e(b"%[]"), (AsmStringError::EmptyName, 1));
    }
}
