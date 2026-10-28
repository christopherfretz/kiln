# spec: kiln_llvm — LLVM IR model and textual printer

Status: **implemented** (WP-053). Normative for `crates/kiln_llvm`.
Consumer: `kiln_codegen` (M2), which mirrors clang's lowering, so clang 19's
`-S -emit-llvm` output is the reference for every construct below.
Companions: `docs/ARCHITECTURE.md` (crate roles), `spec/abi.md` (what the
ABI layer asks of signatures: `sret`, `byval`, `zeroext`/`signext`,
coerced aggregates).

## 1. Scope
`kiln_llvm` is an in-memory model of LLVM IR in the **LLVM 19 dialect**, plus
a printer for its textual form. It has no LLVM FFI and no dependencies.
`kiln` hands the printed text to LLVM's tools (`llc`, `opt`, `clang -x ir`),
which own optimization and every platform detail.

Contract, in one sentence: every module built through the public API either
reports an [`IrError`] at the offending call or prints text that
`llvm-as-19` accepts and `opt-19 -passes=verify` verifies, and that text is
a fixed point of `opt -S` (§7).

Design rules:
1. **Index arenas, no pointer graphs.** Types live in a `TypeTable`;
   constants, globals, functions, aliases, comdats, inline asm, attribute
   sets and metadata live in the `Module`; instructions, blocks and operand
   pools live in their `Function`. Every reference is a `u32` newtype
   (`TypeId`, `FuncId`, `GlobalId`, `AliasId`, `ConstId`, `MetadataId`,
   `ComdatId`, `AsmId`, `AttrSetId`, `MdKindId`, `InstId`, `BlockId`).
2. **Interning.** Equal types, constants, attribute sets and uniqued
   metadata share one id, so equality is an integer compare.
3. **Diagnostics, not panics.** Every builder method returns
   `IrResult<_>`; misuse is an `IrError { kind: IrErrorKind, message }`
   (§4.2). Panics are reserved for misuse the type system cannot rule out
   and that has no sensible recovery: an id from a different module or
   function (out-of-range index), or more than 255 named sync scopes.
4. **`#![forbid(unsafe_code)]`.**

## 2. Types and module

### 2.1 Types
`TypeTable` interns: `void`, `iN` (1 ≤ N ≤ 2^23, `MAX_INT_BITS`), `half`,
`bfloat`, `float`, `double`, `fp128`, `x86_fp80`, `ptr` / `ptr
addrspace(N)` (opaque pointers only), fixed and scalable vectors (`<4 x
i32>`, `<vscale x 4 x i32>`), arrays, literal structs (`{ i32, i8 }`,
packed `<{ ... }>`), identified structs (named, `%0`-style unnamed, opaque,
packed), function types (with varargs), `label`, `metadata`, `token`.

- Common types have fixed ids: `TypeId::{VOID, LABEL, METADATA, TOKEN,
  HALF, BFLOAT, FLOAT, DOUBLE, FP128, X86_FP80, PTR, I1, I8, I16, I32, I64,
  I128}`.
- Structural types are hash-consed: `array(I32, 4)` twice is one id.
  Identified structs are nominal: `named_struct("S")` creates a new type
  each call (a taken name is uniqued as `S.N` with a table-wide counter,
  like LLVM); the body is set once with `set_body`, which errors
  (`InvalidType`) on a second call, on a non-identified type, or on a field
  type that cannot be an aggregate element.
- Identified struct definitions print in LLVM's `TypeFinder` order (first
  reference from globals, then functions, then metadata), named ones first
  as `%struct.S = type { i32, [4 x i8] }`, unnamed ones as `%0 = type ...`.

### 2.2 Module header
`Module::new(source_filename)` or `Module::with_target(name, spec)`;
`TargetSpec` constants `AARCH64_LINUX_GNU` and `X86_64_LINUX_GNU` carry
clang 19's triple and data-layout strings verbatim. The header prints
`; ModuleID = '<name>'`, `source_filename`, `target datalayout`, `target
triple`, then module-level inline asm (`module asm "..."`, one per line).

