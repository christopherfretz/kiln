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
| syntax | introducer, captures (init-captures with `= e`, `= {...}`, `(e)` and `{e}`, WP-518), `<...>` template-parameter-list with requires-clause, parameter-declaration-clause, `mutable` / `constexpr` / `static`, attributes, *noexcept-specifier*, trailing return type, trailing requires-clause | |
| closure type | §3: call operator, conversion to pointer to function, static invoker, deleted special members, linkage, mangling numbers | |
| captures | §4: by copy, by reference, init-captures (every initializer form), `this`, `*this`, implicit capture through `=` / `&`, nested lambdas, capture packs (`xs...`, `...xs = e`, `...xs(e)`, `&...xs{e}`) | |
| generic | §5: `auto` parameters, template-parameter-lists, constraints, conversion-function templates | |
| constant evaluation | §7: calls, captures, conversions, `this`/`*this`; `consteval` and escalated lambdas (§3.2, sema.md §5.10) | |
| templates | §9: lambdas in templates and generic lambdas in templates; simple-capture packs (`[ts...]`) of a function parameter pack (WP-282); init-capture packs; lambdas as template arguments and closure-typed `auto` NTTP defaults | |
| codegen | §8: IR-exact against clang for the corpus in `tests/codegen/lambda_*.cpp` | |

A lambda whose body uses `co_await`, `co_yield` or `co_return` is a
coroutine like any function (spec/sema.md §7.13, WP-351): its call
operator stops being implicitly constexpr, and the closure type is the
implicit object parameter in the `coroutine_traits` arguments but the
closure object is passed neither to the promise constructor nor to a
promise `operator new`.

## 2. Parser to Sema protocol

`kiln_parse/src/lambda.rs` parses a *lambda-expression* and drives Sema
through four entry points (`kiln_sema/src/lambda/mod.rs`), in order:

1. `act_on_lambda_start(&LambdaIntroducer) -> LambdaId` at the `]`:
   creates the closure class (an implicit unnamed `class` in the innermost
   enclosing context, [expr.prim.lambda.closure]/1-2), begins its
   definition, pushes the `LambdaData` side record and a `LambdaScope`,
   and checks the explicit captures (§4.1). The scope records
   `templ.heads.len()` and `scopes.len()` so that a later head is known to
   be the lambda's own. The capture check creates and initializes each
   init-capture variable (§4.1); `create_init_capture_vars` then
   publishes them in `LambdaScope::init_cap_vars`: P2036 (a
   DR, so in every mode) puts them in scope from the introducer on
   ([expr.prim.lambda.capture]/6), so unqualified lookup in the
   *lambda-declarator* (parameters' default arguments, `noexcept`,
   trailing return type, requires-clause) finds them
   (`lambda_init_captures_named`), after the lambda's template and
   function parameters and before the enclosing scopes. They stay in the
   enclosing context until the call operator adopts them when its body
   opens (`declare_init_captures`). Several init-captures named `_` are
   all found, in order ([basic.scope.scope]/5, P2169). As in clang 19 and
   22, `decltype((x))` of an init-capture `x` in the declarator does not
   add the call operator's `const`. A lambda substituted in a template
   instantiation keeps its pattern's requires-clause; when its constraints
   are checked, the pattern's init-capture variables map to the
   substituted lambda's (`Substituter::seed_init_captures`, from
   `sat_atom_uncached`; clang `LambdaScopeForCallOperatorInstantiationRAII`),
   and in a generic lambda a requirement that is only
   instantiation-dependent once every level is substituted (it names such
   a variable, whose context stays a template) is checked then
   (`requires_expr`'s `dep_expr`;
   tests/conformance/expr.prim.lambda.capture/init-capture-in-requires-clause*.cpp;
   WP-1497). An init-capture pack of the pattern maps to the run of
   same-named expanded captures (`DeclOrPack::Pack`), and its element type
   while the lambda is a template is the dependent `auto`, so a call
   through it stays dependent
   (`init-capture-pack-requires-clause.cpp`).
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

Before C++23 the parser accepts two C++23 lambda forms as extensions, as
clang does (WP-769, P2173R1 and P1102R2):
- an *attribute-specifier-seq* (`[[...]]` or `alignas`) directly after
  the *lambda-introducer* gets `parse_ext_decl_attrs_on_lambda`
  (`-Wc++23-lambda-attributes`, inside `-Wc++23-extensions`) at the
  attribute;
- a *lambda-declarator* without a parameter clause that starts with a
  specifier (`mutable`, `constexpr`, `consteval`, `static`), `->`,
  `__attribute__`, `[[`, `noexcept` or `requires` gets
  `parse_ext_lambda_missing_parens` (`-Wc++23-extensions`) with a `() `
  insertion fix-it at that token
  (`at_lambda_specifier_without_params`).
Both are warnings only; the lambda means the same as in C++23
(`tests/conformance/expr.prim.lambda/lambda-cxx23-extensions-cxx17.cpp`).

## 3. The closure type

`RecordData.lambda = Some(LambdaId)`; `DefinitionBits::LAMBDA` marks it,
`LAMBDA_CAPTURES` a lambda with a capture-default or captures. The class
is not an aggregate and has a trivial, referenced destructor (clang
marks it so).

### 3.1 The function call operator

A public inline `CXXMethodDecl` `operator()` ([expr.prim.lambda.closure]/4-5):
`const` unless `mutable` or `static`; `static` makes it a static member
function; `constexpr` (implicitly, [expr.prim.lambda.closure]/5) unless
`consteval` was written, from C++17 only (P0170R1): before, it is constexpr
only when `constexpr` was written (clang's `-Wc++17-extensions` warning
"'constexpr' on lambda expressions is a C++17 extension", at the keyword);
the return type is `auto` (deduced from the body) unless a trailing return
type was written.

**Before C++17** (WP-1666; C++11 [expr.const]/2, C++14 [expr.const]/2.6:
a lambda-expression is not a core constant expression). A lambda-expression
written directly in a constant-evaluated context (a `static_assert`, an
enumerator, a `case`, a template argument or a template parameter's
default, an array bound, a bit-field width, `alignas`, a
*noexcept-specifier*), outside a dependent context (so at instantiation in
a template), is an error at its `[` (`sema_lambda_in_constant_expression`,
clang `PopExpressionEvaluationContext`); the initializer of a constexpr
variable is not such a context. The Oven never evaluates a
lambda-expression (`non-literal type '(lambda at F:L:C)' cannot be used in
a constant expression`, clang `CheckLiteralType`), even discarded
(`([]{}, 0)`: clang 22 accepts it, docs/status/deviations.md), except the
whole initializer of the variable being initialized (through an elidable
copy) at namespace scope from C++14, and at C++11 one without captures:
those stay static initializations as clang emits them. The call operator
and the conversion being non-constexpr, no call of them is constant. Run
time is unchanged. Tests `expr.prim.lambda.closure/p5-*`. Its parameters may have
default arguments; the [dcl.fct.default]/4 check is the ordinary one.

