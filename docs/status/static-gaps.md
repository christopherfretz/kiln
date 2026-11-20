# Static gaps (WP-1719, 2026-11-20)

Every place in kiln's source that reports a language corner kiln knows it
does not handle yet: a "not supported yet" or "not yet implemented"
diagnostic, a mangling failure, a code generation fallback, an Oven trap,
a reflection function without an implementation. One entry per gap, keyed
by a stable id (`GAP-NNN`), each pointing at the BACKLOG row that fixes it.
`cargo xtask ci --step lints` keeps the list exact (spec/harness.md "CI",
step 0): a new site needs an entry and a row, and a fixed gap must leave.

93 gaps at 176 sites; 10 of 56,766 g++.dg cases hit one at the
1.3.0 release candidate (3 of them GAP-093, since fixed). 4 more entries
(5 sites) use wording that matches clang's for a limit clang shares: they
are listed as excluded and stay.

## Method

A *site* is a line in a crate's `src/` (test modules excluded) that
reports a gap:

- a use of a gap diagnostic: `cg_cannot_mangle_expression`,
  `drv_not_implemented`, `parse_unsupported`, `pp_expr_unsupported`,
  `pp_header_unit_unsupported`, `pp_scan_header_unit`,
  `sema_attribute_unsupported_by_kiln`, `sema_conv_unsupported`,
  `sema_multiversion_not_supported`, `sema_unimplemented_template_feature`,
  `sema_vla_unsupported`;
- a call of a function that forwards its text to one: `unsupported`,
  `templ_unimplemented`, `unimplemented` and `not_implemented` in
  `kiln_sema`, `unsupported` in `kiln_parse`, `unsupported` and
  `not_implemented` in `kiln_oven`;
- a string literal holding "not supported yet", "not yet supported", "not
  yet implemented", "not implemented yet", "cannot yet", "support header
  units yet", or both "cannot compile" and " yet" (the codegen fallbacks,
  the Oven traps, reflection);
- `todo!` or `unimplemented!` (none today).

Each site carries a marker comment on its line or in the comment block
directly above it: `// kiln-gap: GAP-NNN` names its entry; `// kiln-gap:
forward` marks the body of a forwarding function (its callers are the
sites; 6 today); `// kiln-gap: check` marks a line that only compares a
diagnostic's id with a gap id (2 today). A `GAP-NNN` marker that no
detected site uses marks the next line of code as a site of its own (the
four places in `kiln_abi` that record an expression with no mangling).
Markers make the list insensitive to line shifts. The sites cell names
each site as `file::fn` (` ×N` for N sites in one function); the lint
prints the expected cell when it differs.

Regenerate and check:

```
cargo xtask static-gaps --sites   # every site: file, line, fn, marker, kind, text
cargo xtask static-gaps           # the lint (also in cargo xtask ci --step lints)
```

The lint fails when a site has no marker (or names an id this page lacks),
when an entry has no site left (a fixed gap: delete the entry), when an
entry's sites cell is not the list its markers give, or when its row is
not an `open` or `in progress` row of docs/BACKLOG.md (an excluded entry
says `excluded: <reason>` instead). To fix a gap, remove its sites and its
entry in the same change.

*Reach* is the number of failing g++.dg cases on aarch64 at the 1.3.0
release candidate whose first kiln error (`kiln_detail` of
`target-kiln/release-gate/out/gxx-dg-aarch64.tsv`, `cargo xtask
release-gate`) is the entry's text. Oven traps and reflection errors are
notes under a constant-evaluation error, so their reach is not counted.
A computed text (`{k}`, `{what}`) lists the strings that reach it; the kinds
of an exhaustive match arm are under Enumerated kinds, below.

## Totals

By family:

| family | gaps | sites | reach |
|---|---|---|---|
| Oven trap | 20 | 90 | 0 |
| codegen fallback | 34 | 39 | 7 |
| drv_not_implemented | 3 | 3 | 0 |
| parse_unsupported | 26 | 29 | 0 |
| pp_expr_unsupported | 1 | 3 | 0 |
| pp_header_unit_unsupported | 1 | 1 | 0 |
| pp_scan_header_unit | 1 | 1 | 0 |
| reflection error | 3 | 3 | 0 |
| sema_attribute_unsupported_by_kiln | 1 | 1 | 0 |
| sema_multiversion_not_supported | 1 | 4 | 0 |
| sema_unimplemented_template_feature | 1 | 1 | 0 |
| sema_vla_unsupported | 1 | 1 | 0 |

By area (the BACKLOG row that fixes it, most-hit first):

| area | gaps | sites | reach | row |
|---|---|---|---|---|
| codegen fallbacks | 34 (GAP-059..GAP-092) | 39 | 7 | WP-1739 |
| NTTP/substitution kinds | 1 (GAP-097) | 1 | 0 | WP-1740 |
| Oven traps: expressions | 15 (GAP-110..GAP-124) | 55 | 0 | WP-1741 |
| Oven traps: types | 5 (GAP-125..GAP-129) | 35 | 0 | WP-1742 |
| reflection | 5 (GAP-130..GAP-134) | 5 | 0 | WP-1743 |
| preprocessor/modules | 9 (GAP-135..GAP-143) | 11 | 0 | WP-1744 |
| misc | 24 (GAP-144..GAP-167) | 30 | 0 | WP-1745 |

## Gaps