### 2.3 Globals
`add_global(name, value_type) -> GlobalId`, then public fields:
`is_constant`, `linkage` (all eleven LLVM linkages), `visibility`,
`dll_storage`, `dso_local`, `thread_local` (`TlsModel`: general dynamic
prints as `thread_local`, others as `thread_local(localdynamic)` etc.),
`unnamed_addr` / `local_unnamed_addr`, `addrspace`,
`externally_initialized`, `section`, `comdat`, `align`; metadata
attachments with `add_global_metadata`. `set_initializer` checks the
initializer's type. A global without initializer prints as a declaration
(`external global`). Names are uniqued (`x`, `x.1`, ...); an empty name
gives an unnamed global printed `@0`, `@1`, ... in creation order.

### 2.4 Functions
`add_function(name, fn_ty) -> FuncId` creates a declaration; it becomes a
definition once it has blocks. Public fields: `linkage`, `visibility`,
`dll_storage`, `dso_local`, `unnamed_addr`, `cc` (`CallConv`: named
constants print as `fastcc`, `coldcc`, `swiftcc`, `x86_stdcallcc`, ...;
any other number prints as `ccN`), `attrs` (§5), `section`, `align`,
`comdat`, `gc`, `personality`, `addrspace` (program address space). Argument
names are optional (`set_arg_name`). Functions print in creation order,
definitions and declarations interleaved, exactly like LLVM's module list.

`dso_local` is not printed where LLVM implies it (local linkage, or
non-default visibility); the printer omits it there so text is canonical.

### 2.5 Aliases, comdats, inline asm
- `add_alias(name, value_type, aliasee)`; the aliasee is any pointer
  constant (`BadGlobal` otherwise) and fixes the alias's address space.
  Public fields mirror globals (linkage, visibility, `dso_local`, TLS,
  `unnamed_addr`).
- `comdat(name, kind)` returns a uniqued `ComdatId`; only referenced
  comdats print (`$c = comdat any`), those of functions first, then those
  of globals, each in first-use order. A global
  whose comdat has its own name prints `comdat`, otherwise `comdat($c)`.
- `inline_asm(InlineAsm { fn_ty, asm, constraints, side_effect,
  align_stack, intel_dialect, can_throw })` returns a value usable only as a callee.
  Module asm is `Module::module_asm: Vec<String>`.

### 2.6 Metadata
Nodes: strings (`!"..."`, uniqued), constant wrappers (`md_const`, printed
as `i32 4`, `ptr @g`), function-local wrappers (`md_local`, intrinsic
arguments only), tuples (`md_tuple`, uniqued; `md_distinct_tuple`, always
new; `None` operands print as `null`), and **specialized nodes**
(`md_specialized(name, distinct, fields)`) — a name plus ordered
`(key, MdField)` list, enough to express every `DI*` node
(`!DILocation(line: 1, scope: !5)`). `md_set_operand` patches an operand of
a distinct tuple (self-references such as `!5 = distinct !{!5, !6}`).

Named metadata: `add_named_metadata`, `add_module_flag(_i32)` (into
`!llvm.module.flags`), `add_ident` (`!llvm.ident`). Attachments:
`Function::add_metadata`, `Function::add_inst_metadata(inst, kind, md)`,
`Module::add_global_metadata`; `md_kind(name)` interns a kind name
(`dbg`, `tbaa`, `llvm.loop`, ...). Numbering follows LLVM's
`SlotTracker`: named metadata operands, then global attachments, then each
function's attachments in instruction order, each node's operands
depth-first after it. Strings, value wrappers, `DIExpression` and
`DIArgList` print inline.

## 3. Values and constants

### 3.1 `Value`
A `Value` is 4 bytes: a 4-bit tag and a 28-bit index (`Option<Value>` is
8 bytes; operand pools use the reserved `Value::NONE`). Tags: instruction
result, argument, constant, function, global, alias, inline asm, metadata,
block (operand pools only). Instruction, argument and block values are
local to the function that created them. `Value::{func, global, alias,
constant, inst, arg}` construct handles; `get()` decodes to `ValueRef`.