**Before C++20** (WP-1671; C++11-17 [expr.prim.lambda]/2, removed by
P0315R4): a lambda-expression shall not appear in an unevaluated operand
(`sizeof`, `alignof`, `noexcept`, `typeid`, `decltype`, so also in a type
template argument, an alias, a typedef, a trailing return type or a
parameter written with `decltype`), nor, from C++17, in a
template-argument. Checked in `act_on_lambda_end` as clang's
`PopExpressionEvaluationContext` does, for a lambda written directly in
the context (not in a lambda body or a nested constant-expression inside
it) outside a dependent context (so at instantiation in a template), at its
`[`, in this order: an unevaluated operand
(`sema_lambda_unevaluated_operand`, "lambda expression in an unevaluated
operand"), before C++17 a constant-evaluated context (above), at C++17 a
template-argument (`sema_lambda_in_invalid_context`, "a lambda expression
cannot appear in this context"; the parser and the substituter mark a
non-type template argument with `Sema::enter_template_argument`, a default
template argument only when it is substituted for a use). The other
C++14/17 places of the rule (an alias-declaration, a typedef, a function
declaration outside its body and default arguments) are reached only
through those contexts, as in clang. GCC 16.1 also rejects such lambdas in
a template never instantiated and `[]() constexpr` at C++14; kiln follows
clang 22 (docs/status/deviations.md). Tests
`expr.prim.lambda/unevaluated-operand-*.cpp`,
`expr.prim.lambda/template-argument-cxx17.cpp`.

**Explicit object parameter** (P0847, WP-356). When the first parameter
is declared `this T` (`lambda_explicit_object`, clang
`CheckExplicitObjectLambda`), the call operator is an explicit object
member function (FnSpec `explicit_object`), never `const`. Errors, with
clang's wording and location: the parameter not first
(`sema_explicit_object_param_not_first`, "of the lambda"), a `static`
lambda (`sema_explicit_object_param_nonmember`, "static lambda"; the
operator is then an ordinary static one), `mutable`
(`sema_explicit_object_lambda_mutable`), a default argument (at its `=`,
`ParamInfo::default_loc`) and a pack. With a capture-default or a
capture, the parameter's type, references and cv removed, must be the
closure type or a class derived from it ([expr.prim.lambda.closure]/5;
`check_lambda_explicit_object_type`, clang
`DiagnoseInvalidExplicitObjectParameterInLambda`):

- a non-template call operator is checked once, at the parameter's
  name, when it is declared: `sema_explicit_object_lambda_capture_type`
  ("must be the same as, or derived from, the lambda");
- a specialization of a generic one is checked at each call that
  selects it (`check_lambda_explicit_object_call`, from
  `build_operator_call` and `explicit_object_call`), at the call's
  object: a type not derived from the closure gets the same message, a
  private or protected derivation "must derive publicly from the
  lambda" plus the access note (`check_lambda_object_base_access`,
  `BaseUse::LambdaObject` in `class/access.rs`); an ambiguous base
  (`find_base` finds several closure subobjects) gets
  `sema_explicit_object_lambda_ambiguous_base` ("lambda '(lambda at
  ...)' is inaccessible due to ambiguity:" with clang's path lines).
  The call is then invalid.

A lambda without captures takes any object type. The conversion to
pointer to function of such a lambda returns the call operator's address
(its type has the object parameter first, [expr.prim.lambda.closure]/8);
there is no static invoker.

### 3.2 Conversion to pointer to function and the static invoker

A lambda without a capture-default has ([expr.prim.lambda.closure]/8):

- a public, implicit, `noexcept` conversion function to pointer to
  function with the call operator's parameters and return type (and its
  *noexcept-specifier*), constexpr from C++17 (never before, even with a
  written `constexpr`: clang `addFunctionPointerConversion`);
- unless the lambda is `static`, an implicit static member function
  `__invoke` with the call operator's signature (clang's static invoker),
  constexpr only if `constexpr` was written.

Both are immediate functions if the call operator is
([expr.prim.lambda.closure]/10-11): `consteval` when it is written, and
escalated (`FnSpec::escalated`) when the call operator escalated
(WP-137). clang 19 rejects every use of a consteval lambda's conversion;
kiln follows the standard (`expr.prim.lambda/consteval-conversion.cpp`
is `xfail(clang)`).

A templated non-generic lambda's conversion function and invoker have
the call operator's trailing requires-clause and constraint levels
([expr.prim.lambda.closure]/9; clang `addFunctionPointerConversion`,
`share_call_constraints`): when the call operator's constraints are not
satisfied, the closure converts to no function pointer and yields no
surrogate call candidate ([over.call.object]/2, spec/sema-overload.md),
so a call in a requires-expression or a function body is "no matching
function" with the constraint notes, as for the call operator itself
(WP-881; clang 19 agrees). Tests
`expr.prim.lambda.closure/trailing-requires-call.cpp`,
`p9-trailing-requires-call-forms.cpp`,
`p9-trailing-requires-surrogate.cpp`,
`p9-trailing-requires-satisfied-run.cpp`.

Neither has a body. `Ast::closure_thunk(d)` classifies them as
`ClosureThunk::Conversion { target }` (the invoker, or the call operator
of a static lambda) and `ClosureThunk::Invoker { call_op }`; the Oven
(§7) and codegen (§8) synthesize their behaviour. For a generic lambda
both are function templates sharing the call operator's template
parameter list (§5.2). The conversion's definition is marked when it is
odr-used (`define_lambda_conversion`), and then the call operator the
invoker forwards to is odr-used: of a generic lambda, the call
operator's specialization for the conversion's template arguments, so
it is instantiated even when its return type is written rather than
deduced (WP-799).

### 3.3 Special members