| gap | family | area | text | sites | reach | row |
|---|---|---|---|---|---|---|
| GAP-059 | codegen fallback | codegen fallbacks | `expression {k} is not supported yet` ({k}: the 37 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::emit_scalar_x` | 4 | WP-1739 |
| GAP-060 | codegen fallback | codegen fallbacks | `code generation for {name} is not supported yet` ({name}: a builtin sema accepts with no lowering in that target's table (at the RC `__atomic_always_lock_free`)) | `crates/kiln_codegen/src/arm.rs::emit_arm_builtin`, `crates/kiln_codegen/src/atomic.rs::emit_atomic_builtin`, `crates/kiln_codegen/src/builtins.rs::emit_builtin`, `crates/kiln_codegen/src/neon.rs::emit_neon_builtin`, `crates/kiln_codegen/src/x86.rs::emit_x86_builtin` | 3 | WP-1739 |
| GAP-061 | codegen fallback | codegen fallbacks | `passing or returning '{t}' by value is not supported yet: it is an SVE tuple type, ...` / `...: it is an AAPCS64 pure scalable type ...` | `crates/kiln_codegen/src/module.rs::check_pure_scalable` ×2 | 0 | WP-1739 |
| GAP-062 | codegen fallback | codegen fallbacks | `code generation for SVE builtin '{name}' (called as '{called}') is not supported yet` | `crates/kiln_codegen/src/sve.rs::emit_sve_alias` | 0 | WP-1739 |
| GAP-063 | codegen fallback | codegen fallbacks | `cast {k} is not supported yet` ({k}: the 8 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::emit_cast` | 0 | WP-1739 |
| GAP-064 | codegen fallback | codegen fallbacks | `unary {op} is not supported yet` ({op}: `Coawait`) | `crates/kiln_codegen/src/expr.rs::emit_unary` | 0 | WP-1739 |
| GAP-065 | codegen fallback | codegen fallbacks | `binary {op} is not supported yet` ({op}: the 17 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::arith` | 0 | WP-1739 |
| GAP-066 | codegen fallback | codegen fallbacks | `reference to {k} is not supported yet` ({k}: the 49 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::decl_lvalue` | 0 | WP-1739 |
| GAP-067 | codegen fallback | codegen fallbacks | `lvalue {k} is not supported yet` ({k}: the 64 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::emit_lvalue_x` | 0 | WP-1739 |
| GAP-068 | codegen fallback | codegen fallbacks | `lvalue unary {op} is not supported yet` ({op}: the 8 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::unary_lvalue` | 0 | WP-1739 |
| GAP-069 | codegen fallback | codegen fallbacks | `lvalue binary {op} is not supported yet` ({op}: the 29 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::binary_lvalue` | 0 | WP-1739 |
| GAP-070 | codegen fallback | codegen fallbacks | `lvalue cast {k} is not supported yet` ({k}: the 38 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::cast_lvalue` | 0 | WP-1739 |
| GAP-071 | codegen fallback | codegen fallbacks | `aggregate expression {k} is not supported yet` ({k}: the 63 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/expr.rs::agg_visit_x` | 0 | WP-1739 |
| GAP-072 | codegen fallback | codegen fallbacks | `pointer to member {k} is not supported yet` ({k}: the 53 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/memptr.rs::mp_addr_of` | 0 | WP-1739 |
| GAP-073 | codegen fallback | codegen fallbacks | `statement {k} is not supported yet` ({k}: `CXXCatchStmt`) | `crates/kiln_codegen/src/stmt.rs::emit_stmt` | 0 | WP-1739 |
| GAP-074 | codegen fallback | codegen fallbacks | `a complex opaque value is not supported yet` | `crates/kiln_codegen/src/complex.rs::emit_cx_x` | 0 | WP-1739 |
| GAP-075 | codegen fallback | codegen fallbacks | `complex expression {k} is not supported yet` ({k}: the 72 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/complex.rs::emit_cx_x` | 0 | WP-1739 |
| GAP-076 | codegen fallback | codegen fallbacks | `complex cast {k} is not supported yet` ({k}: the 32 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/complex.rs::emit_cx_cast` | 0 | WP-1739 |
| GAP-077 | codegen fallback | codegen fallbacks | `complex unary {op} is not supported yet` ({op}: the 6 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/complex.rs::emit_cx_unary` | 0 | WP-1739 |
| GAP-078 | codegen fallback | codegen fallbacks | `complex binary {op} is not supported yet` ({op}: the 27 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/complex.rs::emit_cx_binary` | 0 | WP-1739 |
| GAP-079 | codegen fallback | codegen fallbacks | `atomic compare-exchange of {vq} is not supported yet` | `crates/kiln_codegen/src/atomic.rs::atomic_cmpxchg_lv` | 0 | WP-1739 |
| GAP-080 | codegen fallback | codegen fallbacks | `binding a reference to a prvalue is not supported yet` | `crates/kiln_codegen/src/class.rs::emit_ref_binding_in` | 0 | WP-1739 |
| GAP-081 | codegen fallback | codegen fallbacks | `construction of variable-length arrays is not supported yet` ({k}: the 2 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/class.rs::construct_array` | 0 | WP-1739 |
| GAP-082 | codegen fallback | codegen fallbacks | `calls through member pointers is not supported yet` | `crates/kiln_codegen/src/class.rs::emit_member_call` | 0 | WP-1739 |
| GAP-083 | codegen fallback | codegen fallbacks | `base-class subobjects in constants are not supported yet (M3)` | `crates/kiln_codegen/src/consts.rs::image_const` | 0 | WP-1739 |
| GAP-084 | codegen fallback | codegen fallbacks | `scalable-vector and target-type constants are not supported yet (M3)` ({k}: the 3 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/consts.rs::image_const` | 0 | WP-1739 |
| GAP-085 | codegen fallback | codegen fallbacks | `union base-class constants are not supported yet (M3)` | `crates/kiln_codegen/src/consts.rs::record_elems` | 0 | WP-1739 |
| GAP-086 | codegen fallback | codegen fallbacks | `class constants with this member is not supported yet` | `crates/kiln_codegen/src/consts.rs::record_elems` | 0 | WP-1739 |
| GAP-087 | codegen fallback | codegen fallbacks | `cannot compile a caller-side postcondition of this result yet` | `crates/kiln_codegen/src/contracts.rs::emit_client_post` | 0 | WP-1739 |
| GAP-088 | codegen fallback | codegen fallbacks | `an ignored scalar parameter is not supported yet` | `crates/kiln_codegen/src/func.rs::prolog_arg` | 0 | WP-1739 |
| GAP-089 | codegen fallback | codegen fallbacks | `copying an under-aligned byval argument is not supported yet` | `crates/kiln_codegen/src/func.rs::call_arg` | 0 | WP-1739 |
| GAP-090 | codegen fallback | codegen fallbacks | `this deallocation function is not supported yet` | `crates/kiln_codegen/src/new_delete.rs::emit_delete_call_n` | 0 | WP-1739 |
| GAP-091 | codegen fallback | codegen fallbacks | `this initializer of a variable length array is not supported yet` | `crates/kiln_codegen/src/vla.rs::emit_vla_var` | 0 | WP-1739 |
| GAP-092 | codegen fallback | codegen fallbacks | `RTTI for type `{name}` is not supported yet` ({k}: the 36 kinds of the arm, under Enumerated kinds) | `crates/kiln_codegen/src/vtable.rs::rtti` | 0 | WP-1739 |
| GAP-097 | sema_unimplemented_template_feature | NTTP/substitution kinds | `kiln: not yet implemented: substitution into {k}` ({k}: the 25 kinds of the arm, under Enumerated kinds) | `crates/kiln_sema/src/templ/subst/exprs.rs::ex` | 0 | WP-1740 |
| GAP-110 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: the expression's kind, where an operand form, operator or type of an rvalue is not handled) | `crates/kiln_oven/src/compile.rs::arith`, `crates/kiln_oven/src/compile.rs::binary_node`, `crates/kiln_oven/src/compile.rs::convert`, `crates/kiln_oven/src/compile.rs::effect`, `crates/kiln_oven/src/compile.rs::rv_inner` ×6, `crates/kiln_oven/src/compile.rs::three_way` ×3, `crates/kiln_oven/src/compile.rs::trait_expr`, `crates/kiln_oven/src/compile.rs::unary` | 0 | WP-1741 |
| GAP-111 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: the kind of an lvalue (or assignment target) form not handled) | `crates/kiln_oven/src/compile.rs::compound_assign`, `crates/kiln_oven/src/compile.rs::lv` ×6, `crates/kiln_oven/src/compile.rs::member` | 0 | WP-1741 |
| GAP-112 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: the initializer's kind, for an aggregate or `std::initializer_list` form not handled (the type kinds of the arms in `init_aggr` and `std_initializer_list`)) | `crates/kiln_oven/src/compile.rs::init_aggr` ×3, `crates/kiln_oven/src/compile.rs::std_initializer_list` ×4 | 0 | WP-1741 |
| GAP-113 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: the kind of a class operation not handled (derived-to-base cast, member initializer, construction, inherited constructor, member call, array init loop)) | `crates/kiln_oven/src/compile/class.rs::array_init_loop`, `crates/kiln_oven/src/compile/class.rs::class_cast`, `crates/kiln_oven/src/compile/class.rs::construct_obj`, `crates/kiln_oven/src/compile/class.rs::ctor_init` ×2, `crates/kiln_oven/src/compile/class.rs::inherited_ctor`, `crates/kiln_oven/src/compile/class.rs::member_call` | 0 | WP-1741 |
| GAP-114 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: a `new`/`delete` form not handled) | `crates/kiln_oven/src/compile/new_delete.rs::delete_expr` ×3, `crates/kiln_oven/src/compile/new_delete.rs::new_expr` ×3 | 0 | WP-1741 |
| GAP-115 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: a builtin call form not handled) | `crates/kiln_oven/src/compile/builtins.rs::builtin_call` ×2 | 0 | WP-1741 |
| GAP-116 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: `SourceLocExpr` of a form not handled) | `crates/kiln_oven/src/compile.rs::source_loc` ×2 | 0 | WP-1741 |
| GAP-117 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: the 49 kinds of the arm, under Enumerated kinds) | `crates/kiln_oven/src/compile.rs::rv_inner` | 0 | WP-1741 |
| GAP-118 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: the 12 kinds of the arm, under Enumerated kinds) | `crates/kiln_oven/src/compile.rs::cast` | 0 | WP-1741 |
| GAP-119 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: the 95 kinds of the arm, under Enumerated kinds) | `crates/kiln_oven/src/compile.rs::init_inner` | 0 | WP-1741 |
| GAP-120 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: `CXXExpansionStmt`, `CXXCatchStmt` (as statements)) | `crates/kiln_oven/src/compile.rs::stmt` | 0 | WP-1741 |
| GAP-121 | Oven trap | Oven traps: expressions | `kiln: a 'case' label outside its switch body is not supported yet` | `crates/kiln_oven/src/compile.rs::stmt` | 0 | WP-1741 |
| GAP-122 | Oven trap | Oven traps: expressions | `kiln: a 'case' label in this statement is not supported yet` | `crates/kiln_oven/src/compile.rs::switch_stmt` | 0 | WP-1741 |
| GAP-123 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: `__builtin_bit_cast` of a form not handled) | `crates/kiln_oven/src/compile/bitcast.rs::bit_cast` | 0 | WP-1741 |
| GAP-124 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: a `std::meta` call with more arguments than `MAX_META_ARGS`) | `crates/kiln_oven/src/compile.rs::meta_call` | 0 | WP-1741 |
| GAP-125 | Oven trap | Oven traps: types | `kiln: constant evaluation of {k} is not supported yet` ({k}: a vector operation not handled) | `crates/kiln_oven/src/compile/vector.rs::ext_elt_regs` ×3, `crates/kiln_oven/src/compile/vector.rs::vec_binary`, `crates/kiln_oven/src/compile/vector.rs::vec_compound_assign` ×3, `crates/kiln_oven/src/compile/vector.rs::vec_elementwise`, `crates/kiln_oven/src/compile/vector.rs::vec_gcc_shuffle` ×3, `crates/kiln_oven/src/compile/vector.rs::vec_init`, `crates/kiln_oven/src/compile/vector.rs::vec_reduce`, `crates/kiln_oven/src/compile/vector.rs::vec_regs` ×2, `crates/kiln_oven/src/compile/vector.rs::vec_shuffle` ×2 | 0 | WP-1742 |
| GAP-126 | Oven trap | Oven traps: types | `kiln: constant evaluation of {k} is not supported yet` ({k}: an operation on a `_BitInt` wider than 128 bits not handled (`wide_to_scalar`: arm)) | `crates/kiln_oven/src/compile/wide.rs::wide_compare`, `crates/kiln_oven/src/compile/wide.rs::wide_compound_assign` ×3, `crates/kiln_oven/src/compile/wide.rs::wide_float_compound` ×2, `crates/kiln_oven/src/compile/wide.rs::wide_store`, `crates/kiln_oven/src/compile/wide.rs::wide_to_scalar` | 0 | WP-1742 |
| GAP-127 | Oven trap | Oven traps: types | `kiln: constant evaluation of {k} is not supported yet` ({k}: a complex operation not handled) | `crates/kiln_oven/src/compile/complex.rs::cx_cast`, `crates/kiln_oven/src/compile/complex.rs::cx_compare`, `crates/kiln_oven/src/compile/complex.rs::cx_compound_assign`, `crates/kiln_oven/src/compile/complex.rs::cx_inc_dec`, `crates/kiln_oven/src/compile/complex.rs::cx_init`, `crates/kiln_oven/src/compile/complex.rs::cx_regs` ×2 | 0 | WP-1742 |
| GAP-128 | Oven trap | Oven traps: types | `kiln: constant evaluation of {k} is not supported yet` ({k}: a member-pointer constant or conversion not handled) | `crates/kiln_oven/src/compile/mptr.rs::mp_const`, `crates/kiln_oven/src/compile/mptr.rs::mp_conv` | 0 | WP-1742 |
| GAP-129 | Oven trap | Oven traps: types | `kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet` | `crates/kiln_oven/src/compile.rs::wide_bit_int` | 0 | WP-1742 |
| GAP-130 | reflection error | reflection | `kiln: reflection function '{name}' is not implemented yet (WP-{wp})` ({name}: the queries `meta_query` has no arm for (`symbol_of`, `u8symbol_of`)) | `crates/kiln_sema/src/meta/query/mod.rs::meta_query` | 0 | WP-1743 |
| GAP-131 | reflection error | reflection | `kiln: '{name}' is not implemented yet` ({name}: a `std::meta` trait whose builtin is not implemented (the type traits of the `type trait` entry)) | `crates/kiln_sema/src/meta/traits.rs::trait_result` | 0 | WP-1743 |
| GAP-132 | reflection error | reflection | `kiln: constant_of an object of type '{t}' is not implemented yet` | `crates/kiln_sema/src/meta/query/entity.rs::object_constant` | 0 | WP-1743 |
| GAP-133 | parse_unsupported | reflection | `splice of this value reflection is not supported yet` | `crates/kiln_sema/src/splice.rs::act_on_splice_expr` | 0 | WP-1743 |
| GAP-134 | parse_unsupported | reflection | `{what} is not supported yet` ({what}: `splice of a reflection of this subobject`, `splice of a reflection of this object`) | `crates/kiln_sema/src/splice.rs::act_on_splice_expr` | 0 | WP-1743 |
| GAP-135 | pp_expr_unsupported | preprocessor/modules | `kiln: this #if expression needs the full evaluator (WP-015)` | `crates/kiln_pp/src/cond.rs::cond`, `crates/kiln_pp/src/cond.rs::unary` ×2 | 0 | WP-1744 |
| GAP-136 | pp_header_unit_unsupported | preprocessor/modules | `importing a header unit is not supported yet` | `crates/kiln_pp/src/directive.rs::maybe_module_directive` | 0 | WP-1744 |
| GAP-137 | pp_scan_header_unit | preprocessor/modules | `header unit {0} cannot be imported: kiln-scan-deps does not support header units yet` | `crates/kiln_pp/src/modules.rs::module_directive` | 0 | WP-1744 |
| GAP-138 | parse_unsupported | preprocessor/modules | `modules is not supported yet` | `crates/kiln_parse/src/decl.rs::parse_external_declaration_inner` | 0 | WP-1744 |
| GAP-139 | parse_unsupported | preprocessor/modules | `{what} is not supported yet` ({what}: `a module import`, `a module declaration` (inside a pragma region)) | `crates/kiln_parse/src/pragma.rs::on_event` | 0 | WP-1744 |
| GAP-140 | drv_not_implemented | preprocessor/modules | `{what} is not implemented yet` ({what}: `a module file before C++20`, `preprocessing a module file`) | `crates/kiln_driver/src/pp_cmd.rs::configure_preprocessor` | 0 | WP-1744 |
| GAP-141 | drv_not_implemented | preprocessor/modules | `-fdump-ast of a module file is not implemented yet` | `crates/kiln_driver/src/syntax_cmd.rs::dump_options` | 0 | WP-1744 |
| GAP-142 | parse_unsupported | preprocessor/modules | `{what} (the floating-point environment) is not supported yet` ({what}: `'#pragma clang fp exceptions(v)'`, `'#pragma clang fp eval_method(v)'`, `'#pragma STDC FENV_ACCESS ON'`, `'#pragma STDC FENV_ROUND m'`, `'#pragma float_control(except, on)'`) | `crates/kiln_parse/src/fp_pragma.rs::fenv_unsupported` | 0 | WP-1744 |
| GAP-143 | parse_unsupported | preprocessor/modules | `'#pragma {what}' is not supported yet` ({what}: `align`, `options`, `clang section`, `pack` (the forms clang diagnoses)) | `crates/kiln_sema/src/decl.rs::act_on_unsupported_pragma` | 0 | WP-1744 |
| GAP-144 | sema_multiversion_not_supported | misc | `function multiversioning with '{0}' is not supported yet` ({0}: `target_version`, `target`, or the attribute name `fmv_attached` computes) | `crates/kiln_sema/src/class/mod.rs::act_on_friend`, `crates/kiln_sema/src/decl.rs::act_on_declarator_impl`, `crates/kiln_sema/src/fmv.rs::check_fmv_attrs`, `crates/kiln_sema/src/fmv.rs::fmv_attached` | 0 | WP-1745 |
| GAP-145 | parse_unsupported | misc | `this kind of member access is not supported yet` | `crates/kiln_sema/src/class/member_expr.rs::member_of_object` ×2 | 0 | WP-1745 |
| GAP-146 | parse_unsupported | misc | `calls through this function type is not supported yet` | `crates/kiln_sema/src/class/member_expr.rs::try_member_call`, `crates/kiln_sema/src/expr.rs::act_on_call` | 0 | WP-1745 |
| GAP-147 | parse_unsupported | misc | `constructors is not supported yet` / `overloaded operators is not supported yet` | `crates/kiln_sema/src/class/special/declare.rs::act_on_special_declarator` ×2 | 0 | WP-1745 |
| GAP-148 | drv_not_implemented | misc | `{what} is not implemented yet` ({what}: `-### with -E or -fsyntax-only`, `-fdump-ast without -fsyntax-only`, `-fdump-record-layouts without -fsyntax-only`, `-fdump-vtable-layouts without -fsyntax-only`, `-fkiln-dump-cfg without -fsyntax-only`, `-fkiln-cfg-stats without -fsyntax-only`, `-verify without -fsyntax-only`) | `crates/kiln_driver/src/lib.rs::check_action` | 0 | WP-1745 |
| GAP-149 | sema_attribute_unsupported_by_kiln | misc | `'{0}' attribute is not supported by kiln yet` ({0}: the class-A rows of `attr_triage` (`address_space`, `cpu_dispatch`, `cpu_specific`, `interrupt`, `matrix_type`, `model`, the `opencl_*` address spaces, `riscv_rvv_vector_bits`, the `swift_*` parameter ABIs, `swiftasynccall`, `swiftcall`, `target_clones`, `target_version`) and x86 `interrupt`) | `crates/kiln_sema/src/decl.rs::attribute_node` | 0 | WP-1745 |
| GAP-150 | sema_vla_unsupported | misc | `variable length arrays are not supported yet` | `crates/kiln_sema/src/decl.rs::folded_array_bound` | 0 | WP-1745 |
| GAP-151 | parse_unsupported | misc | `range-based for over this array type is not supported yet` | `crates/kiln_sema/src/vla.rs::vla_range_count` | 0 | WP-1745 |
| GAP-152 | parse_unsupported | misc | `'__declspec' is not supported yet` | `crates/kiln_parse/src/attr.rs::parse_attributes` | 0 | WP-1745 |
| GAP-153 | parse_unsupported | misc | `qualified member initializers is not supported yet` | `crates/kiln_parse/src/class.rs::parse_mem_initializer` | 0 | WP-1745 |
| GAP-154 | parse_unsupported | misc | `a destructor name in a using-declarator is not supported yet` | `crates/kiln_parse/src/decl.rs::parse_using_declaration` | 0 | WP-1745 |
| GAP-155 | parse_unsupported | misc | `'template' expressions is not supported yet` | `crates/kiln_parse/src/expr.rs::parse_primary_expr` | 0 | WP-1745 |
| GAP-156 | parse_unsupported | misc | `lambda expressions is not supported yet` | `crates/kiln_parse/src/new_delete.rs::parse_delete_expr` | 0 | WP-1745 |
| GAP-157 | parse_unsupported | misc | `'{kw}' here is not supported yet` ({kw}: `namespace`, `template`) | `crates/kiln_parse/src/stmt.rs::parse_statement_after_attrs_inner` | 0 | WP-1745 |
| GAP-158 | parse_unsupported | misc | `a braced argument to a builtin function is not supported yet` | `crates/kiln_sema/src/builtins.rs::builtin_custom_call` | 0 | WP-1745 |
| GAP-159 | parse_unsupported | misc | `a braced argument to __builtin_addressof is not supported yet` | `crates/kiln_sema/src/builtins.rs::builtin_addressof_call` | 0 | WP-1745 |
| GAP-160 | parse_unsupported | misc | `this builtin function is not supported yet` | `crates/kiln_sema/src/builtins.rs::builtin_custom_call` | 0 | WP-1745 |
| GAP-161 | parse_unsupported | misc | `pointer to member of reference type is not supported yet` | `crates/kiln_sema/src/class/member_expr.rs::member_pointer_of` | 0 | WP-1745 |
| GAP-162 | parse_unsupported | misc | `{what} is not supported yet` ({what}: `constructors`, `destructors`, `conversion functions`, `overloaded operators`, `literal operators`, `deduction guides here`) | `crates/kiln_sema/src/decl.rs::unsupported_declarator` | 0 | WP-1745 |
| GAP-163 | parse_unsupported | misc | `'explicit' is not supported yet` | `crates/kiln_sema/src/decl.rs::unsupported_declarator` | 0 | WP-1745 |
| GAP-164 | parse_unsupported | misc | `pure virtual friend functions is not supported yet` | `crates/kiln_sema/src/class/virtual/mod.rs::virtual_decl` | 0 | WP-1745 |
| GAP-165 | parse_unsupported | misc | `language linkage "{lang}" is not supported yet` ({lang}: any language other than `C` and `C++`) | `crates/kiln_sema/src/decl.rs::act_on_start_linkage_spec` | 0 | WP-1745 |
| GAP-166 | parse_unsupported | misc | `'selectany' on an 'extern' declaration without an initializer is not supported yet` | `crates/kiln_sema/src/selectany.rs::check_selectany_extern` | 0 | WP-1745 |
| GAP-167 | parse_unsupported | misc | `this kind of name is not supported yet` | `crates/kiln_sema/src/expr.rs::build_decl_ref_extra` | 0 | WP-1745 |