### 3.2 Constants
Interned per module. Integers of any width (`const_int`,
`const_int_words`, `const_bool`; printed in signed decimal, `true`/`false`
for `i1`), floats (`const_float`, `const_float_bits`; printed as LLVM's
`AsmWriter` does: `float`/`double` as `%.6e` decimal (`5.000000e-01`) when
that reparses exactly, otherwise `0x` + 16 hex digits of the double
encoding; `half`, `bfloat`, `x86_fp80`, `fp128` always in hex as `0xH`,
`0xR`, `0xK`, `0xL`), `null`, `zeroinitializer`
(`const_zero`), `undef`, `poison`, `none` (token), arrays, byte strings
(`const_bytes`, `const_cstring`: `c"hi\0A\00"`), structs (identified and
literal), vectors, constant expressions (`getelementptr` with flags and
`inrange`, the casts LLVM 19 still keeps as expressions — `trunc`,
`ptrtoint`, `inttoptr`, `bitcast`, `addrspacecast` — and `add`/`sub`/`mul`/
`xor` with `nuw`/`nsw`), `blockaddress(@f, %bb)`, `dso_local_equivalent`,
`no_cfi`.

Canonicalization matches LLVM's constant folder where LLVM would otherwise
rewrite our text: an aggregate whose elements are all zero becomes
`zeroinitializer`, all-`undef` becomes `undef`, all-`poison` becomes
`poison`; an `[N x i8]` array of integers prints as `c"..."`.

## 4. Instructions and builder
`IrBuilder::new(&mut module, func)` builds into one function. It is
positioned with `position_at_end(bb)`, `position_before(inst)` or
`position_at_start(bb)` (after the phis). Instruction set:

| Group | Methods |
|---|---|
| arithmetic | `binop[_ex]` (all 13 integer and 5 FP ops; `nuw`/`nsw`/`exact`/`disjoint`; fast-math), `add`/`sub`/`mul[_nsw]`, `fneg` |
| compare | `icmp`, `fcmp[_ex]` (all predicates; fast-math) |
| cast | `cast[_ex]` (all 13 casts; `nneg` on `zext`/`uitofp`, `nuw`/`nsw` on `trunc`) |
| memory | `alloca[_ex]` (count, `inalloca`), `load[_ex]`/`store[_ex]` (`MemOpts`: align, volatile, atomic ordering, syncscope), `fence`, `cmpxchg` (`CmpXchgOpts`), `atomicrmw` (all 17 ops) |
| addressing | `gep` (`GepFlags`: `inbounds`, `nusw`, `nuw`; mixed index types; vector GEPs), `struct_gep` |
| aggregates | `extract_value`, `insert_value`, `extract_element`, `insert_element`, `shuffle_vector` (`SHUFFLE_POISON` mask entries) |
| other | `phi[_ex]`, `add_incoming`, `select[_ex]`, `freeze`, `va_arg` |
| calls | `call`, `call_ex` (`CallOpts`: attrs, calling conv, `tail`/`musttail`/`notail`, fast-math, operand bundles; callee = function, pointer value or inline asm), `invoke`, `invoke_ex` |
| EH | `landingpad` (`cleanup`, `catch`/`filter` clauses), `resume`, `catchswitch`, `catchpad`, `cleanuppad`, `catchret`, `cleanupret` |
| terminators | `br`, `cond_br`, `switch`, `indirect_br`, `ret`, `ret_void`, `unreachable` |

Instructions without a result return a `void` value that is rejected as an
operand. An `Inst` is a payload plus result type, **at most 24 bytes**
(asserted); variable-length operand lists live in the function's operand
pool, call data and landing-pad clauses in side tables.

### 4.1 Blocks and numbering
- `append_block(name)` creates and places a block at the end of the layout;
  `create_block` + `place_block` (or `Function::insert_block_after`) place
  later. Layout order is print order.
- Unnamed values and blocks are numbered `%0, %1, ...` exactly as LLVM's
  `SlotTracker` does: unnamed arguments first, then for each placed block
  in layout order the block (if unnamed) and each unnamed non-void
  instruction result. `void` results (stores, void calls, branches) take no
  number. Numbers are computed when printing, so inserting instructions
  earlier renumbers everything after, as in LLVM.
