# Static gaps (WP-1719, 2026-11-20)

Every place in kiln's source that reports a language corner kiln knows it
does not handle yet: a "not supported yet" or "not yet implemented"
diagnostic, a mangling failure, a code generation fallback, an Oven trap,
a reflection function without an implementation. One entry per gap, keyed
by a stable id (`GAP-NNN`), each pointing at the BACKLOG row that fixes it.
`cargo xtask ci --step lints` keeps the list exact (spec/harness.md "CI",
step 0): a new site needs an entry and a row, and a fixed gap must leave.

22 gaps at 31 sites; 10 of 56,766 g++.dg cases hit one at the
1.3.0 release candidate, all since fixed. 4 more entries (5 sites) use
wording that matches clang's for a limit clang shares: they are listed
as excluded and stay.

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
| Oven trap | 4 | 8 | 0 |
| codegen fallback | 1 | 1 | 0 |
| drv_not_implemented | 2 | 2 | 0 |
| parse_unsupported | 6 | 6 | 0 |
| pp_expr_unsupported | 1 | 3 | 0 |
| pp_header_unit_unsupported | 1 | 1 | 0 |
| pp_scan_header_unit | 1 | 1 | 0 |
| reflection error | 3 | 3 | 0 |
| sema_attribute_unsupported_by_kiln | 1 | 1 | 0 |
| sema_multiversion_not_supported | 1 | 4 | 0 |
| sema_unimplemented_template_feature | 1 | 1 | 0 |

By area (the BACKLOG row that fixes it, most-hit first):

| area | gaps | sites | reach | row |
|---|---|---|---|---|
| codegen fallbacks | 1 (GAP-062) | 1 | 0 | WP-1739 |
| NTTP/substitution kinds | 1 (GAP-097) | 1 | 0 | WP-1740 |
| Oven traps: expressions | 2 (GAP-114..GAP-115) | 3 | 0 | WP-1741 |
| Oven traps: types | 2 (GAP-126, GAP-129) | 5 | 0 | WP-1742 |
| reflection | 5 (GAP-130..GAP-134) | 5 | 0 | WP-1743 |
| preprocessor/modules | 9 (GAP-135..GAP-143) | 11 | 0 | WP-1744 |
| misc | 2 (GAP-144, GAP-149) | 5 | 0 | WP-1745 |

## Gaps

| gap | family | area | text | sites | reach | row |
|---|---|---|---|---|---|---|
| GAP-062 | codegen fallback | codegen fallbacks | `code generation for SVE builtin '{name}' (called as '{called}') is not supported yet` | `crates/kiln_codegen/src/sve.rs::emit_sve_alias` | 0 | WP-1739 |
| GAP-097 | sema_unimplemented_template_feature | NTTP/substitution kinds | `kiln: not yet implemented: substitution into {k}` ({k}: the 25 kinds of the arm, under Enumerated kinds) | `crates/kiln_sema/src/templ/subst/exprs.rs::ex` | 0 | WP-1740 |
| GAP-114 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: `CXXDeleteExpr` of a `void *` operand, which clang 22 destroys and frees by the allocation's dynamic type) | `crates/kiln_oven/src/compile/new_delete.rs::delete_expr` | 0 | WP-1741 |
| GAP-115 | Oven trap | Oven traps: expressions | `kiln: constant evaluation of {k} is not supported yet` ({k}: `__builtin_align_up`/`_down`/`__builtin_is_aligned` or an elementwise builtin on a `_BitInt` wider than 128 bits) | `crates/kiln_oven/src/compile/builtins.rs::builtin_call` ×2 | 0 | WP-1741 |
| GAP-126 | Oven trap | Oven traps: types | `kiln: constant evaluation of {k} is not supported yet` ({k}: a write to, or a splat into, a lane of a vector of wide `_BitInt` lanes) | `crates/kiln_oven/src/compile/wide.rs::wide_compound_assign`, `crates/kiln_oven/src/compile/wide.rs::wide_float_compound`, `crates/kiln_oven/src/compile/wide.rs::wide_store`, `crates/kiln_oven/src/compile/wide.rs::wide_to_scalar` | 0 | WP-1742 |
| GAP-129 | Oven trap | Oven traps: types | `kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet` (a wide operand of a builtin call or of `new T[n]`, a lane of a vector of wide lanes) | `crates/kiln_oven/src/compile.rs::wide_bit_int` | 0 | WP-1742 |
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
| GAP-149 | sema_attribute_unsupported_by_kiln | misc | `'{0}' attribute is not supported by kiln yet` ({0}: the class-A rows of `attr_triage` (`address_space`, `cpu_dispatch`, `cpu_specific`, `interrupt`, `model`, the `opencl_*` address spaces, the `swift_*` parameter ABIs, `swiftasynccall`, `swiftcall`, `target_clones`, `target_version`) and x86 `interrupt`) | `crates/kiln_sema/src/decl.rs::attribute_node` | 0 | WP-1745 |

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

### GAP-097

`IntegerLiteral`, `FloatingLiteral`, `CharacterLiteral`, `StringLiteral`, `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr`, `UserDefinedLiteral`, `ImaginaryLiteral`, `EmbedExpr`, `GNUNullExpr`, `UnresolvedMemberExpr`, `ParenListExpr`, `InitListExpr`, `ImplicitValueInitExpr`, `DesignatedInitExpr`, `CXXInheritedCtorInitExpr`, `CXXDefaultArgExpr`, `CXXDefaultInitExpr`, `CXXScalarValueInitExpr`, `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `NoInitExpr`, `DesignatedInitUpdateExpr`, `OpaqueValueExpr`, `PseudoObjectExpr` (ExprKind).