## Excluded (clang-matching wording)

Not counted in the totals: clang reports the same limit with the same
words, so these are not kiln gaps.

| gap | family | area | text | sites | reach | row |
|---|---|---|---|---|---|---|
| GAP-168 | codegen fallback | excluded | `cannot compile this tail call skipping over cleanups yet` | `crates/kiln_codegen/src/expr.rs::finish_musttail` | 0 | excluded: clang's own wording and limit (`[[clang::musttail]]` over cleanups) |
| GAP-169 | Oven trap | excluded | `constexpr bit cast involving type '{n}' is not yet supported` | `crates/kiln_oven/src/compile/bitcast.rs::bit_cast_unsupported` ×2 | 0 | excluded: clang's note `note_constexpr_bit_cast_unsupported_type`, same cases |
| GAP-170 | Oven trap | excluded | `non-trivial destruction of lifetime-extended temporary with type '{t}' used in the result of a constant expression is not yet supported` | `crates/kiln_oven/src/value.rs::check_let_destruction` | 0 | excluded: clang's note `note_constexpr_unsupported_temporary_nontrivial_dtor`, same cases |
| GAP-041 | cg_cannot_mangle_expression | excluded | `cannot yet mangle expression type {kind}` ({kind}: the kinds clang has no encoding for either, under Enumerated kinds) | `crates/kiln_codegen/src/module.rs::run_with_stats` | 0 | excluded: clang's own error (`CXXNameMangler::mangleExpression`) for the same kinds; every kind clang mangles has its encoding (WP-1736, spec/abi.md §8.15) |

