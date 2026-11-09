# Silent drops: constructs kiln ignored instead of implementing or rejecting

Status (WP-805, 2026-11-07). This page is the 2026-11-07 source audit, kept
as the inventory of the policy in spec/sema.md §4.5: kiln never accepts a
semantics-changing construct without implementing it. Every row names its
owner: **fixed in WP-805**, or the work package that implements it
(WP-799 `mode`, WP-806 floating point, WP-807 semantic attributes, WP-808
ABI attributes and flags, WP-809 musttail, transparent_union, selectany,
ms_struct, `[[clang::]]` vector spellings and dependent `mode`). A row still marked open with "error since WP-805"
is rejected loudly until someone implements it. Owners update their own
rows when they land.

What WP-805 changed for every attribute row below:

- Class A attributes clang knows are a hard error, `'X' attribute is not
  supported by kiln yet` (`sema_attribute_unsupported_by_kiln`). It is not
  suppressed in system headers or by `-w`. The table is
  crates/kiln_sema/src/attr_triage.rs.
- Class B and C attributes clang knows are accepted silently, each row with
  its reason. Names clang does not know keep `-Wunknown-attributes`.
- `__has_attribute`, `__has_cpp_attribute` and `__has_declspec_attribute`
  answer 0 for class A attributes, and `__has_builtin` answers 0 for a
  builtin kiln cannot lower or does not declare
  (crates/kiln_driver/src/pp_cmd.rs `StagedFeatures`). `-E` keeps clang's
  raw answers. `__has_c_attribute` is not defined in C++ mode, as in clang.