- Names: `name_next(name)` names the next result; `set_name`,
  `Function::set_value_name`, `set_block_name`, `set_arg_name`. Names are
  uniqued per function by appending a counter (`sum`, `sum1`, ...); names
  that are not plain identifiers print quoted with LLVM's escapes
  (`%"a b"`, `@"q\22"`).

### 4.2 Verification and errors
Two layers, both returning `IrError`, never panicking:
1. **At the call** (builder): operand types agree (`TypeMismatch`), valid
   types for the opcode (`InvalidType`), void/foreign operands
   (`InvalidOperand`), flags allowed for the opcode (`InvalidFlags`),
   atomic ordering rules — `load` not `release`/`acq_rel`, `store` not
   `acquire`/`acq_rel`, `cmpxchg` failure not `release`/`acq_rel`, atomics
   need an alignment and a sized first-class type (`InvalidOrdering`),
   aggregate/GEP indices in range (`BadIndex`), call argument count and
   types, varargs, `musttail`/`tail` placement (`BadCall`), appending to a
   terminated or no block (`TerminatorNotLast`, `DetachedBlock`), phis only
   before non-phis (`PhiNotFirst`).
2. **Whole function** (`verify_function`, `Module::verify`): every placed
   block ends in exactly one terminator (`MissingTerminator`); phis lead
   their block and list exactly one incoming value per predecessor *edge*
   (a `switch` with two cases to one block needs two entries) and no others
   (`PhiArity`); the entry block has no predecessors; branch targets are
   placed (`DetachedBlock`); landing pads lead their block in a function
   with a personality (`BadFunction`).

The light verifier does not check dominance of uses (LLVM's verifier
does); codegen is expected to produce SSA from allocas, as clang does at
`-O0`.

## 5. Attributes
`Attr` is a flag (`AttrKind`, every argument-less attribute of LLVM 19),
a type attribute (`byval`, `byref`, `sret`, `inalloca`, `preallocated`,
`elementtype`), an integer attribute (`align`, `allocsize`,
`dereferenceable[_or_null]`, `memory(...)` via `MemoryEffects`,
`alignstack`, `uwtable[(sync)]`, `vscale_range`), or a string attribute
(`"key"="value"`). `Module::attr_set(&[Attr])` sorts, deduplicates and
interns a set; `AttrList { func, ret, params }` attaches sets to a function
or call site (`CallOpts::attrs`); parameter sets beyond the declared
parameters apply to variadic arguments.

Sort order equals LLVM 19's (variant order of `Attr`, then `AttrKind`), so
sets print like clang's: `ptr dead_on_unwind noalias writable
sret(%struct.Big) align 8 %0`. Function attribute sets of definitions,
declarations and call sites print as `#N` groups, numbered in first-use
order (functions in module order, then call sites in instruction order) and
listed after the functions as `attributes #N = { ... }` (inside a group,
`alignstack(16)` prints as `alignstack=16`, as LLVM writes it).

## 6. Intrinsics
Intrinsics are ordinary declarations whose name starts with `llvm.`.
`kiln_llvm::intrinsics` declares the ones C++ codegen needs with LLVM's
exact type, name mangling (`overloaded_name`: `llvm.memcpy.p0.p0.i64`) and
**built-in attributes** (LLVM's parser adds them, so declaring them
ourselves is what makes our text canonical): `memcpy`, `memmove`, `memset`,
`lifetime_start`/`_end`, `trap`, `debugtrap`, `with_overflow` (six ops),
`va_start`/`va_end`/`va_copy`, `stacksave`/`stackrestore`,
`threadlocal_address`, `eh_typeid_for`, `expect`, `assume`, `abs`, `ctlz`,
`cttz`, `ctpop`, `bswap`, `fabs`, `dbg_declare`/`dbg_value`. Each helper
returns the existing declaration if present.

The generic `intrinsic(m, name, fn_ty)` declares any other intrinsic but
cannot know its attributes; the caller must set `attrs` to LLVM's
built-in set or the printed module is accepted but not a fixed point of
`opt -S`.