## Enumerated kinds

The kinds an exhaustive match arm sends to the gap (WP-1719 replaced each
`_ =>` arm with this list, so a new variant needs a decision).

### GAP-041

`Mangler::expression_node` (`crates/kiln_abi/src/mangle/dep.rs`), the
kinds clang reports the same way (`mangleExpression`): `ChooseExpr`,
`CompoundLiteralExpr`, `ExtVectorElementExpr`, `GenericSelectionExpr`,
`OffsetOfExpr`, `PredefinedExpr`, `ShuffleVectorExpr`,
`ConvertVectorExpr`, `StmtExpr`, `ArrayTypeTraitExpr`,
`ExpressionTraitExpr`, `VAArgExpr`, `PseudoObjectExpr`, `AtomicExpr`,
`SourceLocExpr`, `EmbedExpr`, `BuiltinBitCastExpr`; with clang's other
errors, `BinaryConditionalOperator` (`?: operator with omitted middle
operand cannot be mangled`) and `UnaryExprOrTypeTraitExpr` of
`__datasizeof` / `vec_step` (`cannot yet mangle __datasizeof
expression`). Never in a mangled operand (clang: unreachable): `ParenExpr`,
`ImplicitCastExpr`, `ConstantExpr`, `MaterializeTemporaryExpr`,
`CXXBindTemporaryExpr`, `ExprWithCleanups`, `SubstNonTypeTemplateParmExpr`,
`CXXDefaultArgExpr`, `CXXDefaultInitExpr` (stepped over),
`OpaqueValueExpr`, `ParenListExpr`, `ImplicitValueInitExpr`,
`CXXInheritedCtorInitExpr`, `CXXParenListInitExpr`, `ArrayInitLoopExpr`,
`ArrayInitIndexExpr`, `NoInitExpr`, `DesignatedInitUpdateExpr`,
`RecoveryExpr`, `UserDefinedLiteral` and `UnresolvedMemberExpr` (kiln
builds a `CallExpr` and a `MemberExpr`), `CXXExpansionSelectExpr`,
`AddrLabelExpr` (ExprKind).