- AArch64 `aarch64_vector_pcs` was accepted on a declaration only, as an
  interim (glibc's libmvec declarations in `<math.h>` carry it). WP-808
  implements it, with the other calling conventions, as a function-type
  property.

The audit text follows, with owners added.

Scope: the kiln main branch before WP-805, read-only source reading (no build). Question: where does kiln drop an attribute, builtin, pragma
or driver flag it does not support instead of implementing it as clang does or rejecting it with
an error. Paths below are relative to the kiln checkout.

Classes: **A** = changes program semantics / miscompile / ABI / layout; **B** = optimization,
performance, debug info or hardening only; **C** = diagnostics / analysis only (or a loud break
where clang would compile).

"diag" = does kiln emit anything. "warn" for attributes means `unknown attribute 'X' ignored`
(-Wunknown-attributes), which is **suppressed in system headers**, so the drop is fully silent
there (and under `-w` / `-Wno-unknown-attributes`).

## 0. Summary

### Counts

| category | A | B | C | notes |
|---|---|---|---|---|
| attributes | 22 rows (~41 names) | ~45 names | ~114 names (40 with no diagnostic at all) | 138 relevant clang GNU names unknown to kiln + known-but-unlowered kinds |
| builtins | 1 | 4 | 3 | unlowerable builtins are hard errors (good) |
| pragmas | 4 | 4 | 7 | unknown pragmas match clang; MS/`weak`/`clang attribute` are rejected (good) |
| driver flags | 4 | 11 | 1 | unknown flags are hard errors (good) |
| **total (rows)** | **31** | **~64** | **~125** | |

### Systemic causes (fixing these removes most of the A list at once)

Causes 1 to 3 are fixed in WP-805, and so is the class A half of cause 4 (`used`, `aligned` on
functions). For the class B half of cause 4, WP-813 fixed `nonnull` IR; `likely`/`unlikely`
branch weights at -O1+ are a WP-813 follow-up, and WP-814 implemented the debug attributes. Cause 5 is fixed
in WP-806 (#364).

1. **`__has_attribute`/`__has_cpp_attribute`/`__has_builtin` claim everything clang has**
   (crates/kiln_pp/src/features.rs:391-440 answer from the clang-generated features_table.rs).
   Feature-testing code therefore takes the attribute/builtin path and then loses it.
2. **Unknown-attribute handling is a warning, suppressed in system headers** (crates/kiln_sema/
   src/decl.rs:9944/9959; crates/kiln_diag/src/ids.rs:194). Because kiln's "unknown" set includes
   ~138 attributes clang implements, A-class attributes in headers vanish with no trace. Policy fix:
   split the clang-known-but-unimplemented names out of the "unknown" path; A-class names -> hard
   "attribute 'X' is not supported yet" error that is not suppressed in system headers; B/C names
   -> keep the warning (or implement).
3. **Two silent accept lists**: `THREAD_SAFETY_ATTRS` (34, crates/kiln_sema/src/decl_attrs.rs:94)
   and `IGNORED_CLANG_ATTRS` (6, decl_attrs.rs:154), dropped at decl.rs:9911/9915 with no
   diagnostic. All C-class, so acceptable as a stop-gap (operator note: known-and-ignored is not
   the end state; WP-674/675).
4. **Known attribute kinds with no codegen consumer**: `used`, `aligned` on functions (A); `nonnull`
   IR, `likely`/`unlikely`, debug attributes (B). No diagnostic because sema thinks they are handled.
5. **FP pragmas/flags**: contraction is always on, `-ffp-contract=off` is rejected, and every FP
   pragma that would turn it off or make FP strict is accepted silently. **Fixed in WP-806 (#364):
   implemented, or an error for the non-default FP environment.**

### Class A, all categories, ordered by likely real-world impact

1. attr `constructor`/`destructor` (+priority) — function never runs. warn / silent in sys hdr. (§1.3 A1) **Fixed in WP-807.**
2. pragma `STDC FP_CONTRACT OFF` / `clang fp contract(off)` — always fused to FMA, and the
   `-ffp-contract=off` escape is rejected. **No diagnostic.** (§3 P-A1) **Implemented in WP-806 (#364).**
3. attr `cleanup(fn)` — scope-exit cleanup never runs. warn / silent in sys hdr. (A2) **Fixed in WP-807.**
4. attr `used` — unreferenced static/inline entity not emitted, not in `llvm.used`. **No diagnostic.** (A3) **Owner: fixed in WP-805.**
5. builtin `__builtin_assume(expr)` — side effects of `expr` are executed (clang discards them and
   warns). **No diagnostic.** (§2 B-A1) **Owner: fixed in WP-805.**
6. attr `no_destroy` — destructor registered at exit; silent in libc++ (`_LIBCPP_NO_DESTROY`). (A5) **Owner: fixed in WP-805 (also `always_destroy`, `-f[no-]c++-static-destructors`).**
7. attr calling conventions (`ms_abi`, `regcall`, `vectorcall`, `preserve_*`, `aarch64_vector_pcs`,
   `aarch64_sve_pcs`, ...) — ABI mismatch at every call. warn / silent in sys hdr. (A6) **Fixed in WP-808; `swiftcall`, `swiftasynccall` and x86 `interrupt` are still the class A error.**
8. flag `-ffreestanding` with no `no-builtins` + attr `no_builtin` dropped — memcpy/memset
   implementations can be turned into self-recursion. **No diagnostic** for the flag. (F-A1, A9) **Fixed in WP-808 (also `-fno-builtin[-<name>]`).**
9. attr `mode(...)` — wrong integer/vector width. (A4) **Owner: fixed in WP-799.**
10. attr `internal_linkage` — linkage/ODR differs; silent in libc++ configs that use it. (A8) **Fixed in WP-807.**
11. attr `returns_twice` on user functions. (A7) **Fixed in WP-807.**
12. pragma `STDC FENV_ACCESS ON` / `float_control(except)` / `clang fp exceptions` — no
    constrained FP. **No diagnostic.** (P-A2) **An error since WP-806 (#364); `float_control(except, off)` and `clang fp exceptions(ignore)` accepted.**
13. attr `tls_model`. (A10) **Fixed in WP-807.**
14. attr `pass_object_size` / `pass_dynamic_object_size` — hidden-parameter ABI + mangling. (A16) **Fixed in WP-803; the argument folds as clang's in WP-808.**
15. attr `retain` (gc-sections). (A11) **Owner: fixed in WP-805.**
16. flag `-fno-sized-deallocation` — still emits sized delete. **No diagnostic.** (F-A2) **Fixed in WP-808.**
17. flag `-fno-char8_t` — macros only. **No diagnostic.** (F-A3) **Fixed in WP-808 (also `-fchar8_t` under C++17).**
18. attr `address_space(256/257)`, `transparent_union`, `ms_struct`/`gcc_struct`,
    `arm_sve_vector_bits`, `[[clang::neon_vector_type]]`-style spellings, `musttail`,
    multiversioning (`target_clones`, `target_version`, `cpu_specific`/`cpu_dispatch`),
    `aligned` on functions (**no diagnostic**), `x86 interrupt`. (A13-A22) **Owner: `aligned` on functions fixed in WP-805; `transparent_union`, `ms_struct`/`gcc_struct`, the `[[clang::]]` vector spellings and `musttail` fixed in WP-809; `address_space` an error since WP-805 (WP-809 follow-up); `arm_sve_vector_bits` and multiversioning implemented in WP-810 (x86-64 multiversioning an error); the others are an error since WP-805, unowned.**
19. attr `__clang_arm_builtin_alias` / `clang_builtin_alias`, `selectany` — break at link (loud). (A12, A17) **Owner: `selectany` fixed in WP-809; the builtin aliases implemented in WP-810.**
20. pragma `STDC FENV_ROUND`, flag `-fexcess-precision=16`, pragma `clang fp eval_method`. (P-A3, F-A4, P-A4) **An error since WP-806 (#364), except `FENV_ROUND FE_TONEAREST` and `eval_method(source)`.**

Fully silent A items before WP-805 (no diagnostic anywhere): #2, #4, #5, #8 (flag), #12, #16, #17,
aligned-on-function, P-A3, P-A4, F-A4 — plus every attribute row when it appears in a system header.
## 1. Attributes

### 1.1 How kiln decides (the mechanism)

- `__has_attribute` / `__has_cpp_attribute` are answered from
  `crates/kiln_pp/src/features_table.rs` (ATTRIBUTE at :3024, CPP_ATTRIBUTE at :3369), which is
  **generated from clang 19.1.7 / 22.1.8** (`scripts/gen-features.sh`) and consulted unfiltered by
  `crates/kiln_pp/src/features.rs:405` (`has_attribute`) and `:409` (`has_cpp_attribute`). The only
  kiln-specific adjustments are `KILN_CPP_ATTRIBUTES` (adds `indeterminate`) and
  `KILN_REMOVED_CPP_ATTRIBUTES` (removes `carries_dependency` at C++26), features.rs:68-80.
  **So `__has_attribute(X)` answers 1 for every attribute clang knows (337 GNU names, 310 `[[...]]`
  names in clang 19; plus clang-22 deltas), whether or not kiln implements it.** Portable code that
  feature-tests (`#if __has_attribute(cleanup)`) takes the attribute path and then gets the
  attribute dropped.
- Sema: `Sema::attribute_node` (crates/kiln_sema/src/decl.rs:9894ff):
  - decl.rs:9911 — thread-safety attributes (`THREAD_SAFETY_ATTRS`, decl_attrs.rs:94, 34 names):
    `return None` = accepted, attached nowhere, **no diagnostic**.
  - decl.rs:9915 — `IGNORED_CLANG_ATTRS` (decl_attrs.rs:154: `lifetimebound`,
    `no_address_safety_analysis`, `no_sanitize`, `no_sanitize_address`, `no_sanitize_memory`,
    `no_sanitize_thread`): accepted, dropped, **no diagnostic**.
  - decl.rs:9926-9946 — any `[[scope::x]]` whose scope is not `gnu` and that is not in a 14-name
    `clang::` allow-list: `unknown attribute 'x' ignored` (Wunknown-attributes) and dropped.
  - decl.rs:9951-9963 — `attr_kind_of` (decl.rs:10881, 64 names) returns None: `unknown attribute
    'x' ignored` and dropped (`format` with an unknown archetype: `'format' attribute ignored`).
- That warning is **suppressed in system headers** (`DiagId::shows_in_system_header`,
  crates/kiln_diag/src/ids.rs:194; `suppressed_in_system_header`, crates/kiln_diag/src/engine.rs:208)
  — exactly like clang's real `-Wunknown-attributes`. Consequence: an attribute clang implements
  but kiln does not know is **silently dropped when it appears in libc++/libstdc++/glibc/any
  -isystem header**, and dropped with a warning that clang never gives elsewhere (`-w`,
  `-Wno-unknown-attributes`, or `-Wno-attributes` make that silent too).
- Gap: 241 of clang 19's 337 GNU attribute names are not known to kiln (none of
  `attr_kind_of`, `THREAD_SAFETY_ATTRS`, `IGNORED_CLANG_ATTRS`). 102 of those are ObjC / Swift /
  OpenCL / CUDA / SYCL / AMDGPU attributes (not applicable to C++ on Linux; not classified further),
  leaving **138 relevant GNU names** classified below. 172 `clang::`-scoped names are unknown
  (most are the same names; distinct cases listed in 1.4). `__declspec(...)` is rejected
  (`unsupported`, crates/kiln_parse/src/attr.rs:357-359) — correct per policy.

### 1.2 Known attribute kinds that are stored but not (fully) lowered

| attr | kiln | clang | class | diag | owner |
|---|---|---|---|---|---|
| `used` | parsed to `AttrKind::USED` (decl.rs:10904); no reader in kiln_codegen. Deferred emission (`discardable_fn`, crates/kiln_codegen/src/module.rs:1127; `var_must_emit`, global_init.rs:104) never consults it; `llvm.used` (global_init.rs:852-856) holds only unordered-init guards | `UsedAttr`: always emitted (DeclMustBeEmitted) + `@llvm.used` | **A** — an unreferenced `static`/`inline` function or variable marked `used` (asm-referenced helpers, registration tables, `__attribute__((used, section("...")))` plugin/test tables) is not emitted at all | none | fixed in WP-805 |
| `aligned(N)` on a function | accepted (no subject error); layout reads only types/vars/fields (crates/kiln_abi/src/layout/props.rs:78). Function `align` is set only to 2 for x86 methods (crates/kiln_codegen/src/func.rs:398-409) | `align N` on the `llvm::Function` | A-low (code relying on function-address alignment / low-bit tagging) | none | fixed in WP-805 |
| `[[likely]]` / `[[unlikely]]` | checked in sema (crates/kiln_sema/src/stmt.rs:484-534); no codegen reader | branch weights / `llvm.expect` at -O1+ | B | none | open: WP-813 follow-up (xfail `tests/conformance/dcl.attr.class-b/ir-likely-branch-weights-O1-19.cpp`; no IR effect at -O0, as clang) |
| `nonnull` / `returns_nonnull` | sema warnings only (crates/kiln_sema/src/nonnull.rs, null_compare.rs) | `nonnull` param / return IR attribute | B | none | fixed in WP-813 |
| `nodebug`, `artificial`, `standalone_debug` | shape debug info as clang (crates/kiln_codegen/src/debuginfo/func.rs `has_fn_attr(.., NODEBUG)`; `standalone_debug` in `di_omit`, debuginfo/types.rs) | debug-info shaping | B | none | fixed in WP-814 (`standalone_debug` sema in WP-813) |
| `carries_dependency` | stored, unused | no codegen either | none (matches clang) | — | none (matches clang) |
| thread-safety (34), `lifetimebound`, `no_sanitize*` (6) | dropped at decl.rs:9911/9915 | analysis (`-Wthread-safety`, `-Wdangling`) / sanitizer instrumentation | C (sanitizers: moot unless `-fsanitize` is accepted — see §4) | none | WP-805: rows in the triage table (C; `no_sanitize*` B); `lifetimebound` fixed in WP-674 (`-Wdangling`, spec/landed/sema/WP-674.md) |
| `AttrKind::MODE`, `INTERNAL_LINKAGE`, `NO_SANITIZE` | defined in crates/kiln_ast/src/attr.rs:71-74 but never produced (only the AST dumper names them) | — | see `mode`, `internal_linkage` below | — | `mode` fixed in WP-799; `internal_linkage` WP-807 |

Verified as lowered (cite): `aligned`/`alignas`/`packed`/`no_unique_address` (kiln_abi layout
props.rs:78-176, packed enums decl.rs:8279), typedef `aligned` (layout/mod.rs:434),
`always_inline`/`noinline`/`optnone`/`naked`/`minsize`/`cold`→optsize (codegen module.rs:1260-1305),
`cold`/`hot`/`const`/`pure` (codegen builtins.rs:268-318), `noreturn` (expr.rs), `nothrow`
(ast.rs:651), `malloc`/`alloc_size`/`alloc_align` (codegen new_delete.rs:184-290), `weak`/`weakref`
(module.rs:2048-2263, ast.rs:672), `alias`/`ifunc`/`section` (codegen alias.rs), `visibility`/
`type_visibility` (codegen visibility.rs), `init_priority` (global_init.rs), `target` incl. `arch=`
(target_features.rs), `min_vector_width` (func.rs), `disable_tail_calls` (module.rs),
`uninitialized`/`indeterminate` (auto_init.rs), `[[assume]]` (stmt.rs, fold.rs), loop hints
(loop_md.rs), `may_alias`/`no_unique_address` TBAA (tbaa.rs), `trivial_abi` (kiln_abi call.rs),
`abi_tag`/`enable_if`/asm labels (kiln_abi mangle.rs), `coro_only_destroy_when_complete` (coro.rs),
`gnu_inline` (module.rs), `exclude_from_explicit_instantiation` (kiln_ast templates.rs),
vector/ext_vector/neon types (consumed into the type).

### 1.3 Unknown-to-kiln attributes that clang implements — class A (ordered by real-world impact)

All of these go through the same path: `attr_kind_of` returns None (crates/kiln_sema/src/decl.rs:10881),
so the attribute is dropped at decl.rs:9959 with "unknown attribute 'X' ignored" (-Wunknown-attributes;
`[[clang::X]]` forms at decl.rs:9944). **In system headers that warning is suppressed, so the drop is
silent.** `__has_attribute(X)` is 1 for every row (features_table.rs). "diag" below means "outside
system headers / with default warnings".

| # | attribute | what kiln does | what clang does | diag | owner |
|---|---|---|---|---|---|
| A1 | `constructor`, `constructor(prio)`, `destructor`, `destructor(prio)` | dropped; no consumer (global_init.rs builds `llvm.global_ctors` only from dynamic initializers) | entry in `@llvm.global_ctors`/`global_dtors` with priority | warn (silent in sys hdr) — **the function never runs** | fixed in WP-807 |
| A2 | `cleanup(fn)` | dropped | calls `fn(&var)` at scope exit (incl. EH) | warn — RAII-in-C idioms (g_autoptr, systemd `_cleanup_`, lock guards in C-style headers) leak/never unlock | fixed in WP-807 |
| A3 | `used` | see 1.2 (known kind, not lowered) | always emitted + `llvm.used` | **none** | fixed in WP-805 |
| A4 | `mode(QI/HI/SI/DI/TI/word/pointer/V4SI...)` | dropped; `AttrKind::MODE` exists but is never produced | changes the typedef's type (e.g. `typedef int ti __attribute__((mode(TI)))` is 128-bit) | warn — wrong type width/ABI (libgcc-style, glibc `<sys/types.h>` `__intN_t` uses it under some configs, softfloat, compiler-rt style code) | fixed in WP-799 |
| A5 | `no_destroy` / `[[clang::no_destroy]]` | dropped | no `__cxa_atexit` registration for the variable | warn; **silent in libc++ headers** (`_LIBCPP_NO_DESTROY`) — destructor runs at exit -> static-destruction-order bugs/use-after-destroy at shutdown | fixed in WP-805 |
| A6 | calling conventions: `ms_abi`, `regcall`, `vectorcall`, `preserve_most`, `preserve_all`, `preserve_none`, `aarch64_vector_pcs`, `aarch64_sve_pcs`, `intel_ocl_bicc`, `no_caller_saved_registers`, `interrupt` (x86) | dropped (crates/kiln_llvm/src/inst.rs:412-463 can print these CCs but no codegen path sets them from an attribute) | function and call sites use that CC | warn — **ABI mismatch** at every call across the boundary (Wine/EFI-style `ms_abi`, AArch64 vector-math library declarations that use `aarch64_vector_pcs` — silent when they sit in a system header) | fixed in WP-808, except `swiftcall`/`swiftasynccall` and x86 `interrupt` (class A error; xfail repros `dcl.attr.cconv/ir-swiftcall.cpp`, `x86-interrupt.cpp`) |
| A7 | `returns_twice` | dropped; codegen applies `returns_twice` only by callee name (crates/kiln_codegen/src/outline.rs:71) | `returns_twice` on decl/calls | warn — custom setjmp-like/vfork wrappers miscompile at -O | fixed in WP-807 |
| A8 | `internal_linkage` / `[[clang::internal_linkage]]` | dropped (`AttrKind::INTERNAL_LINKAGE` never produced) | entity gets internal linkage | warn; **silent in libc++** (`_LIBCPP_HIDE_FROM_ABI` under some configs, `_LIBCPP_INTERNAL_LINKAGE`) — static locals in inline functions merge across TUs instead of per-TU, ODR/ABI differences | fixed in WP-807 |
| A9 | `no_builtin` / `no_builtin("memcpy")` | dropped | `no-builtins` / `no-builtin-memcpy` fn attr | warn — memcpy/memset implementations get loop-idiom-recognized into self-calls (infinite recursion) | fixed in WP-808 |
| A10 | `tls_model("initial-exec"/"local-exec"/...)` | dropped | `thread_local(initialexec)` etc. | warn — default general-dynamic TLS in malloc implementations/`dlopen`ed libs: recursion into allocator, or perf; wrong model can break preload libs | fixed in WP-807 |
| A11 | `retain` | dropped | `llvm.used` + `SHF_GNU_RETAIN` | warn — section-registered entries stripped by `--gc-sections` | fixed in WP-805 |
| A12 | `selectany` (GNU spelling) | dropped | linkonce/COMDAT for the variable | warn — duplicate-symbol link errors (loud) | fixed in WP-809 |
| A13 | `ms_struct` / `gcc_struct` | dropped | MS bitfield layout | warn — layout/ABI | fixed in WP-809 (`gcc_struct` at `-fclang-compat=22`) |
| A14 | `transparent_union` | dropped (`__has_attribute` = 1) | clang ignores it in C++ too (`-Wignored-attributes`); the C-only effect (union passed as its first member) never applies | none in C++ | fixed in WP-809: clang's ignored warning and delayed subject/field checks; a kept union is passed as its first field |
| A15 | `address_space(N)` | dropped | pointee in addrspace N (x86 `256`=gs, `257`=fs) | warn — gs/fs-relative accesses become flat-memory accesses | error since WP-805; WP-809 follow-up (xfail `ext.clang.address_space/address-space-run.cpp`) |
| A16 | `pass_object_size(N)`, `pass_dynamic_object_size(N)` | dropped | hidden extra size_t parameter + different mangling (`PS`) | warn — **ABI + mangling mismatch** with clang-built objects (clang-oriented fortify headers use these; silent in sys hdr) | fixed in WP-803; argument folds as clang's `tryEvaluateObjectSize` in WP-808 (lenient-designator cases: xfail `ext.clang.pass.object.size/pass-object-size-lenient-xfail.cpp`) |
| A17 | `__clang_arm_builtin_alias`, `clang_builtin_alias` | dropped | maps the declared function to a builtin | warn; silent in `arm_sve.h`/`arm_mve.h`/`arm_sme.h`/`riscv_vector.h` — functions become undefined external references (link failure, loud) | fixed in WP-810 (an alias of an SVE builtin kiln does not lower is an error) |
| A18 | `[[clang::musttail]]` (statement) | dropped | guaranteed tail call (`musttail`) or error | warn — interpreters relying on guaranteed TCO overflow the stack | fixed in WP-809 |
| A19 | `arm_sve_vector_bits(N)` | dropped, although the driver accepts `-msve-vector-bits=` (crates/kiln_driver/src/args.rs:733) | fixed-length SVE type with sizeof = N/8 | warn — sizeof/layout differ | fixed in WP-810 (passing an AAPCS64 pure scalable type by value at `-fclang-compat=22` is an error) |
| A20 | `[[clang::neon_vector_type]]`, `[[clang::neon_polyvector_type]]`, `[[clang::ext_vector_type]]` (`[[...]]` spellings only; GNU spellings are implemented) | unknown scope path, dropped | vector type | warn — type is scalar instead of vector | fixed in WP-809 (`[[clang::ext_vector_type]]` at `-fclang-compat=22`) |
| A21 | `aligned(N)` on a function | see 1.2 | function alignment | **none** | fixed in WP-805 |
| A22 | `target_version`, `cpu_specific`, `cpu_dispatch`, `target_clones` (also listed in B) | dropped | function multiversioning | warn — redefinition errors or wrong single version (target_clones silently becomes one default version) | fixed in WP-810 on AArch64 (clang's driver default, `-fmv`); an error on x86-64 |

### 1.4 Unknown-to-kiln attributes — class B (optimization / perf / debug / hardening only)

Same path (decl.rs:9959, warn outside system headers, silent inside; `__has_attribute` = 1).
**Owner: fixed in WP-813** (spec/landed/codegen/WP-813.md): every attribute below is
implemented as clang implements it (sema, IR, `__has_attribute`), and its rows left the WP-805
triage table, except these. `target_clones` is WP-810's (implemented on AArch64, an error on
x86-64).
`randomize_layout`/`no_randomize_layout` and `btf_decl_tag`/`btf_type_tag` are C-only in clang:
`'X' attribute ignored`, as clang warns in C++. The debug effects of `nodebug`, `artificial` and
`standalone_debug` are implemented in WP-814. Open follow-ups with xfail repros in
tests/conformance/dcl.attr.class-b/: `[[likely]]`/`[[unlikely]]` branch weights at -O1+,
`noescape` on a function type's parameter, and clang 22's `malloc_span` (still a silent class B
triage row, without clang's subject checks).

- inlining / call shape: `flatten`, `leaf`, `nomerge`, `not_tail_called`, `noduplicate`, `convergent`,
  `nouwtable`, `[[clang::always_inline]]` / `[[clang::noinline]]` (incl. statement forms; the GNU
  spellings are implemented), `target_clones` (A22: WP-810).
- pointer facts: `align_value`, `assume_aligned`, `callback`, `noescape`, `maybe_undef`,
  `nonnull`/`returns_nonnull` IR (1.2), `[[likely]]`/`[[unlikely]]` (1.2), `[[omp::assume]]`.
- layout-free codegen hints: `code_align`, `loader_uninitialized`, `force_align_arg_pointer`,
  `randomize_layout`/`no_randomize_layout` (only meaningful with a randomizing plugin).
- debug: `btf_decl_tag`, `btf_type_tag`, `lto_visibility_public`
  (`-g` is implemented since WP-814; `nodebug`, `artificial` and `standalone_debug` are
  honoured, these are not).
  What `-g` leaves out (structured bindings, VLA locals, labels, `using` imports,
  global-initializer subprograms) is listed in docs/status/debuginfo.md "Gaps".
- instrumentation / hardening: `patchable_function_entry`, `xray_always_instrument`,
  `xray_never_instrument`, `xray_log_args`, `no_instrument_function`,
  `no_profile_instrument_function`, `no_stack_protector`, `no_split_stack`, `nocf_check`,
  `cfi_canonical_jump_table`, `speculative_load_hardening`, `no_speculative_load_hardening`,
  `zero_call_used_regs`, `function_return`, `disable_sanitizer_instrumentation`.
  (`no_stack_protector` is borderline: kiln does accept `-fstack-protector*`, so a function that must
  not have a canary — e.g. one that switches stacks or runs before TLS/canary setup — gets one.)
- clang-22 additions: `coro_await_elidable`, `coro_await_elidable_argument`, `malloc_span`.

### 1.5 Unknown-to-kiln attributes — class C (diagnostics / analysis only)

**Owner: WP-805 (accepted silently as class C rows of the triage table).** The "rejection lost"
names (`error`, `warning`, `unavailable`, `diagnose_if`) stay open: kiln does not give clang's
error at a use. `common` is rejected in C++ as clang does (WP-807);
`nocommon` is a class C row (a no-op in C++).

- thread-safety analysis (34 names, decl_attrs.rs:94) and `lifetimebound`/`no_sanitize*`
  (decl_attrs.rs:154): dropped with **no** diagnostic.
- `-Wunknown-attributes` warning but otherwise matches clang's runtime behaviour: handle attributes
  (`acquire_handle` etc.), typestate (`consumable`, `callable_when`, `param_typestate`,
  `return_typestate`, `set_typestate`, `test_typestate`), `annotate`/`annotate_type`,
  `argument_with_type_tag`/`pointer_with_type_tag`/`type_tag_for_datatype`, `availability`,
  `available_only_in_default_eval_method`, `called_once`, `coro_lifetimebound`, `coro_wrapper`,
  `coro_return_type`, `coro_disable_lifetimebound`, `counted_by`/`counted_by_or_null`/`sized_by`/
  `sized_by_or_null`, `diagnose_as_builtin`, `enforce_tcb`/`enforce_tcb_leaf`,
  `format_arg`, `ownership_holds`/`ownership_returns`/`ownership_takes`, `preferred_type`,
  `reinitializes`, `require_constant_initialization`, `sentinel`, `suppress`, `unsafe_buffer_usage`,
  `warn_unused`, `noderef`, `weak_import`, `always_destroy`, `nonallocating`/`nonblocking`/
  `allocating`/`blocking`, `vecreturn`, `[[gsl::Owner]]`/`[[gsl::Pointer]]`/`[[gsl::suppress]]`,
  `[[clang::warn_unused_result]]` (GNU spelling is implemented), clang-22 `nonstring`,
  `no_specializations`, `lifetime_capture_by`, `format_matches`, `require_explicit_initialization`.
- **Rejection lost** (clang errors, kiln warns and continues): `error("msg")`/`warning("msg")`
  (clang: error/warning when a call survives to codegen), `unavailable` (clang rejects uses),
  `overloadable` (implemented by WP-803),
  `common` (an error in C++ since WP-807, as clang), `matrix_type`/`riscv_rvv_vector_bits` (clang errors
  without the enabling flag), `diagnose_if(..., "error")` (clang errors at matching calls).
- target-irrelevant calling conventions on x86_64/aarch64 (`stdcall`, `fastcall`, `thiscall`,
  `cdecl`, `pascal`, `regparm`, `pcs`, `m68k_rtd`, `sysv_abi`): clang ignores them with a
  "calling convention ignored for this target" warning (or they are the default); kiln warns
  as clang since WP-808 (`regparm` is implemented on x86_64).
- `carries_dependency`, `deprecated`, `format`, `analyzer_noreturn`, `_Nullable`/`_Nonnull`,
  `unused`/`maybe_unused`, `preferred_name`, `flag_enum`, `enum_extensibility`: implemented as
  diagnostics, as in clang (not gaps).

### 1.6 Other attribute-adjacent drops (match clang — not findings)

- GNU attributes before `extern "C" {` (crates/kiln_parse/src/decl.rs:412): clang also drops them.
- `__restrict` on references (crates/kiln_parse/src/decl.rs:2527): clang also drops it.
- `[[...]]` type attributes after a ptr-operator (`skip_type_position_attrs`, parse/decl.rs:2410):
  not attached to the pointer type; GNU attributes there are moved to the declarator. Only
  `address_space`/`noderef`/`btf_type_tag`/`annotate_type` care, all already listed above.
## 2. Builtins / intrinsics

Fallback design is sound: a builtin kiln declares but cannot lower is a hard error ("code
generation for X is not supported yet", crates/kiln_codegen/src/builtins.rs:1164-1170, x86.rs:160,
arm.rs:129, neon.rs:616); a missing target feature is an error (target_features.rs:148-178,
x86.rs:142, neon.rs:608); library builtins become plain libc calls (builtins.rs:1163) as in clang;
undeclared names are "use of undeclared identifier" (crates/kiln_sema/src/builtins.rs:544-560).
Nothing becomes a silent call to an external `__builtin_x`.

Verified matching clang (not findings): atomics incl. runtime memory orders, weak flag, cmpxchg
failure-order clamping, float RMW, fences, lock-free queries (crates/kiln_codegen/src/atomic.rs;
oven mirror crates/kiln_oven/src/vm/builtins.rs:443-453), `__sync_*`, `expect`(-with-probability),
`unpredictable`, `unreachable`, `trap`, `debugtrap`, `prefetch`, nontemporal, `launder`,
`assume_aligned`, `assume_separate_storage`, `*_inline` mem builtins, `object_size` (fold path;
runtime path not traced end to end), `*_overflow`, carry builtins, clz/ctz, `is_constant_evaluated`,
source-location builtins, ARM/x86 hint/barrier/crc builtins (kept memory-unknown), va_*.

### Class A

| # | builtin | where | kiln | clang | diag | owner |
|---|---|---|---|---|---|---|
| B-A1 | `__builtin_assume(expr)` with side effects | crates/kiln_codegen/src/builtins.rs:842-847 (verified: `emit_scalar(a0)` then `llvm.assume`) | **evaluates** `expr`: `__builtin_assume(++i)` increments, `__builtin_assume(f())` calls `f` | emits nothing for a side-effecting argument + `-Wassume` warning | none — no `ASSUME` case in sema builtins; `sema_assume_side_effects` (crates/kiln_sema/src/stmt.rs:580) only runs for `[[assume]]`, which codegen handles correctly (codegen stmt.rs:516-533). Oven is correct (crates/kiln_oven/src/compile/builtins.rs:679). | fixed in WP-805 (sema `-Wassume`, codegen emits nothing) |

### Class B

Owner: open (the fast-math item is fixed in WP-806, #364).

- `__builtin_constant_p` not foldable in sema -> always 0 at runtime (builtins.rs:436, verified);
  clang emits `llvm.is.constant.*` at -O1+, which can become 1 after inlining. Fast paths lost.
- `__builtin_verbose_trap(cat, msg)` -> plain `llvm.trap` (builtins.rs:969-973); clang encodes
  the message in debug info.
- fast-math never reaches IR (`FastMath::NONE`, crates/kiln_codegen/src/fir.rs:1108-1110):
  `isnan`/`isinf`/`fpclassify` not folded, no reassociation. Same item as F-B2 below. **Fixed in
  WP-806 (#364).**
- `__builtin_alloca`/`alloca_with_align` memory is not pattern/zero-initialized under
  trivial-auto-var-init (crates/kiln_codegen/src/checked.rs:250-268); clang `initializeAlloca`
  does. Hardening only (reading it is UB).

### Class C

- `__has_builtin` is answered from the copied clang table (crates/kiln_pp/src/features.rs:391-399,
  features_table.rs BUILTIN ~L520-2833), not from kiln's declarations (crates/kiln_ast/src/decl.rs
  `by_name` ~L5551). About 318 non-target builtins report 1 and then fail as undeclared:
  all 27 `__scoped_atomic_*`, the FORTIFY `__builtin___*_chk` family, `__builtin_setjmp`/
  `__builtin_longjmp`, `__builtin_cpu_supports`/`cpu_is`/`cpu_init`, `__builtin_readcyclecounter`,
  `__builtin_flt_rounds`, `__builtin_dump_struct`, `__builtin_allow_runtime_check`,
  `__builtin_elementwise_*` math, `__builtin_ms_va_*`, ptrauth. Loud, not silent, but it defeats
  `#if __has_builtin(...)` fallbacks. **Owner: fixed in WP-805** (`__has_builtin` is 0 for them).
- ARM ACLE builtins declared but not lowerable (arm.rs:129 hard error): ldrex/strex/ldaex/stlex/
  clrex, rsr/wsr(64/p), arm prefetch/trap, rndr/rndrrs, jcvt, rint*, ld64b/st64b*, gcs*, chkfeat.
  Pass `__has_builtin` and sema, fail at codegen. Loud. **Owner: fixed in WP-805** (`__has_builtin`
  is 0 for every name in `kiln_codegen::UNLOWERED_BUILTINS`; implementation unowned).
- No `-Wassume` for side-effecting `__builtin_assume` (diagnostic half of B-A1). **Owner: fixed in WP-805.**
## 3. Pragmas

Code: crates/kiln_parse/src/pragma.rs (lists at :24 `IGNORED`, :28 `UNSUPPORTED`, :31
`CLANG_IGNORED`, :35 `CLANG_UNSUPPORTED`; STDC at :118), crates/kiln_pp/src/pragma.rs.

Correct already: `once`, `push_macro`/`pop_macro`, `GCC poison`, `GCC system_header`, `message`,
`GCC warning`/`GCC error`, `GCC/clang diagnostic` (applied per-location, kiln_pp pragma.rs:529 ->
kiln_diag engine.rs:178), `pack` (sema decl.rs:10744/7761, layout props.rs:181, templates
templ/class.rs:1993), `GCC visibility push/pop` (sema visibility.rs:312), loop pragmas
(codegen loop_md.rs). Rejected with "'#pragma X' is not supported yet" (error, OK by policy):
`weak`, `redefine_extname`, `align`, `options align`, `ms_struct`, `clang attribute`,
`clang section`. Unknown pragmas get -Wunknown-pragmas exactly like clang (incl. `GCC optimize`,
`GCC target`, MS-only pragmas).

### Class A (silent, no diagnostic), by impact

| # | pragma | where | kiln | clang | owner |
|---|---|---|---|---|---|
| P-A1 | `#pragma STDC FP_CONTRACT OFF`, `#pragma clang fp contract(off)` | pragma.rs:118 (STDC accepted, no event); `fp` in CLANG_IGNORED pragma.rs:31 | always fuses `a*b+c` into `llvm.fmuladd` (codegen expr.rs:1392-1509) -> real FMA on AArch64; and `-ffp-contract=off` is rejected as unknown, so there is no way out | stops contraction in scope | implemented in WP-806 (#364) |
| P-A2 | `#pragma STDC FENV_ACCESS ON`, `float_control(except, on)`, `clang fp exceptions(strict/maytrap)` | :118, :24, :31 | plain FP ops; LLVM may move/fold across `fesetround`/`fetestexcept` | constrained FP intrinsics | an error since WP-806 (#364) |
| P-A3 | `#pragma STDC FENV_ROUND <mode>` | :118 | round-to-nearest | static rounding mode for ops and folding | an error since WP-806 (#364) |
| P-A4 | `#pragma clang fp eval_method(...)` | :31 | none | evaluation precision + `FLT_EVAL_METHOD` | an error since WP-806 (#364) |

### Class B

Owner: `clang fp` / `float_control` / `CX_LIMITED_RANGE` implemented in WP-806 (#364); the rest open.

- `#pragma clang optimize off` (:31): no `optnone` on following functions.
- `clang fp reassociate(on)`, `clang fp contract(fast)`, `float_control(precise, off)` (:31, :24):
  loosening only; kiln stays strict (slower, still correct).
- `#pragma STDC CX_LIMITED_RANGE ON` (:118): full-range complex mul/div kept.
- `#pragma comment(lib, "x")` (:24): clang ELF emits `llvm.dependent-libraries` (lld honors it);
  dropped -> possible link failure (loud).

### Class C

Owner: open (class C).

- `#pragma unused(x)` (:24) — spurious -Wunused possible.
- `clang max_tokens_here/total` (:31) — never warns.
- `clang deprecated/final/restrict_expansion` (kiln_pp pragma.rs:303) — no warnings at macro uses
  (documented, spec/pp.md deviation 3).
- `GCC dependency` (kiln_pp pragma.rs:218) — no date check.
- `omp`, `omp simd` (:24) — silent; clang without -fopenmp has an off-by-default
  -Wsource-uses-openmp; `-fopenmp` itself is rejected. Near parity.
- `comment(compiler/user/exestr)` — clang ELF warns; kiln silent.
- `clang __debug` (:31).

Doc drift (not behaviour): kiln_pp pragma.rs:524-527 and kiln_parse pragma.rs:105-106 say
diagnostic pragmas are inert (they are not); spec/parse.md:403-405 say `pack` unsupported,
`clang diagnostic ignored` inert, `GCC ivdep` ignored — all stale. **Owner: fixed in WP-805.**

## 4. Driver flags

Code: crates/kiln_driver/src/args.rs, codegen_flags.rs. Unknown flags are a hard error
(`ArgError::Unknown`, args.rs:658; also for `-Xclang` args). Explicit ignore table
codegen_flags.rs:60 (`-f[no-]stack-clash-protection`, `-fcf-protection[=]`, `-f[no-]common`,
`-f[no-]math-errno`, `-fstack-usage`, `-gz[=]`, `-fno-short-enums`, `-m64`, `-pipe`)
+ `-fexcess-precision=` (:165). `-g*` and `-gz[=]` are implemented since WP-814
(crates/kiln_driver/src/debug_flags.rs; each unimplemented debug flag is an error). Explicit reject tables
codegen_flags.rs:79/94 (`-ftrapv`, `-fshort-enums`, `-fsanitize*`, `-fprofile-*`, coverage).
All the strictness-relevant FP flags (`-ffp-contract=`, `-ffp-model=`, `-frounding-math`, ...),
`-fms-extensions`, `-fno-builtin*`, `-fno-elide-constructors`, `-fopenmp`, `-flto`, `-m<feature>`
etc. are rejected as unknown (loud; fine by policy).

### Class A (accepted, partly applied, no diagnostic), by impact

| # | flag | where | kiln | clang | owner |
|---|---|---|---|---|---|
| F-A1 | `-ffreestanding` | args.rs:1117 -> only predefined macros (pp_cmd.rs:148) + unwind default (codegen_cmd.rs:181) | no `no-builtins` fn attribute anywhere in kiln_codegen (verified: grep finds only the `nobuiltin` call-site attr for operator new/delete, new_delete.rs:307); library builtins still recognized | implies `-fno-builtin`: `"no-builtins"` on every function -> no loop-idiom self-recursion in `memcpy`/`memset` implementations | fixed in WP-808 |
| F-A2 | `-fno-sized-deallocation` | args.rs:1123 -> only pp_cmd.rs:151 / `__cpp_sized_deallocation` (predef.rs:561) | still emits sized `operator delete(void*, size_t)` (codegen new_delete.rs:1509) | unsized delete | fixed in WP-808 |
| F-A3 | `-fno-char8_t` | args.rs:1107 -> macros only (predef.rs:541) | `char8_t` stays a keyword; `u8""` stays `const char8_t[]` (parse decl.rs:883) | `u8""` is `const char[]`, `char8_t` not a keyword (mostly loud errors; some overloads/mangling silently differ) | fixed in WP-808 |
| F-A4 | `-fexcess-precision=16` | codegen_flags.rs:165 | validated, dropped; always "standard" `_Float16`/`__bf16` (codegen expr.rs:1283) | no excess precision for `_Float16` | an error since WP-806 (#364) |

### Class B

Owner: fast-math and `-fno-math-errno` implemented in WP-806 (#364); `-fcf-protection` in WP-813;
`-g*` in WP-814; the rest open.

- `-g`, `-g1..3`, `-ggdb*`: fixed in WP-814 (crates/kiln_driver/src/debug_flags.rs; DWARF as
  clang 19 emits it, docs/status/debuginfo.md; an unimplemented debug flag is an error).
- `-ffast-math`/`-Ofast`: macros (`__FAST_MATH__`, predef.rs:582) + basic complex range only; no
  fast-math flags reach IR (fir.rs `FastMath::NONE`). (Note: code testing `__FINITE_MATH_ONLY__`
  takes a branch while kiln keeps NaN semantics — consistent but slower.)
- `-fno-math-errno` (codegen_flags.rs:60): errno semantics kept (builtins.rs:194) -> no
  `llvm.sqrt` etc.
- `-fno-threadsafe-statics`: macro only (predef.rs:558); `__cxa_guard_*` still called
  (global_init.rs:545) — extra libc++abi dependency in freestanding builds.
- `-fcf-protection[=...]` (codegen_flags.rs:62): no CET module flags/endbr -> one kiln object
  clears IBT/SHSTK for the whole linked binary (security hardening). **Fixed in WP-813**: the
  `cf-protection-branch`/`-return` module flags and `nocf_check`, as clang (spec/driver.md).
- `-fstack-clash-protection`: no `probe-stack` attribute.
- `-fstack-usage`: no `.su` output.
- `-march=native` on x86 (features.rs:428-433): only `-mcpu=native` to llc; no feature list, no
  `__AVX2__`-style macros (possible ABI difference for 256/512-bit vector by-value args — not
  verified).
- `-Xclang -target-feature` off AArch64 (features.rs:415) silently ignored.
- `-mtune=native` dropped (codegen_cmd.rs:187).
- `-gz[=...]`: fixed in WP-814 (the object's debug sections are compressed, docs/status/debuginfo.md).

### Class C

Owner: open.

- `-fcf-protection=...` on AArch64 accepted silently (clang: error "option not supported on this
  target"). **Fixed in WP-813**: clang's `-cc1` errors.
