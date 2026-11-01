# Lambdas — `kiln_parse`, `kiln_sema`, `kiln_oven`, `kiln_codegen`

Status: **landed** (WP-075). Companions: `ast.md` §5.3 (`LambdaExpr`,
`LambdaData`), `sema-class.md` (closure classes are ordinary
`CXXRecordDecl`s), `sema-templates.md` (generic lambdas are member
templates; substitution), `sema-overload.md` (surrogate calls,
conversion-function templates), `oven.md`, `codegen.md`, `abi.md` §8.9
(mangling and linkage). Standard references are to the C++26 draft;
"clang" means clang 19 with `-std=c++26` on the container target, the
behavioural oracle. The AST shape follows clang's `-ast-dump=json` so
that `expect: ast` goldens compare without special cases.

## 1. Scope

| | Landed (WP-075) | Later |
|---|---|---|
| syntax | introducer, captures, `<...>` template-parameter-list with requires-clause, parameter-declaration-clause, `mutable` / `constexpr` / `static`, attributes, *noexcept-specifier*, trailing return type, trailing requires-clause | braced and parenthesized init-captures (`[x{1}]`, `[x(1)]`) |
| closure type | §3: call operator, conversion to pointer to function, static invoker, deleted special members, linkage, mangling numbers | |
| captures | §4: by copy, by reference, init-captures (`= expr`), `this`, `*this`, implicit capture through `=` / `&`, nested lambdas | capture packs (`xs...`, `...xs = e`) |
| generic | §5: `auto` parameters, template-parameter-lists, constraints, conversion-function templates | |
| constant evaluation | §7: calls, captures, conversions, `this`/`*this`; `consteval` and escalated lambdas (§3.2, sema.md §5.10) | escalation of a generic lambda's conversion per specialization |
| templates | §9: lambdas in templates and generic lambdas in templates | capture packs, lambdas as template arguments |
| codegen | §8: IR-exact against clang for the corpus in `tests/codegen/lambda_*.cpp` | |

Coroutine lambdas are out of scope (coroutines milestone).

## 2. Parser to Sema protocol

`kiln_parse/src/lambda.rs` parses a *lambda-expression* and drives Sema
through four entry points (`kiln_sema/src/lambda/mod.rs`), in order:

1. `act_on_lambda_start(&LambdaIntroducer) -> LambdaId` at the `]`:
   creates the closure class (an implicit unnamed `class` in the innermost
   enclosing context, [expr.prim.lambda.closure]/1-2), begins its
   definition, pushes the `LambdaData` side record and a `LambdaScope`,
   and checks the explicit captures (§4.1). The scope records
   `templ.heads.len()` and `scopes.len()` so that a later head is known to
   be the lambda's own.
2. `act_on_lambda_template_params_start(l_angle)` if `<` follows: opens a
   template head one level deeper than the enclosing ones and a
   template-parameter scope. The parser then parses the parameter list
   and requires-clause with the ordinary template actions.
3. `act_on_lambda_declarator(id, Declarator, &LambdaSpecifiers)` after the
   *lambda-declarator* (a member declarator with one function chunk; an
   omitted parameter clause gives `auto () const`, clang
   `getDummyLambdaType`): declares the call operator (§3.1), and with a
   deeper head finishes the invented head and makes it a function template
   (§5). It numbers the closure (§3.5) and opens the body: it saves the
   enclosing function's state (context, pending odr-uses, cleanup state,
   unevaluated and immediate depths, lifetime extension, construction
   entity, mangling context), enters a function scope with the parameters,
   and declares the init-capture variables.
4. `act_on_lambda_end(id, body, rbrace) -> ExprId` at the `}`: closes the
   body (deduced return type, falloff warnings), restores the enclosing
   state, adds the capture fields (§4.3) and the conversion function and
   invoker (§3.2), completes the class, and builds the `LambdaExpr`
   (a prvalue of the closure type; its children are the member
   initializers, clang `LambdaExpr::capture_inits`).
   `act_on_lambda_error(id)` instead completes the class as invalid.
   Both pop the heads and scopes the lambda pushed.

Delayed default arguments of a call operator in a class definition use
`act_on_start_delayed_default_arg(p)` / `act_on_end_delayed_default_arg()`
to set the numbering context of lambdas inside them.

## 3. The closure type

`RecordData.lambda = Some(LambdaId)`; `DefinitionBits::LAMBDA` marks it,
`LAMBDA_CAPTURES` a lambda with a capture-default or captures. The class
is not an aggregate and has a trivial, referenced destructor (clang
marks it so).

