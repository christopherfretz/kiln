# WP-154 progress — atomic builtins and `_Atomic(T)` semantics

Branch `wp/154-atomics`, worktree `kiln/.worktrees/wp-154`. clang 19 at -O0
is the oracle throughout.

Result: GATE_RESULT

## Items
1. [x] **Atomic builtins as `AtomicExpr`** (clang `Sema::BuildAtomicExpr`):
   `kiln_sema/src/builtins/atomic.rs:51` `builtin_atomic_op` (pointer,
   value-kind and order checks, operand conversions, result type). The
   builtin ids and per-op shapes are `kiln_ast::decl::builtin::atomic`; the
   rows in the shared builtin tables sit in a separate block ending
   `// ---- end WP-154 ----` (WP-153 edits the same tables).
   Tests: `ext.c11.atomic/builtin-misuse.cpp`, `memory-order.cpp`.
2. [x] **`_Atomic(T)` conversions and layout**: `AtomicToNonAtomic` on
   lvalue conversion (`conv/mod.rs:260`), `NonAtomicToAtomic` on
   assignment/initialization, size/alignment rounded to a power of two
   (`kiln_ast/src/types/layout.rs`). Operand checks use the value type
   (`expr.rs:148` `value_ty`; unary, shifts, `* / % & ^ |`, `switch`).
   `++`/`--` and compound assignment on `_Atomic` lvalues (`expr.rs:1751`,
   `:2030`). List-init of `_Atomic` is "illegal initializer type"
   (`init.rs:669`). `_Atomic(T)` is non-deduced (`templ/deduce.rs:889`).
   Tests: `objects-run.cpp`, `templates-run.cpp`, `list-init.cpp`,
   `deduction.cpp`, `tests/layout/atomic_members.cpp`.
3. [x] **Bare `_Atomic` qualifier** (`kiln_parse/src/decl.rs:615`,
   `tentative.rs`, `kiln_sema/src/decl.rs:204`). Test: `qualifier.cpp`,
   `tests/codegen/atomic_qualifier.cpp`.
4. [x] **Oven**: the lock-free queries fold
   (`kiln_oven/src/compile/builtins.rs:423`, `vm/builtins.rs:99`
   `bi::LOCK_FREE`); atomic casts are no-ops; any other `AtomicExpr` is not
   constant. Tests: `lock-free-aarch64.cpp`, `lock-free-x86_64.cpp`,
   `constexpr.cpp`.
5. [x] **Codegen** (`kiln_codegen/src/atomic.rs`, spec/codegen.md §7d):
   `emit_atomic_expr` (:152; constant orders, order switches, `cmpxchg`
   failure/weak switches), `emit_atomic_builtin` (:790; `test_and_set`,
   `clear`, fences, `syncscope("singlethread")` signal fences,
   `is_lock_free`), `_Atomic` lvalue load/store (:572) and
   `++`/`--`/compound assignment (:711, :747). Tests:
   `tests/codegen/atomic_{builtins,orders,lvalues,wide,qualifier}.cpp`
   (exact on aarch64 and x86_64), `builtins-run.cpp`.

## Not done (codegen or sema errors, follow-ups)
- `AtomicExpr` library calls (`__atomic_load_N`/generic, sizes above the
  inline width or not a power of two); padded `_Atomic` types; aggregate
  `_Atomic` lvalue load/store; a `cmpxchg` loop that needs a libcall
  (x86_64 16 bytes, x87 `long double`); `++` of `_Atomic(bool)`.
- Floating `__atomic_{min,max}_fetch`: clang 19 emits invalid IR; left
  unsupported.
- `-Watomic-alignment`.
- Pre-existing, not atomic-specific: kiln does not fold reads of
  `const double` / `const _Atomic(int)` variables at -O0 where clang does
  (the Oven's `var_info.usable` has no fold-mode leniency), and
  `int arr[c]` with a `const _Atomic` bound is an error rather than
  clang's folded VLA with warnings.