### GAP-059

`UserDefinedLiteral`, `ImaginaryLiteral`, `EmbedExpr`, `UnresolvedLookupExpr`, `UnresolvedMemberExpr`, `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr`, `SubstNonTypeTemplateParmPackExpr`, `FunctionParmPackExpr`, `ParenListExpr`, `InitListExpr`, `DesignatedInitExpr`, `CXXConstructExpr`, `CXXTemporaryObjectExpr`, `CXXInheritedCtorInitExpr`, `CXXParenListInitExpr`, `ArrayInitLoopExpr`, `CXXStdInitializerListExpr`, `NoInitExpr`, `DesignatedInitUpdateExpr`, `CXXBindTemporaryExpr`, `CXXPseudoDestructorExpr`, `CXXTypeidExpr`, `SizeOfPackExpr`, `LambdaExpr`, `PackExpansionExpr`, `CXXFoldExpr`, `PackIndexingExpr`, `CXXUnresolvedConstructExpr`, `DependentCoawaitExpr`, `ChooseExpr`, `CXXReflectExpr`, `CXXSpliceExpr`, `RecoveryExpr`, `CXXExpansionSelectExpr`, `GenericSelectionExpr`, `IntegerPackExpr` (ExprKind).

### GAP-063

`Dependent`, `ToUnion`, `IntegralRealToComplex`, `FloatingRealToComplex`, `IntegralComplexCast`, `FloatingComplexCast`, `IntegralComplexToFloatingComplex`, `FloatingComplexToIntegralComplex` (CastKind).