### 3.1 The function call operator

A public inline `CXXMethodDecl` `operator()` ([expr.prim.lambda.closure]/4-5):
`const` unless `mutable` or `static`; `static` makes it a static member
function; `constexpr` (implicitly, [expr.prim.lambda.closure]/5) unless
`consteval` was written; the return type is `auto` (deduced from the body)
unless a trailing return type was written. Its parameters may have
default arguments; the [dcl.fct.default]/4 check is the ordinary one.

### 3.2 Conversion to pointer to function and the static invoker

A lambda without a capture-default has ([expr.prim.lambda.closure]/8):

- a public, implicit, constexpr, `noexcept` conversion function to
  pointer to function with the call operator's parameters and return
  type (and its *noexcept-specifier*);
- unless the lambda is `static`, an implicit static member function
  `__invoke` with the call operator's signature (clang's static invoker),
  constexpr only if `constexpr` was written.

Both are immediate functions if the call operator is
([expr.prim.lambda.closure]/10-11): `consteval` when it is written, and
escalated (`FnSpec::escalated`) when the call operator escalated
(WP-137). clang 19 rejects every use of a consteval lambda's conversion;
kiln follows the standard (`expr.prim.lambda/consteval-conversion.cpp`
is `xfail(clang)`).

Neither has a body. `Ast::closure_thunk(d)` classifies them as
`ClosureThunk::Conversion { target }` (the invoker, or the call operator
of a static lambda) and `ClosureThunk::Invoker { call_op }`; the Oven
(§7) and codegen (§8) synthesize their behaviour. For a generic lambda
both are function templates sharing the call operator's template
parameter list (§5.2). The conversion's definition is marked when it is
odr-used (`define_lambda_conversion`).

### 3.3 Special members

[expr.prim.lambda.closure]/13-14: a closure type without captures has a
defaulted default constructor and copy assignment operator; with any
capture-default or capture, both are deleted. The copy and move
constructors are defaulted. `closure_default_constructible` drives the
`__is_constructible` and value-initialization answers.

### 3.4 Names in diagnostics and dumps

A closure prints as `(lambda at <file>:<line>:<col>)` (the `[`), clang's
form, through `AstNames`. In qualified names of local entities a closure
prints as `(anonymous class)` (clang `printQualifiedName`, also used for
IR value names).

### 3.5 Linkage and mangling numbers

[basic.def.odr]/14 and Itanium §5.1.8 (clang `handleLambdaNumbering`):
a lambda in the body of an inline function, in a default argument in a
class definition, in a default member initializer, or in the
initializer of an inline variable (or variable template) gets a
mangling number per (numbering context, call-operator parameter types,
generic) and, for the last three, a context declaration that prefixes its
name. `LambdaData.mangling` records both. Any other closure has no
number and no linkage: its members are internal and it mangles as an
anonymous class (`$_N`). See abi.md §8.9.

## 4. Captures

### 4.1 Explicit captures ([expr.prim.lambda.capture]/1-8)

`act_on_explicit_captures` checks each capture as written, in order,
with clang's diagnostics: duplicates (`'x' can appear only once`),
`&x` with default `&`, `x` without `&` with default `=` (C++20 allows
`=, this`), non-variables, class members, entities without automatic
storage duration, `this` outside a non-static member function, `this`
in a static lambda. A simple capture names a variable found by ordinary
lookup, unless `LambdaCaptureSpec.var` already names it (a lambda rebuilt
by template instantiation, §9). An init-capture declares a variable of
the closure's call operator, `auto x = init` or `auto &x = init`, whose
type is deduced at the introducer and which the body declares.

### 4.2 Implicit capture ([expr.prim.lambda.capture]/9-12)

