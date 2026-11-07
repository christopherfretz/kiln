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
   add the call operator's `const`.
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
`__is_constructible` and value-initialization answers. A closure with a
capture also loses `TRIVIAL_DEFAULT_CTOR` when its definition completes
(clang `hasTrivialDefaultConstructor` needs a default constructor), so it
is neither trivial nor POD, and codegen initializes it member-wise rather
than copying a constant (WP-162, test
`expr.prim.lambda/closure-triviality.cpp`).

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

## 4. Captures

### 4.1 Explicit captures ([expr.prim.lambda.capture]/1-8)

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
In a class template, where `*this` is dependent, the capture is still
eager.

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
`*this` copy.

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