[expr.prim.lambda.closure]/13-14: a closure type without captures has a
defaulted default constructor and copy assignment operator; with any
capture-default or capture it has no default constructor at all and a
deleted copy assignment operator (and no implicit move assignment).
Before C++20 every closure type is so (C++17
[expr.prim.lambda.closure]/11; clang `lambdaIsDefaultConstructibleAndAssignable`):
the definition bit `LAMBDA_CAPTURES` is set for those closures, and
`decltype(lam) x;` is "no matching constructor" with the copy and move
constructors as candidates (WP-1630). A variable whose default
initialization failed keeps a recovery expression as its initializer and
stays valid (clang `ActOnUninitializedDecl`), so `x = lam;` is then "cannot
be assigned because its copy assignment operator is implicitly deleted"
with "lambda expression begins here" (test
`expr.prim.lambda.closure/p13-special-members-by-std.cpp`). The copy and move
constructors are defaulted. `closure_default_constructible` drives the
`__is_constructible` and value-initialization answers. A closure with a
capture also loses `TRIVIAL_DEFAULT_CTOR` when its definition completes
(clang `hasTrivialDefaultConstructor` needs a default constructor), so it
is neither trivial nor POD, and codegen initializes it member-wise rather
than copying a constant (WP-162, test
`expr.prim.lambda/closure-triviality.cpp`). Before C++20 no closure type
has a default constructor (C++17 [expr.prim.lambda.closure]), so every
closure loses `TRIVIAL_DEFAULT_CTOR` there, the captureless one too: it is
neither trivial nor POD, and a local closure variable is not copied from
a constant, as in clang (WP-1520, tests
`expr.prim.lambda/closure-triviality-cxx17.cpp`,
`tests/codegen/lambda_empty_closure_local_cxx17.cpp`). Before C++20 every
form of the question agrees: `__is_constructible(T)`, its trivially and
nothrow forms, `std::is_default_constructible` and SFINAE on `T()` are
false for a captureless closure, as in clang 22, clang 19 and GCC 16.1
(clang 22 only adds a stray "no matching constructor" error under a failed
`static_assert` of the trait; WP-1652, test
`expr.prim.lambda.closure/p13-default-constructible-traits-cxx17.cpp`).

### 3.4 Names in diagnostics and dumps

A closure prints as `(lambda at <file>:<line>:<col>)` (the `[`), clang's
form, through `AstNames`. In qualified names of local entities a closure
prints as `(anonymous class)` (clang `printQualifiedName`, also used for
IR value names).

### 3.5 Linkage and mangling numbers