A name in the body that odr-uses (or, for a reference or a variable
usable in constant expressions, may odr-use) a local entity of an
enclosing function is captured through every lambda between the use and
the entity (clang `tryCaptureVariable`). `capture_at_decl_ref` runs at
the name; for a variable that might be constant (`might_be_constant`)
the decision is deferred to the end of the full-expression, where
`capture_odr_use` captures only a real odr-use ([basic.def.odr]/5,
lvalue-to-rvalue on a constant). Without a capture-default:
`variable 'x' cannot be implicitly captured in a lambda with no
capture-default specified` with clang's notes (`lambda expression begins
here`, `capture 'x' by value` / `by reference`, `default capture by
value` / `by reference`). A use of an enclosing function's local outside
any lambda is `reference to local variable 'x' declared in enclosing
function`.

`this` is captured implicitly by a use of `this` (explicit or through a
member access) under either capture-default; under `=` clang 19 warns
`implicit capture of 'this' with a capture default of '=' is
deprecated` with its note. `capture_this` walks the enclosing lambdas as
for variables.

### 4.3 The closure's members

Each capture by copy, and each init-capture, gets an unnamed non-static
data member in capture order (explicit captures as written, then implicit
ones in order of first odr-use); by-reference captures get a reference
member (clang's layout, matching `-fdump-record-layouts`). `this` is a
pointer member; `*this` a member of the class type, whose type keeps its
sugar. A captured array is a member array, initialized element-wise.
The member initializers (the `LambdaExpr`'s children) copy-initialize from the entities named, as
[expr.prim.lambda.capture]/15 requires, with the usual temporaries and
cleanups.

### 4.4 Names of captured entities in the body

A `DeclRefExpr` to a captured entity has `ExprFlags::REFERS_TO_ENCLOSING`
and the type of the member's access: `const` in a non-`mutable` lambda for
a by-copy capture ([expr.prim.lambda.capture]/11, so assignment reports
`cannot assign to a variable captured by copy in a non-mutable lambda`).
Such a name is not a candidate for implicit move or NRVO
([class.copy.elision]/3). `this` in the body is the captured `this` or
the address of the `*this` copy.

## 5. Generic lambdas

### 5.1 The call operator template

[expr.prim.lambda.general]/6: an `auto` parameter (an invented type
template parameter, constrained by a type-constraint if written) or a
written template-parameter-list makes the call operator a member
function template of the closure class. The head is the lambda's own (one
level deeper than the enclosing heads); the invented parameters follow
the written ones. The template is public. Its requires-clause and the
call operator's trailing requires-clause are the template's associated
constraints, checked at overload resolution as for any function template.

### 5.2 Conversion function templates

For a generic lambda without a capture-default the conversion function and
the invoker are function templates with the call operator's parameter
list. Deduction of a conversion function template from a target
`R(*)(P...)` (`lambda_conversion_p`) uses P = `auto(*)(T...)` with the
return type taken from the target; `specialize_lambda_conversion` then
specializes the call operator for the deduced arguments to deduce its
return type and types the conversion and invoker specializations from it.
A return type that does not match the target makes the conversion
sequence fail, as in clang. `Ast::call_op_lambda` and `closure_thunk`
accept specializations; a conversion specialization's target is the
invoker (or call operator) specialization for the same arguments (clang
`SpecializeCorrespondingLambdaCallOperatorAndInvoker`).

### 5.3 Return types

`decltype(auto)` mangles as `Dc`. A return type deduced from a dependent
expression is deduced at instantiation; `void` deduction in a
dependent body is deferred to the specialization.

## 6. Diagnostics

All lambda diagnostics use clang's wording, location and notes (the ids
added in `kiln_diag/src/ids.rs`: `parse_*lambda*`, `parse_expected_*capture*`,
`sema_lambda_impcap`, `sema_capture_*`, `sema_this_capture`,
`sema_deprecated_this_capture`, `sema_assign_captured_by_copy`,
`sema_reference_to_local_in_enclosing`, `sema_falloff_nonvoid_lambda*`
and their notes). `-Wdeprecated-this-capture` and `-Wreturn-type`
control the warnings.

## 7. Evaluation (the Oven)

A closure object is an ordinary class object in the Oven's memory; the
`LambdaExpr` initializes its members in order from its children.

- **Calls** go through the ordinary member call path to the call
  operator (or its specialization). The prologue (`lambda_prologue`)
  sets the body's `this` to the captured `this`, or to the address of
  the captured `*this` copy.
- **Captured names** (`captured_var_place`): a name with
  `REFERS_TO_ENCLOSING` in a call operator that captures it is the closure
  member's place, or the referent of a reference member. A by-reference
  capture outliving its referent is a dangling access, diagnosed as for
  any reference (`constexpr-capture-lifetime.cpp`).
- **Thunks**: the conversion function returns the address of its target;
  the static invoker runs the call operator's body with no object
  (its body never uses `this`: the lambda has no captures). Notes print
  such a call as clang's `describeCall` does.

Nothing about closures needs heap allocation in the Oven beyond the
object itself.

## 8. Code generation

`kiln_codegen/src/lambda.rs`. The closure is an ordinary class type
(named as clang names it); capture members are fields. The call
operator's prologue records the closure and the captured `this`. The
conversion function returns the invoker's address (or the call
operator's for a static lambda); `__invoke` calls the call operator on an
unused closure temporary, forwarding each parameter (clang
`EmitLambdaDelegatingInvokeBody`, `EmitForwardingCallToLambda`). Capture members with non-trivial
destructors and captured arrays get EH cleanups as in clang
`-fexceptions`. Linkage and names follow §3.5 and abi.md §8.9; IR is
compared exactly against clang (`tests/codegen/lambda_*.cpp`).

## 9. Lambdas in templates

[temp.inst]/2, [expr.prim.lambda.closure]/2 (clang
`TreeTransform::TransformLambdaExpr`, `kiln_sema/src/templ/subst/lambda.rs`):
each instantiation of the enclosing template gets a closure type of its
own. The substituter replays the lambda through the parser's actions:

1. The explicit captures, with init-capture initializers substituted
   and simple captures resolved to the instantiated variable
   (`LambdaCaptureSpec.var`: the name may not be in scope, e.g. a nested
   lambda instantiated with its enclosing generic lambda's call operator).
   The implicit captures are found again in the rebuilt body.
2. For a generic lambda, `open_substituted_head` substitutes the template
   parameter list with the retained depth (the lambda's own parameters
   one level shallower per substituted level), as for a member template.
3. The call operator's declarator, with substituted parameter types,
   *noexcept-specifier* and trailing return type, and the specifiers.
4. The constraints are not substituted: the new call operator (or its
   template) is entered in `ConceptState::member_constraints` with the
   pattern and the current arguments, so they are checked with this
   instantiation's levels outside the operator's own, as for a member of
   a class template.
5. Default arguments, then the body. In a generic lambda the body is
   rebuilt as a pattern (its own parameters stay dependent); a
   function parameter pack of the pattern stays a pack
   (`param_infos`, `map_params`, `size_of_pack` handle a retained pack).

Uses of `this` in a rebuilt body capture it again (`note_this_use`);
`note_shared_this` keeps the enclosing lambda's `this` capture in step.

## 10. Tests

- `tests/conformance/expr.prim.lambda/`: 46 tests (45 pass, 1 xfail:
  `template-argument.cpp`) covering syntax errors, captures and their
  diagnostics, closure members, generic lambdas, conversions, constant
  evaluation, run-time behaviour, mangling and lambdas in templates,
  plus six `expect: ast` goldens (`ast-*.cpp`) against clang's JSON.
- `tests/codegen/lambda_basic.cpp`, `lambda_captures.cpp`,
  `lambda_captures_class.cpp`, `lambda_names.cpp`: IR-exact.
- `xtask clang-ast` prints `LambdaExpr` and its implicit members.

## 11. Deviations, open questions and gaps

1. **Instantiation notes on warnings.** spec/diag.md §7.4 drops a
   diagnostic with an instantiation context that duplicates one
   (same id and span) reported without; clang re-emits a warning at
   each instantiation with an `in instantiation of ...` note. This shows
   with lambdas (e.g. a deprecated `this` capture in a template) but is
   not lambda-specific. Recommendation: keep the rule; revisit in
   diag.md if strict-diags tests need clang's form.
2. **Capture of `might_be_constant` variables** is decided at the end of
   the full-expression; clang's finer `NonOdrUseReason` tracking is not
   modelled. No known divergence in the tests.
3. **Gaps** (each reported as `not yet implemented` / `not supported`):
   braced and parenthesized init-captures (parser); capture packs
   ([expr.prim.lambda.capture]/17), also in templates; `consteval`
   lambdas; a lambda-expression as a template argument and calling an
   `auto` template parameter whose default is a closure
   (`template-argument.cpp`); `x.template operator()<T>()` (WP-121);
   the `S2_` substitution in some closure manglings inside templates
   (WP-117).

## Landed (WP-075)

Parser (`kiln_parse/src/lambda.rs`), Sema (`kiln_sema/src/lambda/`:
`mod.rs` closure and call operator, `capture.rs` captures, `generic.rs`
conversion templates; `templ/subst/lambda.rs` instantiation), AST
(`kiln_ast/src/lambda.rs`: `LambdaData`, `LambdaCapture`,
`LambdaMangling`, `ClosureThunk`; `Ast::closure_lambda`,
`call_op_lambda`, `closure_invoker`, `closure_thunk`), Oven
(`kiln_oven/src/compile/lambda.rs`, closure thunks in `compile.rs`),
codegen (`kiln_codegen/src/lambda.rs`), mangling and linkage
(`kiln_abi/src/mangle.rs`), dump and `xtask clang-ast` support.