### GAP-065

`PtrMemD`, `PtrMemI`, `Cmp`, `LAnd`, `LOr`, `Assign`, `MulAssign`, `DivAssign`, `RemAssign`, `AddAssign`, `SubAssign`, `ShlAssign`, `ShrAssign`, `AndAssign`, `XorAssign`, `OrAssign`, `Comma` (BinaryOp).

### GAP-066

`TranslationUnit`, `Namespace`, `NamespaceAlias`, `LinkageSpec`, `Export`, `Empty`, `FileScopeAsm`, `UsingDirective`, `Using`, `UsingEnum`, `UsingShadow`, `ConstructorUsingShadow`, `UsingPack`, `UnresolvedUsingValue`, `UnresolvedUsingTypename`, `UnresolvedUsingIfExists`, `Typedef`, `TypeAlias`, `Enum`, `EnumConstant`, `CXXRecord`, `ClassTemplateSpecialization`, `ClassTemplatePartialSpecialization`, `Field`, `IndirectField`, `AccessSpec`, `Friend`, `FriendTemplate`, `CXXConstructor`, `CXXDestructor`, `CXXDeductionGuide`, `ImplicitParam`, `VarTemplateSpecialization`, `VarTemplatePartialSpecialization`, `ClassTemplate`, `FunctionTemplate`, `VarTemplate`, `TypeAliasTemplate`, `Concept`, `BuiltinTemplate`, `TemplateTypeParm`, `NonTypeTemplateParm`, `TemplateTemplateParm`, `ImplicitConceptSpecialization`, `RequiresExprBody`, `LifetimeExtendedTemporary`, `StaticAssert`, `Label`, `ConstevalBlock` (DeclKind).

### GAP-067