Debug intrinsics print in intrinsic form (`call void @llvm.dbg.declare`);
LLVM 19's `opt -S` reads and writes that form unchanged.

## 7. Printer
`Module::print(&mut impl io::Write)` and `Module::to_text()`.

- **Accepted**: `llvm-as-19 -` and `opt-19 -passes=verify` accept the
  output of every module the builder and `Module::verify` accept.
- **Fixed point**: `strip_comments(opt -S (text)) == strip_comments(text)`,
  where `strip_comments` drops the comments LLVM adds (`; ModuleID` of the
  re-read input, `; Function Attrs:`, `; preds = ...`,
  `; No predecessors!`). The printer emits none of those comments except
  `; ModuleID`: they are derived information LLVM recomputes, and leaving
  them out keeps printing free of a predecessor pass.
- **Order**: header, module asm, identified struct types, comdats, globals, aliases,
  functions, attribute groups, named metadata, numbered metadata — LLVM's
  `AsmWriter` order, each list in creation order.
- **Deterministic**: the output depends only on the sequence of API calls
  (no hash-map iteration order, no addresses).
- **Lean**: printing allocates per-module and per-function side tables
  (slot numbers, metadata and attribute-group numbering) and nothing per
  instruction beyond what the writer does.

## 8. Deliberately unsupported (LLVM 19)
Not modeled; codegen must not need them in M2. Each is additive later.
- Typed pointers, `ppc_fp128`, `x86_mmx`, `x86_amx`, target extension
  types (`target("...")`).
- Attributes `allockind`, `nofpclass`, `range`, `initializes`, `captures`; `memory(...)`
  covers argmem/inaccessiblemem/other only.
- `ifunc`s, prefix/prologue data, `partition`, `sanitizer_metadata`,
  `no_sanitize_*` on globals, `uselistorder` directives, summary entries
  (`^0 = ...`), `callbr`.
- Debug records (`#dbg_value(...)`); the intrinsic form is used (§6).
- Constant expressions LLVM 19 removed or folds on parse (`fadd`, `icmp`,
  `select`, `extractvalue`, ...).
- `; preds =` and `; Function Attrs:` comments (§7).

## 9. Testing
All tool-backed checks find `llvm-as-19`, `opt-19`, `clang-19` on `PATH`
(or `/usr/lib/llvm-19/bin`); without them the tool half of a test is
skipped with a note, unless `KILN_REQUIRE_LLVM=1`, which makes a missing
tool a failure (release-candidate runs).

- `tests/builder.rs`: interning, size assertions (`Value` 4 B,
  `Option<Value>` 8 B, ids 4 B, `Inst` ≤ 24 B), every `IrErrorKind`,
  verifier rules, determinism.

### 9.1 Corpus
`tests/corpus.rs`: 40 hand-built modules, one per feature group (types,
constants, globals, comdats, attributes, every instruction group, EH both
styles, intrinsics, metadata, debug info, x86-64 signatures, byte strings,
a 1000-value function, CFG shapes). Each test pins the exact text of the
lines it exercises, then requires `llvm-as`/`opt -passes=verify` acceptance
and the `opt -S` fixed point (§7).

### 9.2 Differential against clang
`tests/differential.rs`: for each C program in `tests/cprogs/` (16:
arithmetic, loops with `!llvm.loop`, globals and string literals, structs
with `sret` and `memcpy`, `switch`, varargs with the AArch64 `va_arg`
expansion, floating point, pointers, bit operations, short-circuit `phi`s,
direct/indirect/recursive calls, atomics, overflow builtins and `trap`,
VLAs, TLS, computed goto) the test builds by hand, with the builder, the
module `clang-19 -S -emit-llvm -O0 -fno-ident
--target=aarch64-unknown-linux-gnu` produces, and requires
`strip_comments(opt -S ours) == strip_comments(opt -S clang)`, plus the
fixed point of ours. `all_programs_covered` keeps the list and the
directory in sync. These builders are the reference for how `kiln_codegen`
mirrors clang's `-O0` lowering (attribute sets `#0`, `noundef` parameters,
alloca-then-store prologue, block layout).