[basic.def.odr]/14 and Itanium §5.1.8 (clang `handleLambdaNumbering`):
a lambda in the body of an inline function or of a templated function
(an instantiation in progress, or a dependent context such as a function
template pattern or a generic lambda's body; clang
`getCurrentMangleNumberContext`'s `inTemplateInstantiation() ||
isDependentContext()`, WP-182), in a default argument in a
class definition, in a default member initializer, or in the
initializer of an inline variable or of a variable template pattern or
non-explicit specialization (instantiation sets the numbering variable
around the initializer, `subst::var_init`) gets a
mangling number per (numbering context, call-operator parameter types,
generic) and, for the last three, a context declaration that prefixes its
name. `LambdaData.mangling` records both. Any other closure has no
number and no linkage: its members are internal and it mangles as an
anonymous class (`$_N`). See abi.md §8.9.
`tests/codegen/lambda_in_template_mangling.cpp` checks the templated
cases IR-exact. An instantiated default argument (`templ/default_args.rs`)
is substituted in the function's own context with
`ManglingState.default_arg` set to the parameter (clang
`SubstDefaultArgument`'s `ContextRAII(FD)`): a closure created there is
numbered per parameter (`d_` in the local name) when the function is a
member defined in its class, and otherwise has no number and mangles as
the function's local `$_N` (`_ZZ4dfltIiEiPFivEEN3$_0clEv`; WP-203,
`tests/codegen/lambda_default_arg_template.cpp`).
A default template argument ([temp.param]/10) is substituted at each use
with `ManglingState.template_default` set (`templ/args.rs`
`default_template_arg_in`; `templ/member.rs` when a class template
specialization redeclares its member template). A closure created there
at namespace or class scope is a new type at each use and is numbered
per <lambda-sig> in that scope, with no context declaration (clang 19
and 22: `f() + f()` calls `f<UlvE_>` and `f<UlvE0_>`; WP-1062,
`tests/conformance/temp.param/lambda-default-arg-mangling-two-units-run.cpp`).
The closure parsed in the default itself gets no number.
A function template specialization's signature is substituted with
`ManglingState.signature` set to (pattern, the template's semantic
context) (`Sema::begin_signature_mangling`, around `deduce_spec`'s steps
and `get_function_spec_signature`'s substitution; the variable, default
argument and default template argument state is cleared for the
duration). A closure created there in the template's context is
numbered per <lambda-sig> in the pattern, which is also its context
declaration; one created in a class template specialization while it is
instantiated (no numbering variable of that class) is numbered in the
class (clang `Normal` with `inTemplateInstantiation()`; WP-1077,
`tests/codegen/lambda_in_function_signature.cpp`).
How many numbers a call takes follows clang's substitutions (clang 19
and 22 agree; WP-1086). A non-empty explicit template argument list
(`f<int>(…)`, a partial list, a wholly or partially explicit pack; not
`f<>`) runs the explicit-argument step on every call, even one that finds
an existing specialization (clang `SubstituteExplicitTemplateArguments`):
it substitutes the parameter types and the return type, a leading one
before the parameters and a trailing one after them, and numbers every
lambda there. Forming a new specialization substitutes the signature
once more (`FinishTemplateArgumentDeduction`); a repeat numbers nothing
in that step. So `ret2<int>()` twice, then `ret2<char>()`, numbers 1-2,
3, 4-5: the specializations' closures are `UlvE0_` and `UlvE2_`. Kiln's
step (`templ/deduce_call.rs` `subst_explicit_param_types`) substitutes a
trailing return type only when the parser recorded a lambda-expression in
it (`Sema::act_on_trailing_return_type`, `ManglingState.trailing_lambdas`),
so calls whose signature has none do no extra work; the substitution's
type is discarded (the trailing return type is a non-deduced context and
deduction substitutes it again). A partially explicit pack is left
unsubstituted there (it stays open to deduction).
A template-id whose explicit arguments complete one specialization is
resolved early (`overload/address.rs` `resolve_single_template_id`; the
early resolution and the specialization stand for clang's first
resolution and its build), but clang keeps it an overload set and runs
`ResolveAddressOfOverloadedFunction` (a function target, deducing each
template from it and the explicit arguments) or
`ResolveSingleFunctionTemplateSpecialization` (no function target) again
at each step that meets it; each run's explicit step numbers the lambdas
once more (WP-1142, clang 19 and 22 agree). Kiln re-runs the same
deductions there, discarding their result (`Sema::rerun_template_id_resolution`,
`rerun_template_id_single`, with the enclosing full-expression's pending
deleted uses and held warnings set aside, `holding_pending_uses`):
the implicit conversion (`conv::implicit_convert`, clang
`PerformImplicitConversion`; not a condition, resolved before; not an
assignment's checking step, which resolves the operand and converts the
result, `forget_template_id`), a reference bound to a temporary
([dcl.init.ref]/5.4.2, `bind_reference`; a direct binding resolves once
and reuses the result), a candidate's conversion sequence (`best.rs`
`add_candidate`, `TryCopyInitialization`; a function template candidate
reuses the sequence its deduction formed for a non-dependent parameter,
clang `CheckNonDependentConversions`, `OverloadScratch::nondep_ics`), a
call by an unqualified name with one candidate (`convert_call_args`),
the deduction of a function-like `P` (`deduce_call_arg`,
`ResolveOverloadForDeduction`), `auto` deduction (`deduce_from`, `new
auto`, an `auto` template parameter; `DeduceAutoType`), a braced scalar
or aggregate element (the list checker's verifying pass, `init.rs`), and
the perform step of a `static_cast`, C-style or functional cast
(`act_on_cast`, `TryStaticImplicitCast`). A reference to function
list-initialized from an overload set binds like a direct-initialization
(`reference_list_init`, clang `ResolveOverloadedFunctionForReferenceBinding`).
Counting the early resolution and the specialization, `f<int>` takes 2
numbers when discarded, compared or bound to a reference (`FR r = f<int>`,
`FR r{f<int>}`, `static_cast<FR>`, `reinterpret_cast`), 3 in a copy- or
direct-initialization of a pointer, an assignment, a pointer cast, a
qualified call's argument, a reference parameter, a `const FP &`, `auto
&`; 4 in a braced initializer, `auto`, `auto *`, an unqualified call
with one candidate, an `auto` or explicitly typed `F` template argument;
5 for a deduced `F` and `auto x{f<int>}`.
The count depends only on the context, not on whether the explicit
arguments alone complete a specialization (clang never forms one early).
When the target deduces a different specialization than the explicit
arguments complete (`f<int>` with `U = void` defaulted, target
`int (*)(int *, int)`), a cast, an assignment and an argument retarget
it like an initialization does ([over.over]/1; `retarget_cast_operand`,
`mismatched_template_id`, `ConvCtx::Assignment | Argument` in
`retarget_template_id`), and the first resolution that forms the
target's specialization takes over the numbers the early resolution
gave (`taking_over_numbers`; the early resolution records them per
numbering key, `OverloadScratch::template_id_numbers`, `IdNumber`); a
candidate's re-run is skipped for a mismatched template-id, whose
conversion is that resolution. Numbers given in those keys by another
signature since make the record stale; it is dropped and the resolution
numbers after them. An early resolution that deduces
nothing takes its numbers back. When the explicit arguments
complete none (`U` undeduced), the target's resolution registers the
resolved template-id like an early one (`resolve_address_of_overloaded_noted`),
so the later steps re-run it. Tests:
`lambda-in-signature-explicit-args-mismatched-spec-two-units-run.cpp`,
`…-no-early-spec-two-units-run.cpp`,
`tests/mangle/lambda_in_signature_contexts.cpp` (one template per
context, with and without the default).
The address of an overload set deduces its templates from the target
even when a non-template matches it, then prefers the non-template
([over.over]/5; clang `FindAllFunctionsThatMatchTargetTypeExactly`, then
`EliminateAllTemplateMatches`; `prefer_non_templates`, WP-1143), and only
the templates whose kind of member the target can point to
(`AddMatchingTemplateFunction`). Tests:
`lambda-in-signature-explicit-args-address-two-units-run.cpp`,
`…-explicit-args-conversions-two-units-run.cpp`,
`lambda-in-signature-address-nontemplate-match-two-units-run.cpp`,
`tests/conformance/over.over/template-id-retarget-cast-assign-argument-run.cpp`.
`tests/mangle/lambda_in_signature_explicit_args.cpp` covers every shape
checked.

## 4. Captures

### 4.1 Explicit captures ([expr.prim.lambda.capture]/1-8)

**Capture-default by scope** ([expr.prim.lambda.capture]/3, WP-1652). A
capture-default is allowed only where the lambda's innermost enclosing
scope is a block scope, or in a default member initializer (CWG1632).
`act_on_lambda_start` (the parser's action, once at the definition; not
on instantiation) applies clang's test from
`ActOnLambdaExpressionAfterIntroducer`: the context the closure is
created in is not a function and `this` is not capturable there
(`this_capturable`); then "non-local lambda expression cannot have a
capture-default" at the `=` or `&`. So it is rejected at namespace scope,
in a default argument, a template argument, a requires-clause, a static
data member's initializer and a member's declared type, and accepted in a
lambda nested in another lambda's body and in a (local or template)
class's default member initializer. A simple-capture there fails as a
capture of a variable without automatic storage duration; an init-capture
alone is allowed (tests `expr.prim.lambda.capture/capture-default-*.cpp`).

`act_on_explicit_captures` checks each capture as written, in order,
with clang's diagnostics: duplicates (`'x' can appear only once`),
`&x` with default `&`, `x` without `&` with default `=` (C++20 allows
`=, this`), non-variables, class members, entities without automatic
storage duration, `this` outside a non-static member function, `this`
in a static lambda. A simple capture names a variable found by ordinary
lookup, unless `LambdaCaptureSpec.var` already names it (a lambda rebuilt
by template instantiation, §9).

**Init-captures** ([expr.prim.lambda.capture]/6, WP-518). The parser
reads `identifier initializer` with the `&` and `...` prefixes, where the
initializer is `= initializer-clause`, `= braced-init-list`,
`( expression-list )` or `braced-init-list`, into
`LambdaCaptureSpec.init: Option<(InitStyle, Initializer)>` -- the
simple-declaration representation (`InitStyle::C` / `Call` / `List`,
`Initializer::Expr` / `Paren` / `List`), not a lambda-only one. `()` is
`expected expression` at the `)`; an initializer that fails to parse
becomes an error expression and the capture is dropped without further
diagnostics.

`add_init_capture` declares the variable at the introducer (in the
enclosing context; the call operator adopts it when its body opens), as
if by `auto x <initializer>` or `auto &x <initializer>`, and records the
style on it (`VarSpec` init style, so instantiation rebuilds the same
form). The type is deduced by `deduce_var_auto` -- the variable rule,
[dcl.type.auto.deduct]/2-4:

- `(e)`, `{e}`, `= e`: from `e`; `= {e...}`: `std::initializer_list<U>`,
  which needs a declared `std::initializer_list` (`cannot deduce type of
  initializer list because std::initializer_list was not found`).
- `(e1, e2)` and `{e1, e2}` are ill-formed (P0960 does not apply to a
  placeholder): `initializer for lambda capture 'x' contains multiple
  expressions`, at the second expression, also when the expressions are
  type-dependent (checked before dependence, as clang; a pack expansion
  counts as one). `{}` is `initializer missing for lambda capture 'x'`.
- `({e})` and `{{e}}`: `cannot deduce type for lambda capture 'x' from
  parenthesized|nested initializer list`; an overload set: `cannot
  deduce type for lambda capture 'x' from initializer of type
  '<overloaded function type>'`.

The capture wording (clang `err_init_capture_*`) is chosen by the
`entity::CAPTURE` initialization entity; a plain variable gets the
`variable 'x' with type 'auto'` forms, including the parenthesized /
nested-list check (`sema_auto_var_init_paren_braces`). The variable is
then initialized with `perform_init(ty, init, direct = style is List)`
under `entity::CAPTURE`: `(e)` and `{e}` are direct-initialization and
consider explicit constructors and conversion functions, `= e` is
copy-initialization and does not ([dcl.init.general]/16). `&r(x)` is a
reference init-capture. A type-dependent initializer is kept in the
variable form (`dependent_init_form`: a `ParenListExpr`, an
`InitListExpr` or the expression) and deduced at instantiation.

An init-capture pack (`...xs <initializer>`, [expr.prim.lambda.capture]/17)
must name an unexpanded pack in its initializer (`pack expansion does not
contain any unexpanded parameter packs`); its type is a pack expansion of
`auto` and its initializer the unexpanded pattern, expanded per element
at instantiation (§9). A non-pack init-capture whose initializer names an
unexpanded pack is `initializer contains unexpanded parameter pack`.

The captured type of a by-copy capture, explicit or implicit, must be
complete, sized and not abstract ([expr.prim.lambda.capture]/10, clang
`captureInLambda`; WP-1426, `lambda/capture.rs` `check_by_copy_capture`):
otherwise `by-copy capture of variable 'v' with incomplete type T` (and
the forward-declaration note), `... with sizeless type T`, or `by-copy
capture of value of abstract type T` (with the pure-virtual notes, once
per class). Only the outermost lambda whose capture fails reports, once
per variable; the lambda then becomes an error expression, so nothing
else (`-Wunused-value`) is said about it. A by-reference capture is not
checked. `[*this]` of an abstract class and a by-copy init-capture of
one are `allocating an object of abstract class type` instead, and leave
the lambda valid.

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
value` / `by reference`). A member of a block-scope anonymous union
names a member of the union's unnamed variable, which a lambda with a
capture-default cannot capture either: `unnamed variable cannot be
implicitly captured in a lambda expression` with a `declared here` note at
the union (clang `isVariableCapturable`, WP-1405). A use of an enclosing function's local outside
any lambda is `reference to local variable 'x' declared in enclosing
function`. So is an odr-use whose innermost enclosing context is a local
class or enumeration nested in the variable's function, inside a lambda
body or not ([basic.def.odr]/10: a class or enumeration scope is no
block scope, so the variable is not odr-usable there; `capture_chain`,
clang `getParentOfCapturingContextOrNull`): an enumerator's value, a
bit-field width, a member's array bound, noexcept-specifier or default
member initializer (clang accepts the last; GCC and the text reject it).
A use that is no odr-use (a constant's value, an unevaluated operand)
stays valid there (WP-1621, basic.def.odr/odr-usable-local-class-*.cpp).

In a dependent context (a generic lambda's body, or a lambda in a
template) no use is an odr-use yet ([temp.res.general]/6), but a generic
lambda's closure type is complete before its call operator is ever
instantiated, so its implicit captures are decided in the definition
(WP-581; clang `DoMarkPotentialCapture` and
`CheckIfAnyEnclosingLambdasMustCaptureAnyPotentialCaptures`). A name of
a might-be-constant enclosing local there is a *potential capture*
(`note_potential_capture`, `Sema::potential_caps`; never a reference
whose initializer is a constant expression), resolved at the end of its
full-expression (`resolve_potential_captures`): it is captured by the
enclosing lambdas unless the full-expression is not
instantiation-dependent and the name is an lvalue-to-rvalue potential
result of a variable usable in constant expressions. So
`[=](auto x) { return x + n; }` with `const int n = 10` captures `n`
(as clang), `[=](auto x) { int y = n; return x + y; }` does not, and
`const T& rr = *p; [&](const auto& x) { return x == rr; }` captures
`rr`. When no lambda can capture it, the use is diagnosed at once if the
full-expression is not dependent or the variable can never be a
constant expression (`never_constant`); otherwise the instantiation
decides. When a lambda nested in a generic lambda is formed again for a
specialization of the outer call operator, the names in the shared
(non-dependent) expressions of its body capture as fresh names would
(`Substituter::note_shared_captures`, with `this`). Tests:
`tests/conformance/expr.prim.lambda/*generic-captured-reference*`.

`this` is captured implicitly by a use of `this` (explicit or through a
member access) under either capture-default; under `=` clang 19 warns
`implicit capture of 'this' with a capture default of '=' is
deprecated` with its note. `capture_this` walks the enclosing lambdas as
for variables. An unqualified call whose overload set mixes static and
instance member functions uses the implicit `this` only when overload
resolution picks an instance member (`finish_member_winner`), so a lambda
without a capture-default may call the static one (clang's
`BuildPossibleImplicitMemberExpr`; WP-726,
`tests/conformance/expr.prim.lambda.capture/implicit-this-*-overload-winner.cpp`).
The rule is general (WP-779; [expr.prim.lambda.capture]/7-8,
[class.mfct.non.static]/2):

- A set of instance members only (an explicit object member function is
  one) captures `this` when the name is formed, as clang's
  `BuildImplicitMemberExpr` with `IsKnownInstance`.
- A mixed set whose call is resolved when it is formed captures `this`
  only for an instance winner, qualified (`S::f(s)`) or not.
- In a template (a member of a class template, a member function
  template, a member found through a dependent base) the implicit access
  is kept dependent without a capture, and the lambda is formed again at
  instantiation: `templ/subst` rebuilds an implicit `this` base without
  capturing it (`member_base_deferred`) and captures once the member is
  looked up again (`note_deferred_this`), except for a mixed set, which
  captures at its winner, and a static member. clang's `TreeTransform`
  does not transform an implicit access's `this` either.
- A mixed set whose call stays dependent (an argument depends on a
  generic lambda's parameter) is a potential use of `this`
  (`note_implicit_this_of_dependent_call`): if every open lambda can
  capture `this` (each has a capture-default), the innermost lambda whose
  closure is formed outside a template captures it then, with the lambdas
  enclosing it, since the closure is complete before the call operator is
  instantiated; otherwise nothing is captured (clang
  `tryImplicitlyCaptureThisIfImplicitMemberFunctionAccessWithDependentArgs`
  and `getStackIndexOfNearestEnclosingCaptureReadyLambda`). An instance
  winner in the instantiated call operator then is `'this' cannot be
  implicitly captured in this context` (`finish_member_winner`).

Tests: `implicit-this-overload-winner-template-run.cpp`,
`implicit-this-overload-winner-generic-run.cpp`,
`implicit-this-overload-winner-inst-error.cpp`. Gaps: a qualified
`A::f()` capture is reported at the member name, clang at the qualifier
(both before and after WP-779); clang repeats `-Wdeprecated-this-capture`
in each instantiation of a nested generic lambda, kiln reports it once.

(WP-716) A generic lambda's call operator instantiated later gets the
enclosing `this` ([expr.prim.this]/2), not the closure's: the enclosing
member function's, through each enclosing lambda, or an NSDMI's class
under a capture-default or `this` capture (`instantiated_lambda_this`).
A dependent call naming class members by an unqualified id (e.g.
`Get<T>(v)` in a generic lambda) captures `this` when the lambda is
defined, as clang's `BuildImplicitMemberExpr`: when every candidate is
a non-static member function, with the usual diagnostics, and when the
set mixes static and non-static members only if `this` is capturable
without error (a potential capture, `this_capturable`;
`note_implicit_this_of_dependent_call`). Tests:
`expr.prim.this/generic-lambda-implicit-this-member-template-run.cpp`.

### 4.3 The closure's members

Each capture by copy, and each init-capture, gets an unnamed non-static
data member in capture order (explicit captures as written, then implicit
ones in order of first odr-use); by-reference captures get a reference
member (clang's layout, matching `-fdump-record-layouts`). `this` is a
pointer member; `*this` a member of the class type, whose type keeps its
sugar. A captured array is a member array, initialized element-wise.
The member initializers (the `LambdaExpr`'s children) copy-initialize from the entities named, as
[expr.prim.lambda.capture]/15 requires, with the usual temporaries and
cleanups. An init-capture's member is initialized by its variable's
initializer as built (clang's shape: `[x{1}]` an `InitListExpr`, `[y(2)]`
the bare expression, `[&r(a)]` the `DeclRefExpr`, a class type its
`CXXConstructExpr`); the Oven and codegen evaluate it as any member
initializer.

### 4.4 Names of captured entities in the body

A `DeclRefExpr` to a captured entity has `ExprFlags::REFERS_TO_ENCLOSING`
and the type of the member's access: `const` in a non-`mutable` lambda for
a by-copy capture ([expr.prim.lambda.capture]/11, so assignment reports
`cannot assign to a variable captured by copy in a non-mutable lambda`).
Such a name is not a candidate for implicit move or NRVO
([class.copy.elision]/3). With an explicit object parameter, a by-copy
capture is `const` exactly when the parameter's (referenced) type is
const-qualified, whatever `mutable` says ([expr.prim.id.unqual]/3;
`lambda_capture_const`, clang `lambdaCaptureShouldBeConst`). For a
generic lambda it is decided per specialization: the call operator
consulted is the one enclosing the current context (the specialization
whose body is being instantiated, `current_call_op`), so `this const
Self&` makes the captures const there and `this Self&` with a non-const
object does not. To that end a use of a variable captured by copy
(explicitly, or implied by a `=` default) in a lambda whose explicit
object parameter has a dependent type is type-, value- and
instantiation-dependent ([temp.dep.expr]/3;
`captured_by_copy_in_dependent_object_lambda`,
`ExprArena::mark_type_dependent`, clang
`isCapturedByCopyInLambdaWithExplicitObjectParameter`), so
`decltype((x))` is recomputed in each specialization. A use in such a
specialization that is not an odr-use and has no capture still gets the
type a capture-default would give it ([expr.prim.id.unqual]/3).
`this` in the body is the captured `this` or the address of the
`*this` copy. Likewise (WP-1472) `this` written or implied in a lambda
whose innermost `*this`-capturing enclosing lambda has a dependent
explicit object parameter not known to be const is type-dependent
(`build_this`), so a member `x` of the copy and `decltype((x))` are
re-formed in each specialization: `const int&` when the object
parameter refers to a const closure (clang 22 and GCC 16.1 agree).

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
A conversion specialization is named by its substituted target type with
the placeholder return type kept (`operator auto (*)(int)`, as clang;
mangling uses the template's name). Escalation is per specialization
(WP-158): when the call operator specialization escalated, the
conversion and invoker specializations are immediate too (§3.2;
`expr.prim.lambda/consteval-conversion-generic.cpp`, which clang 19
accepts). The escalation reason note names a generic call operator
specialization unqualified (`'operator()<int>' is an immediate
function because ...`); clang gives none for a non-template call
operator.

### 5.3 Return types

`decltype(auto)` mangles as `Dc`. A return type deduced from a dependent
expression is deduced at instantiation; `void` deduction in a
dependent body is deferred to the specialization.

## 6. Diagnostics

All lambda diagnostics use clang's wording, location and notes (the ids
in `kiln_diag/src/ids/parse.rs` and `ids/sema_*.rs`: `parse_*lambda*`, `parse_expected_*capture*`,
`sema_lambda_impcap`, `sema_capture_*`, `sema_this_capture`,
`sema_deprecated_this_capture`, `sema_assign_captured_by_copy`,
`sema_reference_to_local_in_enclosing`, `sema_falloff_nonvoid_lambda*`
and their notes; for explicit object parameters,
`sema_explicit_object_param_*`, `sema_explicit_object_lambda_mutable`
and `sema_explicit_object_lambda_capture_type`, §3.1).
`-Wdeprecated-this-capture` and `-Wreturn-type` control the warnings.

## 7. Evaluation (the Oven)

A closure object is an ordinary class object in the Oven's memory; the
`LambdaExpr` initializes its members in order from its children.

- **Calls** go through the ordinary member call path to the call
  operator (or its specialization). The prologue (`lambda_prologue`)
  sets the body's `this` to the captured `this`, or to the address of
  the captured `*this` copy. A call operator with an explicit object
  parameter is called through the generic call path with the object as
  argument 0 (also for an overloaded `operator()` call); its closure
  object (`closure_reg`) is that parameter or its referent, adjusted to
  its closure-type base when the parameter's class derives from the
  closure, so captured names and `this` read the right object.
- **Captured names** (`captured_var_place`): a name with
  `REFERS_TO_ENCLOSING` in a call operator that captures it is the closure
  member's place, or the referent of a reference member. A by-reference
  capture outliving its referent is a dangling access, diagnosed as for
  any reference (`constexpr-capture-lifetime.cpp`).
- **Outside a call of the call operator** (WP-1590; a constant
  expression evaluated in the body: `constexpr int j = *&n;`, a
  `static_assert`, a template argument) there is no closure object. A
  name that is not an odr-use (`NonOdrUse::Constant`, [basic.def.odr]/5)
  is the variable and reads its value, at no extra cost. An odr-use of an
  entity captured by copy names a member of the closure object
  ([expr.prim.lambda.capture]/11; a by-reference capture of a by-copy
  capture names the outer closure's member, /12): sema marks such a
  `DeclRefExpr` `ExprFlags::NAMES_CLOSURE_COPY` (bit 4 of its flags, no
  extra storage; `Sema::odr_use_names_closure_copy` walks the lambdas
  between the name and the variable's scope, taking an implicit capture's
  kind from the capture-default), and the Oven gives it an unbacked
  external object (`GKind::Capture`, `Oven::closure_copy_object`: one per
  variable per translation unit, kept in `Oven::unknowns`). Its global
  resolves provisionally, afresh in each evaluation: a codegen fold
  outside a constant context (`RunState::fold_outer`, outermost context
  only) resolves it to the variable instead, as clang's codegen folds
  `(&ca)->v` in the body to the variable's value. A variable's
  reference is marked when its full-expression ends and shows it is an
  odr-use (`act_on_full_expr_at`; in a template definition
  `resolve_potential_capture_list`), a binding's when it is built. Every
  constant expression ends its full-expression before it is evaluated
  (`act_on_constant_full_expr`; a case value too since WP-1606, as clang's
  `EvaluateConvertedConstantExpression` finishes it first), so `case i:`
  in a `[=]` body reads the variable and `case *&i:` is rejected, in and
  outside a template (tests
  `expr.prim.lambda.capture/odr-use-case-value{,-ok}.cpp`). The Oven
  caches a compiled thunk per expression (`compile_thunk`): a fold of an
  expression before its full-expression ends compiles it unmarked for
  good, so no fold may precede that end: a non-type template argument's
  narrowing check folds its operand after it (`templ/args.rs`, WP-1622;
  before, a temporary's destructor and the odr-use marking were missing
  from the thunk the evaluation reused). `T<*&i>` in a `[=]` body reads
  the variable by a rule of its own: while a non-type template
  argument's full-expression ends (`Sema::capture_reads_variable`), an
  odr-use still captures but is not marked `NAMES_CLOSURE_COPY`, as clang
  22 and GCC 16.1 read the variable (docs/status/deviations.md;
  `temp.arg.nontype/closure-copy-odr-use-*.cpp`). This holds for a
  parameter of class type too (`convert_class_nttp_arg`: `I<v[K]>` with
  `operator[]` returning a reference into a `constexpr` local, in a
  generic lambda, where WP-1655's call-operand rule does not apply;
  WP-1717, a regression since 1.2.1). The keyword `alignas`
  argument shares the rule (`check_alignment_expr`, WP-1640) where both
  accept: in a lambda whose enclosing entities are not templated other
  than generic lambdas' call operators
  (`Sema::lambda_in_enclosing_template`); GNU `aligned(*&i)` and a lambda
  in a function or class template keep the text's reading, as GCC does
  (`dcl.align/closure-copy-odr-use-*.cpp`). Reading it is
  diagnosed ("kiln: read of variable 'n' captured by copy is not allowed
  in a constant expression outside a call of the lambda": clang reads the
  variable and accepts), its address is not constant (clang's "pointer to
  'n' is not a constant expression", as for the variable), and naming it
  without reading (`sizeof`, a member function that reads nothing) is
  constant, as for an object of unknown value ([expr.const]/10). A
  by-reference capture names the variable and is unchanged; an
  init-capture is a variable of the lambda and is unchanged. A structured
  binding is always odr-used ([basic.def.odr]/5), so by the text `[=] {
  constexpr int j = a; }` with `constexpr auto [a, b] = ...` is rejected.
  clang 22 and GCC 16.1 both accept reading the member through a
  reference parameter or `this` (`rd(n)`, `s.val()`), the capture of a
  capture and the binding's read, which the text rejects; kiln follows
  them (WP-1655): sema leaves those names unmarked (spec/sema.md, the
  by-copy capture a call receives), so they read the variable here
  (`expr.prim.lambda.capture/constexpr-call-reads-by-copy-capture-*.cpp`,
  `odr-use-closure-member-constexpr*.cpp`, `by-copy-capture-member-run.cpp`).
- **Thunks**: the conversion function returns the address of its target;
  the static invoker runs the call operator's body with no object
  (its body never uses `this`: the lambda has no captures). Notes print
  such a call as clang's `describeCall` does.

Nothing about closures needs heap allocation in the Oven beyond the
object itself.

## 8. Code generation

`kiln_codegen/src/lambda.rs`. The closure is an ordinary class type
(named as clang names it); capture members are fields. The call
operator's prologue records the closure and the captured `this`; with an
explicit object parameter the closure is that parameter (its referent
for a reference), adjusted by the non-virtual offset of the closure-type
base when its class derives from the closure (`explicit_object_closure`,
`nv_base_offset`; clang `EmitLValueForLambdaField`), derived again at
each capture access (`Fx::closure_obj`: a reference parameter is loaded
once per access, as clang does; WP-162,
`tests/codegen/lambda_explicit_object.cpp`). The
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
   The rebuilt declarator is marked as a lambda's (`Declarator::lambda`,
   as the parser marks it), so its implied `auto` return type is exempt
   from the C++11 "'auto' return without trailing return type" gate
   ([dcl.spec.auto], WP-1123/1134) in every instantiation (WP-1213).
4. The constraints are not substituted: the new call operator (or its
   template) is entered in `ConceptState::member_constraints` with the
   pattern and the current arguments, so they are checked with this
   instantiation's levels outside the operator's own, as for a member of
   a class template.
5. Default arguments, then the body. In a generic lambda the body is
   rebuilt as a pattern (its own parameters stay dependent); a
   function parameter pack of the pattern stays a pack
   (`param_infos`, `map_params`, `size_of_pack` handle a retained pack).

A simple-capture pack expansion (`[ts...]`, `[&ts...]`,
[expr.prim.lambda.capture]/17) whose pack is an instantiated function
parameter pack becomes one simple capture per element, each resolved to
its `ParmVar` (`LambdaCaptureSpec.var`); the elements share a name, so
the "can appear only once" check compares the resolved variables. An
outer function parameter pack named in a generic lambda's body is a
`FunctionParmPackExpr` that captures every element (spec/sema-templates.md
§7.2).

An init-capture is rebuilt from its pattern variable's initializer and
init style (`init_capture_init`, subst `initializer(init, style)`: a
`ParenListExpr` gives `Initializer::Paren`, an `InitListExpr`
`Initializer::List`, a single expression of `Call` style a one-element
`Paren`) and replayed through `add_init_capture` (§4.1), so deduction and
direct- versus copy-initialization are those of the written form. An
init-capture pack (`[...xs = ts]`, `[...xs(ts)]`, `[&...xs{ts}]`) becomes
one init-capture per element, each with the element's initializer in the
pattern's style. A capture pack of a pack that is still unexpanded (in a
generic lambda substituted one level) remains "not yet implemented".

Uses of `this` in a rebuilt body capture it again (`note_this_use`);
`note_shared_this` keeps the enclosing lambda's `this` capture in step.

A specialization of a generic lambda's call operator does not rebuild the
closure: its body (and the body of a generic lambda nested in it) names
the init-capture variables the closure already has
([expr.prim.lambda.capture]/6). `find_instantiated_decl` maps an
init-capture variable that the current substitution has not rebuilt (a
rebuilt one is in `locals`, step 1) to itself, as clang's
`addInstantiatedCapturesToScope` does (WP-286;
`tests/conformance/expr.prim.lambda/run-generic-init-capture.cpp`).

**Never-dependent closures (WP-1141).** A lambda substituted in an
unevaluated operand or a constant-evaluated context, whose context (the
current declaration context, skipping requires-expression bodies) is a
namespace scope or has a non-dependent parent, gets a closure type that
is not dependent (clang `TreeTransform::TransformLambdaExpr`,
`LambdaDependencyKind::LDK_NeverDependent`; `Sema::substituted_lambda_never_dependent`).
This is a member template's signature substituted while its class
template is instantiated: in `template<class T> struct S { template<class
U> auto f(U) -> decltype([]{ return 5; }); };`, `S<int>::f`'s return type
is the closure of `S<int>` and `S<int>::f<int>` mangles it as a type,
`_ZN1SIiE4fIiEENS0_UlvE_ET_`, not as `DTLNS0_UlvE_EEE`. The closure is
built with `templates.dependent_depth` cleared
(`Sema::act_on_lambda_start_in`, `AstBuilder::closure_record`; restored
at `act_on_lambda_end`/`act_on_lambda_error`), so its members and call
operator are ordinary (an `auto` return type is deduced). With
`-fclang-compat>=22` a context whose parent is a generic lambda's closure
is excluded (clang 22 adds `!(isa<CXXRecordDecl>(DC->getParent()) &&
isGenericLambda())`). Its call operator's own template parameters keep
their depth in the <lambda-sig>, `UlTL0__E_` (clang
`mangleTemplateParameter`). A lambda's dependence comes from its closure
type alone (clang `computeDependence(LambdaExpr *)`): an init-capture
contributes only unexpanded packs and errors, so `decltype([u]{...})` is
the closure as well. Deduction substitutes the specialization's
(instantiation-dependent) function type again, and that closure, numbered
in the member template, is the one whose call operator the program uses
(`_ZZN1SIiE4fET_ENKUlvE_clEv`). A variable template member whose type is
neither instantiation-dependent nor a placeholder keeps the type its class
instantiation formed (`instantiate_var_spec_decl`), as clang does, so its
closure is numbered in the class. A lambda in the default of a non-type
template parameter of a member template, substituted while the class
template is instantiated (`templ/member.rs::param_list`), is not
never-dependent outside an unevaluated operand: the member template's own
parameters are still open there (`TemplState::member_head_default`, with
`dependent_depth` raised), so `template<auto N, auto F = [] { return
N.n; }>` sees `N` and each use is a new closure numbered in the class,
while `class U = decltype([] {})` stays the one closure of the class, as
clang 19 and 22 mangle them
(tests/conformance/temp.param/lambda-default-arg-member-nttp-mangling-run.cpp)
(tests/conformance/temp.arg.nontype/lambda-default-argument-member.cpp,
lambda-default-argument-siblings.cpp; WP-1497).

## 10. Tests

- `tests/conformance/expr.prim.lambda/`: 79 tests (xfails are marked
  per compiler in the files) covering syntax errors, captures and their
  diagnostics, closure members, generic lambdas, conversions, constant
  evaluation, run-time behaviour, mangling and lambdas in templates
  (WP-282: `run-template-param-pack-fold`, `run-capture-pack`,
  `constexpr-template-param-pack-fold`, `errors-template-param-pack-length`),
  plus seven `expect: ast` goldens (`ast-*.cpp`) against clang's JSON.
- `tests/conformance/dcl.fct/p0847-lambda-*.cpp` (explicit object
  parameters: compile, run including derived closures, the `[*this]`
  run test under clang >= 22, and the diagnostics) and
  `tests/conformance/expr.prim.lambda/p2036-*.cpp` (init-capture scope).
- Init-capture forms (WP-518): `run-init-capture-forms`,
  `run-init-capture-unique-ptr` (`[p(std::move(q))]`),
  `constexpr-init-capture-forms` (templates and packs),
  `errors-init-capture-forms` (`verify`), `errors-init-capture-forms-pack`,
  the `ast-init-capture-forms` golden, and
  `tests/conformance/dcl.spec.auto/direct-init-forms.cpp` (the variable
  rule); `consteval-lambda` and `template-operator-call` for the
  WP-123 rows.
- `tests/codegen/lambda_basic.cpp`, `lambda_captures.cpp`,
  `lambda_captures_class.cpp`, `lambda_names.cpp`,
  `lambda_init_capture_forms.cpp`: IR-exact.
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
   a capture pack of a pack still unexpanded after substitution (§9).
   Init-captures of every form (WP-518), capture packs, `consteval` and
   immediate-escalating lambdas, a lambda-expression as a template
   argument, calling an `auto` template parameter whose default is a
   closure (`template-argument.cpp`), and `x.template operator()<T>()`
   (WP-364, spec/sema-templates.md) all work
   (`consteval-lambda.cpp`, `template-operator-call.cpp`).
4. **Diagnostic wording.** The init-capture deduction errors use clang's
   capture wording; other init-capture errors (initialization failures)
   use the general initialization wording, as clang's do.

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