`IntegerLiteral`, `FloatingLiteral`, `CharacterLiteral`, `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr`, `UserDefinedLiteral`, `ImaginaryLiteral`, `EmbedExpr`, `GNUNullExpr`, `CXXThisExpr`, `SourceLocExpr`, `UnresolvedLookupExpr`, `UnresolvedMemberExpr`, `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr`, `SubstNonTypeTemplateParmPackExpr`, `FunctionParmPackExpr`, `CXXMemberCallExpr`, `ParenListExpr`, `ImplicitValueInitExpr`, `DesignatedInitExpr`, `CXXConstructExpr`, `CXXTemporaryObjectExpr`, `CXXInheritedCtorInitExpr`, `CXXScalarValueInitExpr`, `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `CXXStdInitializerListExpr`, `CompoundLiteralExpr`, `NoInitExpr`, `DesignatedInitUpdateExpr`, `CXXBindTemporaryExpr`, `CXXNewExpr`, `CXXDeleteExpr`, `CXXPseudoDestructorExpr`, `CXXThrowExpr`, `CXXNoexceptExpr`, `UnaryExprOrTypeTraitExpr`, `TypeTraitExpr`, `ArrayTypeTraitExpr`, `ExpressionTraitExpr`, `OffsetOfExpr`, `SizeOfPackExpr`, `RequiresExpr`, `ConceptSpecializationExpr`, `LambdaExpr`, `PackExpansionExpr`, `CXXFoldExpr`, `PackIndexingExpr`, `CXXUnresolvedConstructExpr`, `DependentCoawaitExpr`, `StmtExpr`, `AddrLabelExpr`, `ChooseExpr`, `VAArgExpr`, `AtomicExpr`, `ShuffleVectorExpr`, `ConvertVectorExpr`, `CXXReflectExpr`, `CXXSpliceExpr`, `RecoveryExpr`, `CXXExpansionSelectExpr`, `GenericSelectionExpr`, `IntegerPackExpr` (ExprKind).

### GAP-068

`PostInc`, `PostDec`, `AddrOf`, `Plus`, `Minus`, `Not`, `LNot`, `Coawait` (UnaryOp).

### GAP-069

`Mul`, `Div`, `Rem`, `Add`, `Sub`, `Shl`, `Shr`, `Cmp`, `LT`, `GT`, `LE`, `GE`, `EQ`, `NE`, `And`, `Xor`, `Or`, `LAnd`, `LOr`, `MulAssign`, `DivAssign`, `RemAssign`, `AddAssign`, `SubAssign`, `ShlAssign`, `ShrAssign`, `AndAssign`, `XorAssign`, `OrAssign` (BinaryOp).

### GAP-070

`Dependent`, `LValueToRValueBitCast`, `BitCast`, `ArrayToPointerDecay`, `FunctionToPointerDecay`, `BuiltinFnToFnPtr`, `NullToPointer`, `NullToMemberPointer`, `IntegralToPointer`, `PointerToIntegral`, `PointerToBoolean`, `IntegralCast`, `IntegralToBoolean`, `BooleanToSignedIntegral`, `IntegralToFloating`, `FloatingToIntegral`, `FloatingToBoolean`, `FloatingCast`, `DerivedToBaseMemberPointer`, `BaseToDerivedMemberPointer`, `MemberPointerToBoolean`, `ReinterpretMemberPointer`, `ConstructorConversion`, `ToVoid`, `ToUnion`, `VectorSplat`, `NonAtomicToAtomic`, `AtomicToNonAtomic`, `IntegralRealToComplex`, `FloatingRealToComplex`, `IntegralComplexCast`, `FloatingComplexCast`, `IntegralComplexToReal`, `FloatingComplexToReal`, `IntegralComplexToBoolean`, `FloatingComplexToBoolean`, `IntegralComplexToFloatingComplex`, `FloatingComplexToIntegralComplex` (CastKind).

### GAP-071

`IntegerLiteral`, `FloatingLiteral`, `CharacterLiteral`, `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr`, `UserDefinedLiteral`, `ImaginaryLiteral`, `EmbedExpr`, `GNUNullExpr`, `CXXThisExpr`, `PredefinedExpr`, `SourceLocExpr`, `UnresolvedLookupExpr`, `UnresolvedMemberExpr`, `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr`, `SubstNonTypeTemplateParmExpr`, `SubstNonTypeTemplateParmPackExpr`, `FunctionParmPackExpr`, `BinaryOperator`, `CompoundAssignOperator`, `ParenListExpr`, `ImplicitCastExpr`, `CStyleCastExpr`, `CXXFunctionalCastExpr`, `CXXStaticCastExpr`, `CXXConstCastExpr`, `CXXReinterpretCastExpr`, `CXXDynamicCastExpr`, `BuiltinBitCastExpr`, `DesignatedInitExpr`, `ArrayInitIndexExpr`, `OpaqueValueExpr`, `CXXNewExpr`, `CXXDeleteExpr`, `CXXPseudoDestructorExpr`, `CXXNoexceptExpr`, `CXXTypeidExpr`, `UnaryExprOrTypeTraitExpr`, `TypeTraitExpr`, `ArrayTypeTraitExpr`, `ExpressionTraitExpr`, `OffsetOfExpr`, `SizeOfPackExpr`, `RequiresExpr`, `ConceptSpecializationExpr`, `PackExpansionExpr`, `CXXFoldExpr`, `PackIndexingExpr`, `CXXUnresolvedConstructExpr`, `DependentCoawaitExpr`, `AddrLabelExpr`, `ChooseExpr`, `ShuffleVectorExpr`, `ConvertVectorExpr`, `ExtVectorElementExpr`, `CXXReflectExpr`, `CXXSpliceExpr`, `RecoveryExpr`, `CXXExpansionSelectExpr`, `GenericSelectionExpr`, `IntegerPackExpr` (ExprKind).

### GAP-072

`TranslationUnit`, `Namespace`, `NamespaceAlias`, `LinkageSpec`, `Export`, `Empty`, `FileScopeAsm`, `UsingDirective`, `Using`, `UsingEnum`, `UsingShadow`, `ConstructorUsingShadow`, `UsingPack`, `UnresolvedUsingValue`, `UnresolvedUsingTypename`, `UnresolvedUsingIfExists`, `Typedef`, `TypeAlias`, `Enum`, `EnumConstant`, `CXXRecord`, `ClassTemplateSpecialization`, `ClassTemplatePartialSpecialization`, `AccessSpec`, `Friend`, `FriendTemplate`, `Function`, `CXXConstructor`, `CXXDestructor`, `CXXDeductionGuide`, `Var`, `ParmVar`, `ImplicitParam`, `Binding`, `VarTemplateSpecialization`, `VarTemplatePartialSpecialization`, `ClassTemplate`, `FunctionTemplate`, `VarTemplate`, `TypeAliasTemplate`, `Concept`, `BuiltinTemplate`, `TemplateTypeParm`, `NonTypeTemplateParm`, `TemplateTemplateParm`, `ImplicitConceptSpecialization`, `RequiresExprBody`, `LifetimeExtendedTemporary`, `TemplateParamObject`, `StaticAssert`, `Label`, `ContractResultName`, `ConstevalBlock` (DeclKind).

### GAP-075

`IntegerLiteral`, `FloatingLiteral`, `CharacterLiteral`, `StringLiteral`, `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr`, `UserDefinedLiteral`, `EmbedExpr`, `GNUNullExpr`, `DeclRefExpr`, `MemberExpr`, `CXXThisExpr`, `PredefinedExpr`, `SourceLocExpr`, `UnresolvedLookupExpr`, `UnresolvedMemberExpr`, `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr`, `SubstNonTypeTemplateParmPackExpr`, `FunctionParmPackExpr`, `BinaryConditionalOperator`, `ArraySubscriptExpr`, `CallExpr`, `CXXMemberCallExpr`, `CXXOperatorCallExpr`, `ParenListExpr`, `DesignatedInitExpr`, `CXXConstructExpr`, `CXXTemporaryObjectExpr`, `CXXInheritedCtorInitExpr`, `CXXParenListInitExpr`, `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `CXXStdInitializerListExpr`, `NoInitExpr`, `DesignatedInitUpdateExpr`, `MaterializeTemporaryExpr`, `CXXBindTemporaryExpr`, `CXXNewExpr`, `CXXDeleteExpr`, `CXXPseudoDestructorExpr`, `CXXThrowExpr`, `CXXNoexceptExpr`, `CXXTypeidExpr`, `UnaryExprOrTypeTraitExpr`, `TypeTraitExpr`, `ArrayTypeTraitExpr`, `ExpressionTraitExpr`, `OffsetOfExpr`, `SizeOfPackExpr`, `RequiresExpr`, `ConceptSpecializationExpr`, `LambdaExpr`, `PackExpansionExpr`, `CXXFoldExpr`, `PackIndexingExpr`, `CXXUnresolvedConstructExpr`, `CoawaitExpr`, `DependentCoawaitExpr`, `CoyieldExpr`, `AddrLabelExpr`, `ChooseExpr`, `AtomicExpr`, `ShuffleVectorExpr`, `ConvertVectorExpr`, `ExtVectorElementExpr`, `CXXReflectExpr`, `CXXSpliceExpr`, `RecoveryExpr`, `CXXExpansionSelectExpr`, `GenericSelectionExpr`, `IntegerPackExpr` (ExprKind).

### GAP-076

`Dependent`, `BitCast`, `ArrayToPointerDecay`, `FunctionToPointerDecay`, `BuiltinFnToFnPtr`, `NullToPointer`, `NullToMemberPointer`, `IntegralToPointer`, `PointerToIntegral`, `PointerToBoolean`, `IntegralCast`, `IntegralToBoolean`, `BooleanToSignedIntegral`, `IntegralToFloating`, `FloatingToIntegral`, `FloatingToBoolean`, `FloatingCast`, `DerivedToBase`, `UncheckedDerivedToBase`, `BaseToDerived`, `Dynamic`, `DerivedToBaseMemberPointer`, `BaseToDerivedMemberPointer`, `MemberPointerToBoolean`, `ReinterpretMemberPointer`, `ToVoid`, `ToUnion`, `VectorSplat`, `IntegralComplexToReal`, `FloatingComplexToReal`, `IntegralComplexToBoolean`, `FloatingComplexToBoolean` (CastKind).

### GAP-077

`AddrOf`, `Deref`, `LNot`, `Real`, `Imag`, `Coawait` (UnaryOp).

### GAP-078

`PtrMemD`, `PtrMemI`, `Rem`, `Shl`, `Shr`, `Cmp`, `LT`, `GT`, `LE`, `GE`, `EQ`, `NE`, `And`, `Xor`, `Or`, `LAnd`, `LOr`, `MulAssign`, `DivAssign`, `RemAssign`, `AddAssign`, `SubAssign`, `ShlAssign`, `ShrAssign`, `AndAssign`, `XorAssign`, `OrAssign` (BinaryOp).

