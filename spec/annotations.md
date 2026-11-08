# spec: annotations (P3394R4, `[[=e]]`)

Standard: [dcl.attr.grammar], [dcl.attr.annotation],
[meta.reflection.result] (`reflect_constant`), [temp.param]/8 (structural
types and template parameter objects), [basic.scope.scope]/2 (host scope),
[temp.variadic]. Draft: the C++26 working draft as of 2026-11
(https://eel.is/c++draft/). Status: syntax and semantics implemented
(WP-481); the queries `annotations_of`, `annotations_of_with_type`,
`is_annotation` and `constant_of`/`extract` on an annotation are M6-09
(docs/plans/m6-reflection.md §4), so nothing in a program observes an
annotation yet. This file is the contract; spec/parse.md §6.2, spec/ast.md
§7 and spec/ast-dump.md ("annotation" row) point here.

## 1. Model

An annotation is a constant attached to one declaration. It is not an
attribute: it has no effect on semantics, overload resolution, layout,
codegen or mangling. Its *underlying constant* is the value of
`std::meta::reflect_constant(e)` ([dcl.attr.annotation]/2), so
an annotation is checked and frozen exactly as a template argument for a
constant template parameter of type `T` would be, where `T` is the
operand's type decayed and cv-unqualified (`reflect_constant` takes `T
expr` by value; [meta.reflection.result]/1 Mandates "`T` is a
cv-unqualified structural type that is not a reference type").

## 2. Syntax (kiln_parse `attr.rs`)

Grammar ([dcl.attr.grammar]/1):

```
attribute-specifier: [ [ attribute-using-prefix_opt attribute-list ] ]
                     [ [ annotation-list ] ]
                     alignment-specifier
annotation-list:     annotation ..._opt
                     annotation-list , annotation ..._opt
annotation:          = constant-expression
```

- **Mixing (verify, brief item 1).** The brief asked for annotations
  "mixed with ordinary attributes in any order" (`[[nodiscard, =1]]`) and
  "not affected by" a using-prefix. The wording says otherwise: an
  attribute-specifier is *either* an attribute-list (optionally with a
  using-prefix) *or* an annotation-list, so `[[nodiscard, =1]]`,
  `[[=1, nodiscard]]` and `[[using gnu: =1]]` are ill-formed
  ([dcl.attr.grammar]/1). kiln reports `parse_annotation_mixed` ("an
  attribute-specifier cannot mix annotations and attributes") once per
  specifier and drops its annotations. Separate specifiers mix freely:
  `[[nodiscard]] [[=1]]`. Test: `dcl.attr.annotation/mixed-with-attributes.cpp`.
- **Operand.** A *constant-expression*, which is a conditional-expression
  ([expr.const]/1), not an assignment-expression: the comma separates
  annotations, so `[[=a, =b]]` is two annotations, `[[=(a, b)]]` one, and
  `[[=a, b]]` is an annotation followed by the attribute `b` (the mixing
  error).
- **Packs (verify, brief item 4).** The brief asked whether
  `[[=args...]]` is forbidden. It is not: "an annotation followed by an
  ellipsis is a pack expansion" ([dcl.attr.grammar]/6, [temp.variadic]).
  `= e...` expands to one annotation per element, in order; an operand
  that names an unexpanded pack without `...` is the ordinary
  `unexpanded parameter pack` error (`act_on_annotation`). Tests:
  `templates.cpp` (`packed`), `invalid-operand.cpp`.
- **Language mode.** Accepted in every mode; before C++26 the first
  annotation of a specifier warns `parse_ext_annotations` (ExtWarn,
  `-Wc++26-extensions`). There is no feature-test macro of its own.
  Test: `cxx23-extension.cpp`.
- The parser produces, per annotation, an `AttrKind::ANNOTATION` node
  (`a` = the operand `ExprId`; with `...` the operand is a `PackExpansionExpr`) through
  `Sema::act_on_annotation`; it travels in the attribute vector of the
  position like any attribute until sema takes it (§3).

## 3. Appertainment (kiln_sema `annot.rs`)

[dcl.attr.annotation]/1: an annotation may appertain to a base-specifier
or to a declaration D of "a type, type alias, variable, function, function
parameter of non-void type, namespace, enumerator, or non-static data
member", unless, X being the function declaration for a parameter of its
declarator and D otherwise, (1.1) "the host scope of X differs from its
target scope" or (1.2) X is a non-defining friend declaration.

- Where a declaration is created, `take_annotations(d, attrs)` removes the
  annotation nodes from its attribute vector, checks `d`
  (`annotation_subject_error`), evaluates each (§4) and appends the list
  to the table. Allowed: namespaces, typedefs and alias declarations
  (templates included), classes and enumerations (templates and
  specializations included), enumerators, non-static data members (bit
  fields included), functions (all member kinds, templates), variables
  (static members, block-scope, variable templates and specializations),
  and function parameters of non-void type.
- **Rejected as "an annotation cannot appertain to X"**
  (`sema_annotation_not_allowed`): a block-scope function declaration or
  `extern` variable (host scope is the block, target scope the namespace:
  [basic.scope.scope]/2 "the host scope of a declaration is the inhabited
  scope if that scope is a block scope and the target scope otherwise"),
  a friend declaration that is not a definition (/1.2; a defining friend
  is allowed: its inhabited scope is the class, so host = target), a
  `void` parameter list, a using-directive, a using-declaration, a
  namespace alias, a concept, a deduction guide, a template parameter, a
  structured binding written after its identifier, an asm-declaration, an
  attribute-declaration, a declaration with no declarator, a label, and a
  lambda's call operator (none of these declares an entity of the listed
  kinds). `static_assert` takes no attribute-specifier-seq at all.
- **Non-declarations** (`sema_annotation_non_declaration`, "an annotation
  can appertain only to a declaration"): a statement, a type
  (type-specifier-seq or declarator-chunk position), anything else that
  is not a declaration. Test: `non-declaration.cpp`.
- **Base-specifiers** (/1): the annotations of a base-specifier (its
  leading attribute-specifier-seq, before any `virtual` or access
  keyword, [class.derived.general]/1) are kept in `AnnotArena::of_base`,
  keyed by the derived class's first declaration and the base's index in
  its definition's base list; `annotations_of` of a `bases_of` reflection
  reads them, and a class template specialization substitutes them with
  its bases (each element of a base pack expansion gets its own copy).
  The pattern of a base pack expansion is the whole base-specifier,
  attribute-specifier-seq included ([temp.variadic]/5.4), so an operand
  naming the pack (`[[=sizeof(Bs)]] Bs...`) is substituted per element,
  in source order with the operands substituted once
  (`Substituter::class_bases`). The parser keeps such an operand
  (`Sema::set_base_annotation_packs`) until the `...` is known; on a
  base-specifier without one it is the unexpanded-pack error
  (`Sema::check_base_annotation_packs`). GCC 16.1 crashes on both forms
  (docs/status/deviations.md).
  Tests: `meta.reflection.annotation/base-specifier*.cpp` (WP-830, WP-867).
- A structured binding declaration's leading annotations appertain to the
  declaration, i.e. the unnamed variable `e` ([dcl.struct.bind]/1): kiln
  accepts them there (open question §9).

## 4. Evaluation and storage

- `annotation_value(e)`: `T` = the operand's type, arrays and functions
  decayed, cv-qualifiers dropped; `T` must be complete, not `void`, and
  structural ([temp.param]/8; `sema_annotation_not_structural` with the
  same notes as a non-structural NTTP type). The value is a `const T`
  copy-initialized from `e`, checked as a converted constant expression
  and evaluated with `EvalKind::TemplateArg`; a class value is frozen as
  its template parameter object ([meta.reflection.result]/2.1, /4). A
  failure is `sema_expr_not_cce` ("annotation is not a constant
  expression") with the Oven's notes. A reflection (`std::meta::info`)
  is structural, so `[[=^^int]]` is an annotation (verify, brief item 2:
  P3394 allows `info` annotations; [meta.reflection.result]/1 asks only
  for a structural type, and `info` is a scalar).
- A value-dependent or type-dependent operand is stored unevaluated
  (`value: None`, `written_type` the operand's type) and substituted with
  its entity (§5).
- Table: `kiln_ast::annot::AnnotArena` in `Ast::annots`, one 20-byte
  `Annotation { decl, value, loc, written_type, expr }` per annotation
  and a cold `DeclId → ListId` map from each declaration to the
  annotations written on it, in source order. No `AttrNode` survives
  sema. Its memory is the `annots` rows of the AST memory report
  (`HeapSize`) and counts in the AST's live bytes.
- **Redeclarations.** Each declaration keeps its own list;
  `AnnotArena::of_entity(d)` returns the lists of the entity's
  declarations from first to latest, each in source order
  (P3394R4 Example 2: `[[=2, =3, =2]] void g(); void g [[=4, =2]] ();`
  gives g five annotations). A class head's and the definition's lists
  are joined (`attach` appends). Parameters: an annotation on a parameter
  belongs to that declaration's parameter; merging parameters across a
  function's redeclarations (Note 1 / Example 1: the parameter has the
  annotations of every declaration, the variable only the definition's)
  is the query's job (M6-09, §9).
- **No de-duplication (verify, brief storage item).** The brief asked for
  an annotation "equal to one already on the entity" not to be added
  twice. The wording is the opposite: "Each annotation or instantiation
  thereof produces a unique annotation" ([dcl.attr.annotation]/3,
  Examples 2 and 3). kiln never merges equal values. Test:
  `duplicates.cpp`, `redeclarations.cpp`.
- The AST dump prints one `CXX26AnnotationAttr` line per annotation
  (spec/ast-dump.md).

## 5. Templates

- A templated entity's annotations are instantiated with it
  (`Substituter::copy_annotations`, called from the attribute copy of
  every instantiated declaration and for enumerators): a frozen value is
  shared by a new annotation on the instantiation; a dependent one is
  substituted, expanded and evaluated. Each instantiation's annotations
  are new annotations (/3, Example 3). A class specialization's members,
  its member templates' patterns, a variable template specialization,
  partial and explicit specializations (their own annotations only) and
  function parameters (paired with the pattern's) all follow this.
  A function parameter pack (`[[=1]] Ts... xs`) gives each expanded
  parameter its own copy of the pattern's annotations, substituted per
  element (`annotated_param_pairs`; `meta.reflection.annotation/parameter-pack.cpp`,
  WP-830). With more than one function parameter pack each pack pairs
  with the parameters of its own expansion, its length probed under the
  current arguments as the body's parameter map does
  (`Substituter::param_pack_len`); a pack that stays a pack (a member
  template's own, substituted with its class) pairs one to one
  (`parameter-packs-several.cpp`, WP-866).
- **Function template specializations are lazy.** "Substituting into an
  annotation is not in the immediate context" ([dcl.attr.annotation]/4):
  a failure is a hard error, never a deduction failure, and Example 4
  (`template<class T> [[=T::type()]] void f(T); void f(int);` — `f(0)` is
  fine, `f('0')` is an error) requires that a specialization which loses
  overload resolution never substitutes. So a function template
  specialization whose own or whose parameters' annotations are dependent
  records `first decl → pattern` in `Sema::uninstantiated_annots`, and
  `require_annotations(f)` substitutes them when `f` is referenced
  (`mark_function_referenced`, as for an exception specification) in an
  instantiation frame `FrameKind::Annotation` ("in instantiation of
  annotation for 'f<char>' requested here"). A query must call
  `require_annotations` first (§9). Tests: `templates.cpp`,
  `invalid-operand.cpp`.

## 6. Diagnostics

| id | when |
|---|---|
| `parse_annotation_mixed` | annotation and attribute, or a using-prefix, in one specifier |
| `parse_ext_annotations` | before C++26 |
| `sema_annotation_non_declaration` | on a statement, type or other non-declaration |
| `sema_annotation_not_allowed` | on a declaration /1 excludes (§3) |
| `sema_annotation_not_structural` | `T` not structural |
| `sema_annotation_invalid_value` | the value is, or points into, a string literal, a temporary, a `typeid` result or a label: not a valid constant template argument ([temp.arg.nontype]/6; WP-830) |
| `sema_expr_not_cce` ("annotation") | the operand is not a constant expression |
| `note_template_annotation_here` | instantiation note for a lazy substitution |

## 7. Interface

- kiln_ast: `annot::{Annotation, AnnotArena}`, `ids::AnnId`,
  `Ast::annots`; `AttrKind::ANNOTATION` (transient).
- kiln_sema: `act_on_annotation`, `reject_annotations_in` (parser hooks);
  crate-internal `take_annotations`, `reject_annotations`,
  `annotation_value`, `require_annotations`, `copy_annotations`.
- Reflection: `Reflectee::Annotation(AnnId)` (WP-472's enum) is to name
  one annotation; its `constant_of` is `value`.

## 8. Cost

One constant evaluation per annotation, no Oven steps for literal operands, and one frozen value
(a class value's template parameter object) per annotation;
`stress/annotations-many.cpp` (10,000 declarations, 20,000 annotations)
pins `evaluations`, `steps`, `calls`, `allocs`, `peak_bytes`,
`frozen_bytes` and `static_bytes`, and `mem<=256` MiB. The brief's
`refl_bytes` counter does not exist (spec/harness.md lists the counters);
`frozen_bytes` and `static_bytes` stand in for it.

## 9. Open questions and follow-ups

1. *Resolved (WP-830, WP-867).* Base-specifier annotations (§3),
   including an operand naming the base pack (`[[=sizeof(Bs)]] Bs...`),
   substituted per element.
2. *Resolved (WP-478).* Parameter annotations across redeclarations
   (Note 1, Example 1): `annotations_of` of a parameter lists the
   parameter's annotations in every declaration of the function
   (spec/reflection-members.md §5).
3. *Resolved (WP-830, WP-866).* A pattern parameter pack gives each
   expanded parameter the pattern's annotations (§5), with any number of
   function parameter packs.
4. *Resolved (WP-830).* A value that is, or points into, a string literal
   (`[[="abc"]]`) is `sema_annotation_invalid_value`
   ([temp.arg.nontype]/6.2; `dcl.attr.annotation/string-literal-neg.cpp`,
   no oracle: GCC 16.1 accepts it). `X<"abc">` was already rejected.
5. Annotations on a parameter of a function-pointer or function-type
   declarator chunk, and on a lambda's parameters, are accepted and kept
   on the parameter; whether a lambda parameter is a "function parameter"
   for /1 is for M6-09 to settle with the query.
6. *Resolved (WP-478).* Pending lazy annotations are never flushed: a
   specialization that is never referenced shows none, so the queries
   call `require_annotations` first (spec/reflection-members.md §5).
7. *Resolved (WP-830).* After a label's colon the parser takes only GNU
   attributes for the label; a C++11 attribute-specifier-seq there
   belongs to the sub-statement (as clang parses it), so `lbl: [[=4]] int
   x;` annotates `x` and `lbl: [[=4]];` is `sema_annotation_non_declaration`
   (`dcl.attr.annotation/label-colon*.cpp`; GCC 16.1 only warns, so the
   negative test has no oracle).
8. `[[=1]] struct S {};` is the existing "misplaced attributes" error (a
   leading attribute-specifier-seq on a declaration with no declarator is
   ill-formed, [dcl.pre]/5) and namespace-scope `[[=1]] asm("")` the
   parser's "an attribute list cannot appear here"; the block-scope forms
   are diagnosed as in §3.
9. A structured binding declaration's annotations appertain to the
   declaration (§3); confirm against the queries once `variable_of` and
   structured-binding reflection land.
