# Static gaps (WP-1719, 2026-11-20)

Every place in kiln's source that reports a language corner kiln knows it
does not handle yet: a "not supported yet" or "not yet implemented"
diagnostic, a mangling failure, a code generation fallback, an Oven trap,
a reflection function without an implementation. One entry per gap, keyed
by a stable id (`GAP-NNN`), each pointing at the BACKLOG row that fixes it.
`cargo xtask ci --step lints` keeps the list exact (spec/harness.md "CI",
step 0): a new site needs an entry and a row, and a fixed gap must leave.

1 gap at 1 site; 10 of 56,766 g++.dg cases hit one at the
1.3.0 release candidate, all since fixed. 4 more entries (5 sites) use
wording that matches clang's for a limit clang shares: they are listed
as excluded and stay.

## Method

A *site* is a line in a crate's `src/` (test modules excluded) that
reports a gap:

- a use of a gap diagnostic: `cg_cannot_mangle_expression`,
  `drv_not_implemented`, `parse_unsupported`,
  `sema_attribute_unsupported_by_kiln`, `sema_conv_unsupported`,
  `sema_unimplemented_template_feature`, `sema_vla_unsupported`;
- a call of a function that forwards its text to one: `unsupported`,
  `templ_unimplemented` and `not_implemented` in `kiln_sema`,
  `unsupported` and `not_implemented` in `kiln_oven`;
- a string literal holding "not supported yet", "not yet supported", "not
  yet implemented", "not implemented yet", "cannot yet", "support header
  units yet", or both "cannot compile" and " yet" (the codegen fallbacks,
  the Oven traps, reflection);
- `todo!` or `unimplemented!` (none today).

A *spec site* is a paragraph of a `spec/*.md` contract (the
`spec/landed/` records excluded) whose bold lead is a gap phrase
(`static_gaps::SPEC_LEADS`: `**Not implemented`, `**Not yet
implemented`, `**Not supported`, `**Not yet supported`,
`**Unimplemented`, `**Unsupported`, at the start of a line after any
list marker, outside fenced code). It catches a gap that has no text of
its own in the code: a match arm that sends a valid construct to a
diagnostic kept for invalid ones, as `constant_of` of an explicit-object
member function went to "not a valid argument to a splice-expression"
until WP-1782, which the source scan cannot tell from a correct failure.
A spec site carries `<!-- kiln-gap: GAP-NNN -->` on its line or the line
directly above; its sites cell names it `spec/<file>.md::§<heading>`
(the nearest heading above). WP-1782 found 7 such paragraphs: the
explicit-object gap (fixed), 4 stale ones (closure template-heads,
GNU statements, new-expression forms, `_Complex _Float16`: implemented
since, rewritten), a missing warning (relabelled "Missing warnings",
not a language corner) and the 2 excluded entries GAP-171 and GAP-172.

Each site carries a marker comment on its line or in the comment block
directly above it: `// kiln-gap: GAP-NNN` names its entry; `// kiln-gap:
forward` marks the body of a forwarding function (its callers are the
sites; 5 today); `// kiln-gap: check` marks a line that only compares a
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
| sema_attribute_unsupported_by_kiln | 1 | 1 | 0 |

By area (the BACKLOG row that fixes it, most-hit first):

| area | gaps | sites | reach | row |
|---|---|---|---|---|
| misc | 1 (GAP-149) | 1 | 0 | WP-1745 |

## Gaps

| gap | family | area | text | sites | reach | row |
|---|---|---|---|---|---|---|
| GAP-149 | sema_attribute_unsupported_by_kiln | misc | `'{0}' attribute is not supported by kiln yet` ({0}: the class-A rows of `attr_triage` left after WP-1745: `address_space`, the `opencl_*` address spaces and the `swift_*` parameter ABIs `swift_context`, `swift_async_context`, `swift_error_result`, `swift_indirect_result`) | `crates/kiln_sema/src/decl.rs::attribute_node` | 0 | WP-1745 |

## Excluded (clang-matching wording)

Not counted in the totals: clang reports the same limit with the same
words, so these are not kiln gaps.

| gap | family | area | text | sites | reach | row |
|---|---|---|---|---|---|---|
| GAP-168 | codegen fallback | excluded | `cannot compile this tail call skipping over cleanups yet` | `crates/kiln_codegen/src/expr.rs::finish_musttail` | 0 | excluded: clang's own wording and limit (`[[clang::musttail]]` over cleanups) |
| GAP-169 | Oven trap | excluded | `constexpr bit cast involving type '{n}' is not yet supported` | `crates/kiln_oven/src/compile/bitcast.rs::bit_cast_unsupported` ×2 | 0 | excluded: clang's note `note_constexpr_bit_cast_unsupported_type`, same cases |
| GAP-170 | Oven trap | excluded | `non-trivial destruction of lifetime-extended temporary with type '{t}' used in the result of a constant expression is not yet supported` | `crates/kiln_oven/src/value.rs::check_let_destruction` | 0 | excluded: clang's note `note_constexpr_unsupported_temporary_nontrivial_dtor`, same cases |
| GAP-171 | spec | excluded | `__builtin_va_arg_pack()` / `__builtin_va_arg_pack_len()` are undeclared (C++) or an unknown builtin (C) | `spec/sema-templates.md::§9.5 __builtin_* functions used by the M3 headers` | 0 | excluded: clang 22 implements neither, and glibc uses them only for a `__GNUC_PREREQ (4,3)` kiln's clang version macros do not meet (WP-1550) |
| GAP-172 | spec | excluded | `-fcomplex-arithmetic=promoted` is "unsupported argument", `-ffp-model=fast` (which selects it) "'…' is not supported by kiln" | `spec/codegen.md::§7n. Landed (WP-211): GNU _Complex (complex.rs)` | 0 | excluded: a driver flag kiln rejects by name, as every `drv_unsupported_flag` use is (outside this page's language corners); promoted complex division is a proposed follow-up row (WP-1782) |
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