### GAP-081

`IncompleteArray`, `VariableArray` (TypeView).

### GAP-084

`Void`, `ScalableVector`, `TargetExt` (IrType).

### GAP-092

`Invalid`, `Builtin`, `BitInt`, `DependentBitInt`, `LValueRef`, `RValueRef`, `VariableArray`, `DependentSizedArray`, `Record`, `UnresolvedUsing`, `Paren`, `Atomic`, `DependentVector`, `ExtQual`, `Typedef`, `Using`, `Elaborated`, `QualifiedArray`, `TemplateTypeParm`, `CanonTemplateTypeParm`, `SubstTemplateTypeParm`, `SubstTemplateTypeParmPack`, `TemplateSpecialization`, `InjectedClassName`, `DependentName`, `DependentTemplateSpecialization`, `PackExpansion`, `PackIndexing`, `Decltype`, `DependentDecltype`, `TypeOf`, `UnaryTransform`, `Auto`, `DeducedTemplateSpecialization`, `Attributed`, `Splice` (TypeView).

### GAP-097

`IntegerLiteral`, `FloatingLiteral`, `CharacterLiteral`, `StringLiteral`, `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr`, `UserDefinedLiteral`, `ImaginaryLiteral`, `EmbedExpr`, `GNUNullExpr`, `UnresolvedMemberExpr`, `ParenListExpr`, `InitListExpr`, `ImplicitValueInitExpr`, `DesignatedInitExpr`, `CXXInheritedCtorInitExpr`, `CXXDefaultArgExpr`, `CXXDefaultInitExpr`, `CXXScalarValueInitExpr`, `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `NoInitExpr`, `DesignatedInitUpdateExpr`, `OpaqueValueExpr`, `PseudoObjectExpr` (ExprKind).

### GAP-117

`StringLiteral`, `UserDefinedLiteral`, `ImaginaryLiteral`, `EmbedExpr`, `MemberExpr`, `PredefinedExpr`, `UnresolvedLookupExpr`, `UnresolvedMemberExpr`, `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr`, `SubstNonTypeTemplateParmPackExpr`, `FunctionParmPackExpr`, `ArraySubscriptExpr`, `CallExpr`, `CXXMemberCallExpr`, `CXXOperatorCallExpr`, `ParenListExpr`, `DesignatedInitExpr`, `CXXConstructExpr`, `CXXTemporaryObjectExpr`, `CXXInheritedCtorInitExpr`, `CXXParenListInitExpr`, `ArrayInitLoopExpr`, `CXXStdInitializerListExpr`, `NoInitExpr`, `DesignatedInitUpdateExpr`, `MaterializeTemporaryExpr`, `OpaqueValueExpr`, `CXXPseudoDestructorExpr`, `CXXTypeidExpr`, `SizeOfPackExpr`, `LambdaExpr`, `PackExpansionExpr`, `CXXFoldExpr`, `PackIndexingExpr`, `CXXUnresolvedConstructExpr`, `CoawaitExpr`, `DependentCoawaitExpr`, `CoyieldExpr`, `ChooseExpr`, `VAArgExpr`, `AtomicExpr`, `ShuffleVectorExpr`, `ConvertVectorExpr`, `CXXSpliceExpr`, `RecoveryExpr`, `CXXExpansionSelectExpr`, `GenericSelectionExpr`, `IntegerPackExpr` (ExprKind).

### GAP-118

`Dependent`, `LValueBitCast`, `BuiltinFnToFnPtr`, `BooleanToSignedIntegral`, `ToUnion`, `VectorSplat`, `IntegralRealToComplex`, `FloatingRealToComplex`, `IntegralComplexCast`, `FloatingComplexCast`, `IntegralComplexToFloatingComplex`, `FloatingComplexToIntegralComplex` (CastKind).

### GAP-119

`IntegerLiteral`, `FloatingLiteral`, `CharacterLiteral`, `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr`, `UserDefinedLiteral`, `ImaginaryLiteral`, `EmbedExpr`, `GNUNullExpr`, `DeclRefExpr`, `MemberExpr`, `CXXThisExpr`, `PredefinedExpr`, `SourceLocExpr`, `UnresolvedLookupExpr`, `UnresolvedMemberExpr`, `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr`, `SubstNonTypeTemplateParmExpr`, `SubstNonTypeTemplateParmPackExpr`, `FunctionParmPackExpr`, `UnaryOperator`, `BinaryOperator`, `CompoundAssignOperator`, `ArraySubscriptExpr`, `ParenExpr`, `CallExpr`, `CXXMemberCallExpr`, `CXXOperatorCallExpr`, `CXXRewrittenBinaryOperator`, `ParenListExpr`, `ImplicitCastExpr`, `CStyleCastExpr`, `CXXFunctionalCastExpr`, `CXXStaticCastExpr`, `CXXConstCastExpr`, `CXXReinterpretCastExpr`, `CXXDynamicCastExpr`, `BuiltinBitCastExpr`, `ImplicitValueInitExpr`, `DesignatedInitExpr`, `CXXConstructExpr`, `CXXTemporaryObjectExpr`, `CXXInheritedCtorInitExpr`, `CXXDefaultArgExpr`, `CXXDefaultInitExpr`, `CXXScalarValueInitExpr`, `CXXParenListInitExpr`, `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `CXXStdInitializerListExpr`, `CompoundLiteralExpr`, `NoInitExpr`, `DesignatedInitUpdateExpr`, `MaterializeTemporaryExpr`, `CXXBindTemporaryExpr`, `ExprWithCleanups`, `OpaqueValueExpr`, `CXXNewExpr`, `CXXDeleteExpr`, `CXXPseudoDestructorExpr`, `CXXThrowExpr`, `CXXNoexceptExpr`, `CXXTypeidExpr`, `UnaryExprOrTypeTraitExpr`, `TypeTraitExpr`, `ArrayTypeTraitExpr`, `ExpressionTraitExpr`, `OffsetOfExpr`, `SizeOfPackExpr`, `RequiresExpr`, `ConceptSpecializationExpr`, `PackExpansionExpr`, `CXXFoldExpr`, `PackIndexingExpr`, `CXXUnresolvedConstructExpr`, `CoawaitExpr`, `DependentCoawaitExpr`, `CoyieldExpr`, `ConstantExpr`, `AddrLabelExpr`, `ChooseExpr`, `VAArgExpr`, `AtomicExpr`, `ShuffleVectorExpr`, `ConvertVectorExpr`, `ExtVectorElementExpr`, `CXXReflectExpr`, `CXXSpliceExpr`, `RecoveryExpr`, `CXXExpansionSelectExpr`, `PseudoObjectExpr`, `GenericSelectionExpr`, `IntegerPackExpr` (ExprKind).
