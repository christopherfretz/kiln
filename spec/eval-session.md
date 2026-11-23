# Evaluation sessions (WP-1995)

Status: implemented (WP-1995; nested work, §5.1, WP-2073; `constexpr`
entries, §3/§4, WP-2072; results of class and other types, §3/§4.1,
WP-2071; class, exception-specification, constraint and default member
initializer units, §5.1, WP-2075). Code: `crates/kiln_sema/src/session.rs`
(the session, entries, runs, reports, diagnostic replay),
`crates/kiln_oven/src/session.rs` (budgets, cancellation, the Oven side
of a run), `crates/kiln_diag/src/engine.rs` (`DiagEngine::capture_begin`,
the per-run capture), `crates/kiln_parse/src/lib.rs`
(`parse_preprocessed_retained`), `crates/kiln_driver/src/session.rs`
(`with_eval_session`, from a command line). Standard: [expr.const]
(the run of an immediate entry is an immediate invocation,
[expr.const.imm]/1; of a `constexpr` entry, the initialization of a
notional `constexpr` variable, [dcl.constexpr]/6, [expr.const.init]/4;
both manifestly constant-evaluated, [expr.const.defns]/1, at the end of
the translation unit), [temp.point]/8 (the instantiations a run needs),
[except.terminate] (an exception leaving the entry).

A tooling client compiles one translation unit (a *transform*), then
evaluates a `constexpr` or `consteval` *entry* function of it many
times, once per
input. Each evaluation is a *run*. A run talks to the client through
host calls (spec/host-calls.md) and leaves nothing behind that a later
run can observe. Nothing here changes what kiln does for a translation
unit that opens no session.

## 1. Design: retain the finished state

The session keeps the finished front end of the translation unit: the
`Sema` after the end-of-translation-unit work (pending instantiations
done, end-of-unit checks run), which owns the AST and the Oven. A run
is a constant evaluation of the entry against that state (§4), as one
at the end of the translation unit would be.

The alternative, re-entering from a parsed prefix (`parse_prefix`,
`continue_preprocessed`) per run, was rejected: every run would repeat
the main file's parse and the end-of-unit work (instantiating what the
transform uses), which is the cost a session exists to pay once; and a
prefix cannot hold the transform's own definitions, so each run would
have to re-declare them. Retaining costs the memory of the finished
front end, which the client holds anyway while it runs (§10 has the
measurements), and nothing per run beyond the run's own state.

## 2. The interface

In `kiln_sema::session` (re-exported by `kiln_parse`; the budget,
token and value types are `kiln_oven`'s):

```rust
// Opening.
EvalSession::open(sema: Sema<'a, 'sm>) -> Result<EvalSession<'a, 'sm>, OpenError>;
kiln_parse::parse_preprocessed_retained(pp, interner, diags, target, opts, limits, tu_host)
    -> (Sema<'a, 'sm>, Preprocessor<'sm, 'a>);
kiln_driver::session::with_eval_session(argv0, command_line, io, |front_end| ...) -> R;
front_end.render(&report.diagnostics) -> String;          // §7 (WP-2324)
kiln_diag::render_diagnostics(sm, &diag_options, records) -> String;

// Entries.
session.entry(f: DeclId) -> Result<Entry, EntryError>;
session.entry_named("ns::name") -> Result<Entry, EntryError>;

// Runs.
session.run(entry, &mut dyn HostCalls, RunBudget, &CancelToken) -> RunReport;

// Memory.
session.kept() -> KeptMemory;
session.clear_memo() -> usize;

// Closing.
drop(session); session.into_sema() -> Sema;
```

- **Opening from parts.** `parse_preprocessed_retained` parses as
  `parse_preprocessed_with_host` does and returns the finished `Sema`
  instead of a `ParsedTu`; `EvalSession::open` takes it. The handler
  given to the parse (`tu_host`) is the translation unit's: it makes
  `__builtin_kiln_host_call` visible (spec/host-calls.md §1) and serves
  the evaluations of the translation unit itself. `open` takes it out of
  the Oven ([`EvalSession::take_tu_host_calls`] returns it): runs use
  only their own handler (§6).
- **Opening from a command line.** `with_eval_session` takes a driver
  command line naming exactly one C++ source input (the driver's
  include paths, `-D`/`-U`, `-std=`, `-resource-dir`, `-stdlib=`,
  `--target=`, `-W` options and constexpr limits apply as for
  `-fsyntax-only`) and calls the closure with a `SessionFrontEnd`
  (`#[non_exhaustive]`: the session, the source manager, the main file,
  the command line's `DiagOptions`, the translation unit's own
  diagnostics, `tu_diagnostics`, its warnings and remarks as printed,
  and the resources it consulted, `consulted`; later fields may be
  added) or with the reason no session was opened.
  The builtin is visible (a placeholder handler that fails every call is
  the translation unit's). The translation unit's own diagnostics go to
  `io`'s standard error as the driver prints them. The closure form is
  forced by the borrows: the session borrows the diagnostics engine,
  which borrows the source manager, both owned by the call.
- **Refusal.** `open` fails with `OpenError::TuHasErrors { errors }`
  when the translation unit reported an error (an error-limit fatal
  included). `with_eval_session` says why it opened no session with a
  `SessionRefused` (`#[non_exhaustive]`; WP-2322), one case each:
  - `CommandLine { message }`: the command line does not name exactly
    one C++ source input (`message` says so), or the driver rejected it
    before the translation unit's job ran (an unknown `-std=`, a missing
    input, an unusable toolchain): `message` is the driver's first error
    as the text form prints it after `error: ` (with its ` [option]`
    suffix when options are shown), the whole report on standard error;
  - `TuHasErrors { errors, diagnostics }`: the unit reported errors and
    no fatal error but the error limit's (`diag_too_many_errors`: the
    limit only cuts the list; it counts as one error); `diagnostics` is
    every record the unit's engine sent to its sink, warnings included,
    in order (`kiln_diag::Diagnostic`, as `RunReport::diagnostics`);
  - `Stopped { diagnostics }`: the front end stopped: a fatal error other
    than the error limit's (a header not found, an internal limit such as
    the include depth), the fatal record last; or no tree was built (the
    job's preprocessor or target could not be set up, with what it
    reported);
  - `Panicked(text)`: the front end panicked; the payload as text, as
    `run_front_end`'s `FrontEndError::Panicked` (spec/driver.md
    "Front-end library entry"). The panic is caught (the panic hook is
    not touched) and the closure is called with this; a panic of the
    closure itself is not caught and propagates to the caller.

  **What outlives the call** (WP-2332). The records' locations (their
  `span`s and their notes') are the unit's source manager's, which ends
  with the call: inside the closure, which gets the refusal while it
  lives, they are what the unit reported, but after `with_eval_session`
  returns nothing can render or resolve them. So `TuHasErrors` and
  `Stopped` (each `#[non_exhaustive]`) also carry **`rendered`**: the
  records rendered as the driver printed them on standard error, by the
  same renderer (`kiln_diag::render_diagnostics` with the command line's
  `DiagOptions`: its format, text or JSON, and display settings; the
  include stack, source lines and carets included), without the summary
  line (`N errors generated.`) and without `drv_eval_session_tu_errors`,
  which follow it there. A client keeps `rendered` (and the records'
  level, id and message text, which hold no location) after the call;
  `SessionRefused::rendered()` gives the text of every case (the message
  of `CommandLine`, the payload of `Panicked`). Test:
  `crates/kiln_driver/tests/eval_session.rs`
  `a_refusal_renders_after_the_call` (a unit with errors, one in an
  included header, and a stopped one, read after the call; the text is
  the start of what standard error got; the JSON format).
  **Records as data, the stack guard** (WP-2827). `TuHasErrors` and
  `Stopped` also carry **`records`**: the same diagnostics as
  `kiln_diag::DiagRecord`s (spec/diag.md §10.4), each with its id,
  severity, message, display group, option suffix, caret location
  (`RecordLocation`: the buffer's `FileId` and byte offset, the presumed
  file name, line and byte column, as the text header prints them) and
  its notes (attached and macro-expansion ones, as the rendering shows
  them), which hold no source-manager location and outlive the call;
  `SessionRefused::records()` gives them (empty for `CommandLine` and
  `Panicked`). `Stopped` carries **`stack_exhausted`**: the phase
  (`StackPhase`: parsing, template instantiation, constant evaluation)
  when the fatal error that stopped the unit is the native-stack
  guard's `sema_stack_exhausted` (spec/sema-templates.md §10.3; read
  back from its record's message by `StackPhase::of_record`), `None`
  for any other stop; `SessionRefused::stack_exhausted()` is `None` for
  every other case. A client tells a stack trip, the host's limit
  (`-fkiln-stack-size`), from an error of the program without reading
  the rendered text. The records are made once, when the unit is
  refused, by the layout the rendering uses; an opened session's own
  warnings (`tu_diagnostics`) and a run's diagnostics become records
  with `SessionFrontEnd::records`. Tests:
  `crates/kiln_driver/tests/eval_session.rs` `a_refusal_is_data` (an
  error; a warning and an error; an error with a note; an error in a
  macro's expansion; `#line`) and `a_stack_trip_is_flagged` (the
  parenthesis stress test on an 8 MiB guarded thread; a missing header;
  errors only).
  A unit with errors or a stopped one also gets
  `drv_eval_session_tu_errors` ("cannot open an evaluation session: the
  translation unit has N error(s)") on standard error. Test:
  `crates/kiln_driver/tests/eval_session.rs` `refusals_say_why` (each
  case, the error limit, `-fkiln-crash-for-testing=parse`, a panicking
  closure).
- **Words, no response files** (WP-2411). The command line is taken
  as argv words already split: no `@file` word is read as a response
  file (the driver's `rsp::expand`, spec/driver.md "Response files",
  does not run), so `@x` is an ordinary word, an input named `@x` (a
  second input refuses the session as `CommandLine`). A client that
  validated the words one by one knows every option that reaches the
  driver; one that wants response files expands them itself
  (`kiln_driver::rsp::expand_in`) and validates the result. There is no
  option to turn expansion on: a session's command line is the
  client's, never a build's. Test: `crates/kiln_driver/tests/eval_session.rs`
  `a_sessions_words_are_not_response_files`.
- **Consulted resources** (WP-2412). `SessionFrontEnd::consulted` lists
  the paths the unit depends on that are in no buffer it entered: each
  `#embed` resource (`ConsultedKind::Embed`) and each `__has_include` /
  `__has_embed` hit (`Probe`), with what was read there
  (`Content { size, digest }`), in the order first met, each once; the
  list `FrontEnd::consulted` gives for the same command line
  (spec/driver.md "Front-end library entry"). A run reads no file, so
  the list is the unit's for every run. Test:
  `crates/kiln_driver/tests/eval_session.rs`
  `consulted_resources_are_listed`.
- **The driver's stages** (WP-2120). `with_eval_session` runs the
  command line through the front-end library entry's stages
  (`front_end::prepare`, spec/driver.md "Front-end library entry"): the
  same checks, toolchain, driver warnings, `-v` block, standard and job
  as `kiln -fsyntax-only`, with the action forced to `-fsyntax-only`
  (so `-c` / `-o` on the line are accepted and nothing is written), the
  PCH probe off, `-verify` and `-include-pch` refused, the module cache
  read only, and the process environment.

## 3. Entries

An entry is a function the session may run:

- a **`constexpr` or `consteval` function** ([dcl.constexpr]/1; a
  function escalated to immediate, [expr.const.imm]/4.2, was declared
  `constexpr` and counts as one); a function that is neither is
  `EntryError::NotConstexpr` (WP-2072; `NotConsteval` before). A
  function template is not a function (`NotAFunction`), but a
  `constexpr` or `consteval` specialization of one is accepted by
  `DeclId`. An entry is *immediate* when the function is an immediate
  function ([expr.const.imm]/4: `consteval`, or escalated), read when
  the run reports a failure (a specialization a run instantiates
  escalates in that run, and stays so for the session); §4 gives each
  kind's run;
- with **no parameters** (a C-style `(void)` included; not variadic):
  else `HasParameters`;
- returning **`void` or a result type** (cv-qualifiers ignored): else
  `WrongReturnType`. A result type (WP-2071; before it, only the
  integral types) is a type whose value can be the
  value of a `constexpr` variable after the run and be read without
  the translation unit, §4.1. That is a literal type
  ([basic.types.general]/10; a `constexpr` variable has one,
  [dcl.constexpr]/6) built only from: the integral types, `_BitInt(N)`
  with N <= 128, enumerations, the floating-point types, `std::nullptr_t`,
  `std::meta::info`, pointers to objects and functions, pointers to
  members, arrays of known bound of result types, `_Complex` of a
  floating-point type, and classes (unions included) whose bases and
  non-static data members all have result types (a reference member's
  value is what it binds, read as a pointer). Refused at resolve
  with `WrongReturnType`, each a type no run could return to the host:
  a reference (already refused: a reference return is no object), a
  non-literal type (a class with a non-`constexpr` destructor, or with
  no `constexpr` constructor that is not an aggregate or closure type),
  `_BitInt(N)` with N > 128, and a vector type. A class with virtual
  bases is a result type when its bases and members are (WP-2210; a
  literal type in C++26, P3533R2); its virtual base
  subobjects are decoded as §4.1 says. The
  check walks the return type once, at resolve; it keeps nothing;
- **defined** in the translation unit: else `Undefined`.

`entry_named` looks up a qualified name (`f`, `ns::f`, `::ns::f`,
`ns::C::f` for a static member function; a scope may be named
through a namespace alias or a type alias of a class; inline
namespaces are searched) from the translation unit's scope ([namespace.qual],
[class.qual]): no declaration is `NotFound`, more than one function
(an overload set) is `Overloaded`, a non-function is `NotAFunction`;
the one function is then checked as above. An `Entry` is a small
`Copy` value; any number of entries per session may be resolved and
run, in any order.

Deferred (follow-up rows): parameters (`std::meta::info` or integers
given per run, WP-2070).

## 4. A run

`run(entry, host, budget, cancel)` evaluates a call `f()` of the entry
without arguments, at the end of the translation unit (after its last
declaration, at namespace scope), as one of these:

- **An immediate entry**: the call is an immediate invocation
  ([expr.const.imm]/1: an invocation of an immediate function not in an
  immediate function context, as namespace scope is not), manifestly
  constant-evaluated ([expr.const.defns]/1.4).
- **A `constexpr` entry returning `R`** (not immediate): the
  initialization of a notional variable `constexpr R v = f();` declared
  there. A `constexpr` variable shall be constant-initializable
  ([dcl.constexpr]/6): the full-expression of its initialization is a
  constant expression ([expr.const.init]/4.1), and as the initializer of
  a variable usable in constant expressions it is manifestly
  constant-evaluated ([expr.const.defns]/1.6). For an integral `R` the
  other conditions hold of any value: the result is no invalid pointer
  ([expr.const.const]/4; an indeterminate result fails the run by
  [expr.const.core]/2.12 first), it is constexpr-representable
  ([expr.const.init]/4.2, 4.3) and has constant destruction
  ([expr.const.defns]/3.1). For any other `R` (§4.1) the run
  checks them as the translation unit checks `v`'s: the value is frozen
  by the same Oven code (every subobject initialized, no pointer to an
  object of the run, no immediate function), and the result object of a
  class with a non-trivial destructor is then destroyed by the run (its
  constant destruction, [expr.const.defns]/3.1), whose steps are the
  run's own.
- **A `void` `constexpr` entry** (no variable has type `void`): the
  evaluation of `f()` as the left operand of the comma in
  `static_assert((f(), true));` there, a constant-expression
  ([dcl.pre]/12, contextually converted to `bool`), manifestly
  constant-evaluated by [expr.const.defns]/1.1. The comma's right
  operand adds no step: kiln evaluates `f()` alone.

What follows holds of every kind (a sentence that names one kind is the
only difference):

- **`if consteval` and `std::is_constant_evaluated()`** are true in a
  run, the function the run calls included: the run is manifestly
  constant-evaluated, so a consteval if statement executes its first
  substatement ([stmt.if]/5) and `std::is_constant_evaluated()` returns
  `true` ([meta.const.eval]/1; [expr.const.init] Note 1 says the same of
  the determination of constant-initializability). The Oven's
  `begin_outermost(false, true, true)`: no trial fold, a constant
  context.
- **No injection.** A run is not the evaluation of a
  consteval-block-declaration's expression, so an expression that would
  produce an injected declaration makes it not a constant expression
  ([expr.const.core]/2.32): `std::meta::define_aggregate` fails, and
  `__kiln_plainly_constant_evaluated()` (spec/consteval-blocks.md §3) is
  `false`. For a `constexpr` entry this follows from its form, not from
  a rule on immediate invocations: the notional variable's initializer
  is not a consteval-block-declaration's expression either.
- **The evaluation `E`** of every [expr.const.core]/2 rule is the whole
  run, the same boundary for every kind: a call of a non-`constexpr`
  function (2.3), of an undefined one (2.4), undefined behaviour (2.8),
  a read of an object whose lifetime began outside the run that is not
  usable in constant expressions (2.10), a modification of one (2.18,
  §5), an allocation not deallocated by the end of the run (2.20, 2.22),
  an exception object not destroyed in it (2.24, `RunError::Uncaught`),
  each fails the run with `RunError::Failed`. kiln diagnoses
  runtime-undefined behaviour ([expr.const.core]/3) and library
  undefined behaviour it detects ([expr.const.core]/4) as it does in
  every constant evaluation.
- **A `constexpr` body is checked by the run.** A `constexpr` function
  is checked for constant-expression-ness only where its definition is
  (a function that can never be constant is valid since C++23,
  [dcl.constexpr] Note 4); the run is what decides that this call is a
  constant expression. Nothing is checked or cached before the run.
- **Immediate invocations in the entry's body.** In a `constexpr`
  entry (not immediate) a call of a `consteval` function with constant
  arguments is an immediate invocation ([expr.const.imm]/1) evaluated
  when the body was defined, by the translation unit for a function
  defined in it (never charged to a run, §5.1 "Unchanged"), or, for a
  specialization the run instantiates, as a nested evaluation of the
  `Func`/`Inst` unit that instantiates it (charged to `nested_steps` by
  §5.1, alike on every run); the run reads its value and spends no step
  on it. In an immediate entry the same call is in an immediate
  function context ([expr.const.imm]/1.1), not an immediate invocation,
  and its execution is part of the run's own `steps`. A `consteval`
  call that is not a constant expression in an immediate-escalating
  function makes it immediate ([expr.const.imm]/2.2, 4.2): its run is
  then an immediate invocation.
- **Contract assertions** are evaluated with the translation unit's
  evaluation semantic, as in kiln's evaluation of a `constexpr`
  variable's initializer ([expr.const.init] Note 2): no
  ignore-semantic trial is made first, for any kind.
- **The point of evaluation** ([expr.const.reflect]/5) is the end of
  the translation unit; reflection queries see it, for every kind.

An error reported by semantic work the run needed (an instantiation it
caused, or one replayed, §7) fails the run with `RunError::Failed` even
when the evaluation completed: that error is the run's diagnostic.

**The entry stub.** The Oven compiles one call stub per entry
(`compile_call`: a call of the entry and a return of its value) the
first time the entry runs; it is the same for every kind, does no
semantic work (§7) and is kept (§5). The entry itself is compiled by
the stub's call, as any callee is (§5.1 `Func`).

**A consteval wrapper.** The run of a `constexpr` entry `f` and the
run of `consteval R w() { return f(); }` give the same `result`,
`nested_steps` and `nested_calls`; `w`'s `calls` is one more and its
`steps` exceed `f`'s by the steps of `w`'s own `return` (a constant,
the same for every `f` and every run). A failure has the same notes
but for the headline (§7: `w`'s is an immediate invocation's) and one
more `in call to 'w()'` frame. The call of `f` inside `w` is not an
immediate invocation (`f` is not immediate, and `w`'s body is an
immediate function context anyway), and `f` is the same compiled
function either way. For a class `R` with a non-trivial destructor
(§4.1) `f`'s `steps` also hold the destructor's: `f`'s run
destroys its result (the notional variable's constant destruction),
`w`'s does not (the temporary an immediate invocation returns is
destroyed by the enclosing full-expression's execution, which is no
part of the invocation, [expr.const.imm]/1, and which a run does not
have).

`RunReport`:

| field | meaning |
|---|---|
| `result` | `Ok(RunValue)`: `Void`, `Bool(b)`, `Signed(i128)`, `Unsigned(u128)` (by the return type's signedness), any other value of §4.1; `Err(RunError)`, §8 |
| `constants` | the constants table of a successful run's value (§4.1); empty otherwise |
| `steps` | steps the run's own execution took (spec/oven.md "Limits"), §5 |
| `nested_steps` | steps of nested evaluations charged to the run, fresh and replayed (§5.1; WP-2073) |
| `host_calls` | handler calls the run made |
| `peak_heap` | the run's peak live abstract memory (frame and heap bytes; spec/oven.md `peak_bytes`), charged nested peaks included (§5.1) |
| `calls` | function calls the run's own execution made |
| `nested_calls` | function calls of nested evaluations charged to the run (§5.1) |
| `diagnostics` | the run's diagnostics, §7, in order |

### 4.1 Results (WP-2071)

A run's value crosses to the client as a `RunValue` tree **owned by
the report** (`kiln_sema::session`, re-exported by `kiln_parse`): it
outlives the session, and equal values of two sessions compare equal.
Names are strings (`Arc<str>`): each entity, type and base is spelled
as `std::meta::display_string_of` spells it (spec/reflection.md §3.9,
GCC 16.1's form: a type as `%T`, a declaration as `%D`), a member by
its identifier. A spelling is not an identity: two distinct entities
can print alike (two static locals named `x` in sibling blocks, two
specializations a run instantiated with arguments that print alike).
For identity a node carries **`decl: Option<DeclId>`** (the first
declaration), with one rule: `decl` is `Some` if and only if the
entity existed when the session was opened (its first declaration's id
is below the declaration count `open` recorded), never for an entity
an earlier run or this run created (a specialization instantiated in a
run, a template parameter object made in one), so no result depends on
what earlier runs did; such ids are numbered by history (§7 "Keys and
caches"), so a fresh and a used session would number them apart. No
other id (`QualType`, `ReflectionId`, `ConstValueId`, `Symbol`) is in
the tree.

**The types are a closed set.** No enum of the tree is
`#[non_exhaustive]`: a client matches them exhaustively, so a new
variant is a breaking change by design (the client's build fails and
it decides how to read it). The structs are plain (public fields).

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunValue {                           // 32 bytes (asserted)
    Void,
    Bool(bool),
    Signed(i128),                             // signed integral, `_BitInt(N <= 128)`
    Unsigned(u128),                           // unsigned integral; character types by signedness
    Float { kind: FloatKind, bits: u128 },    // kiln_ast's FloatKind, exact encoding
    Enum(Box<EnumValue>),
    Null,                                     // null pointer, null pointer to member, `nullptr`
    Pointer(Box<PointerValue>),
    MemberPointer(Box<MemberPointerValue>),   // the member and its conversion path
    Info(Box<InfoValue>),
    Text { unit: CharUnit, data: Box<[u8]> }, // an array of a character type
    Array(Box<[RunValue]>),
    Record(Box<RecordValue>),
}
pub struct Entity { pub display: Arc<str>, pub decl: Option<DeclId> }
pub enum IntValue { Signed(i128), Unsigned(u128) }
pub struct EnumValue { pub ty: Entity, pub value: IntValue, pub enumerator: Option<Arc<str>> }
pub struct RecordValue { pub ty: Entity, pub union: bool, pub fields: Box<[Field]>,
                         pub virtual_bases: Box<[Field]> }
pub struct Field { pub kind: FieldKind, pub name: Arc<str>, pub value: RunValue }
pub enum FieldKind { Base, Member, Anonymous }
pub struct PointerValue { pub base: PointerBase, pub path: Box<[PathStep]>, pub past_end: bool }
pub enum PointerBase {
    Variable(Entity), Function(Entity), Temporary(Entity), TypeInfo(Arc<str>),
    Constant(u32),                            // index into the constants table
}
pub enum PathStep { Base(Arc<str>), VirtualBase(Arc<str>), Member(Arc<str>), Anonymous(u32),
                    Index(u64) }
pub struct MemberPointerValue { pub member: Entity, pub derived: bool, pub path: Box<[Entity]> }
pub struct InfoValue { pub kind: InfoKind, pub display: Arc<str>, pub decl: Option<DeclId>, pub value: Option<RunValue> }
pub enum InfoKind { Null, Type, Declaration, Namespace, NamespaceAlias, Template, Value, Object,
                    BaseSpecifier, DataMemberSpec, Annotation, Parameter }
pub enum CharUnit { Char, SignedChar, UnsignedChar, Char8, Char16, Char32, WChar }
pub struct ValueTree { pub root: RunValue, pub constants: Vec<RunValue> }
```

`RunReport::result` is `Ok(root)`; `RunReport::constants` is the
constants table (empty when no pointer reaches an unnamed object).

| type | value |
|---|---|
| `bool`, integral, `_BitInt(N <= 128)` | `Bool`, `Signed`, `Unsigned` by signedness (a bit-field by its declared type) |
| enumeration | `Enum`: the type, the value by the underlying type's signedness (`bool` underlying: `Unsigned`), and the first enumerator in declaration order with that value; `None` when no enumerator has it |
| floating-point | `Float`: the format and its exact bits, so `-0.0` and a NaN's sign and payload survive |
| `std::nullptr_t`, a null pointer or pointer to member | `Null` (the client knows the type it declared) |
| pointer, reference member, `cv void *` | `Pointer` (below) |
| pointer to member | `MemberPointer` (below): the member designated (`%D`) and its conversion path |
| `std::meta::info` | `Info`: its kind (the `Reflectee` kinds), its `display_string_of` text, `decl` for a declaration, namespace, namespace alias, template, or a type that is a class or enumeration (by the rule above), and `value` for a value (`reflect_constant`: its value) or an object (`reflect_object`: a `Pointer` to it) |
| array of a character type | `Text`: every element as a code unit, little-endian at the unit's width (1, 2 or 4 bytes; `wchar_t` the target's); every NUL is kept, nothing is trimmed |
| other array, `_Complex T` | `Array`: the elements in order (`_Complex` as `[real, imag]`) |
| class, union | `Record`: the type; the direct non-virtual bases in declaration order (`FieldKind::Base`, named by type), then the non-static data members in declaration order (`Member`; an anonymous union or struct member is `Anonymous`, its name empty); unnamed bit-fields omitted; empty bases and empty members (`[[no_unique_address]]` or not) present, as `Record`s without fields; so positions map onto the class and never depend on the layout. A most derived object's virtual base subobjects are `virtual_bases` (below; empty for every other record). A union gives its active member only (no field when none is active), `union: true`. A closure type's captures are its unnamed members (`Anonymous`) |

**Virtual bases** (WP-2210). A virtual base subobject is shared by
every base that names it ([class.mi]) and only the most derived
object's constructor initializes it ([class.base.init]/13.1): so a class's
virtual bases are decoded once, at the most derived object (a variable,
a member, an array element, the root, a constant), in
`RecordValue::virtual_bases`, every one of them, direct or indirect, as
`FieldKind::Base` fields named by type, in the order
[class.base.init]/13.1 initializes them (a depth-first left-to-right
traversal of the base classes' graph: for each direct base in
declaration order, that base's virtual bases, then the base itself if it
is virtual; each class once: clang's `CXXRecordDecl::vbases()` order,
the order of the record layout's `vbases`). A base subobject's
`Record` (a `Base` field, or a virtual base itself) has empty
`virtual_bases`: its own virtual bases are the most derived object's.
They are a list apart, not `fields` entries with a flag, so a class's
`fields` are the same for a most derived object and a base subobject of
it: `field(name)` and every `Anonymous(i)` position of a class mean one
thing wherever the class occurs. They are decoded after `fields` (the
order constants are first reached in). The AST dump's `value=` of such
a constant prints no virtual base (clang's `APValue` has no form for
one; a class with virtual bases is not literal in clang 22), so the
tests compare a run with the translation unit's own `constexpr`
variable through `variable_value` and with hand-written trees.

**Pointers.** A `Pointer` gives the complete object it points into and
the designator from it. `Variable` is a variable with static storage
duration, `Function` a function (overloads tell apart by `%D`'s
signature), `Temporary` a lifetime-extended temporary (named by the
variable that extends it), `TypeInfo` a `std::type_info` object (by
its type's spelling). The path's steps are a base (by type), a virtual base of the most
derived object the path has reached (`VirtualBase`, by type: an index
into its `virtual_bases`, as `Base` names a `Base` field), a member
(by name), an unnamed member (`Anonymous(i)`: `i` is the member's
position among its record's bases and members, unnamed bit-fields
omitted: its index in a class's `fields`; a union's `fields` holds
only the active member, so there it counts every member) and an array
element (by index); an
`Index` equal to the array's length is one past its last element
(`&a[N]`). `past_end` is a pointer one past the end of the object the
path designates that is no array element (`&x + 1` for a scalar `x`:
an empty path; `&s.a + 1`: `[Member("a")]`). The designator comes from
the frozen pointer by the designator recovery the constant printer
uses (the base's type, the offset and the pointer's union path,
spec/ast.md §9.1 "Union paths"): at each union the path enters the
member the union path names, else the union's default member, so of
members that share an offset and a type `&u.b` reads as `[Member("b")]`
and `&u.a` as `[Member("a")]`, through nested unions and array elements
alike, as the translation unit's own constants print them. A pointer to the run's result object
itself has no object a host could name: like a pointer to any object
of the run, the freezer refuses it.

**The constants table.** An object that has no name (a string literal
object, a template parameter object, a `define_static_string`,
`define_static_array` or `reflect_constant` object) is `Constant(i)`:
an index into `constants`, whose entry is the object's value (a
string literal's is its `Text`, terminating NUL included). Indices are
given in first-reached order: the root is decoded depth-first, in the
order of its fields and elements; the first pointer to an object gives
it the next index; then the constants are decoded in index order, and
a pointer reached inside constant `i` that names a new object gives it
the next index. The same object reached twice has one index, so two
pointers into one object (a span, a `[begin, end)` pair) show their
identity, not equal content, and a constant shared by others is
decoded once. Objects are told apart as the constant model tells them
(spec/ast.md §9; `PtrBase`): a string literal object is identified by
its contents and the `StringLiteral` expression whose evaluation
created it ([lex.string]/9), so two evaluations of one literal
expression are one object and two equal literals written apart are two
objects (two indices, equal values); a `define_static_*` or
`reflect_constant` object is interned by content (spec/reflection.md),
so equal content is one object and one index; a template parameter
object is one per value and type ([temp.param]/8).

**How a result is read.** A `void` or integral result is read from the
stub's result register (no other work). Any other result object is
frozen by the Oven's freezer (`freeze_object` / `freeze_scalar`: the
code that gives `constexpr R v = f();` its value, so every check of §4
is that code's) into the AST's constant store above a mark
(`ConstStore::mark`); the frozen value is decoded into the tree through
the record layouts (the constant printer's `Shape`) and the store's
images; then the store is truncated back to the mark
(`ConstStore::truncate`: drops the values pushed since the mark and
their index entries; only the freezer runs between the two, so no id
above the mark escapes). Objects the frozen value points to that are
kept (§5) were interned by the run's evaluation, below the mark, and
stay. Then, for a `constexpr` entry, the leak check and the constant
destruction run, as for `constexpr R v = f();` in the translation unit
(`eval_at`). The alternative, a walker that builds the tree from the
Oven's memory directly, was rejected: it would duplicate every check
of the freezer (initialized subobjects, pointer validity, immediate
functions, union members, past-the-end bits) and its notes, and could
drift from what a `constexpr` variable gets.

**Pointers to members** (WP-2212). A `MemberPointer` is the
translation unit's constant (`Scalar::MemberPtr`, clang's
`APValue::MemberPointer`): `member` the member designated (a data
member or member function, `%D`, an anonymous union's member by its
own name); `path` the classes the conversions of [conv.mem]/2 and
[expr.static.cast]/12 went through since `&C::m` named it, each by its
type (an `Entity`), in conversion order, normalized as the evaluator
does (a conversion back to the previous class removes the last entry,
clang's `castBack`); `derived` clang's `isMemberPointerToDerivedMember`:
`false` when the path walks from the member's class towards derived
classes, `true` when it walks towards bases (a member of `D` cast to
`int B::*`). The pointer's class is the path's last entry, the member's
class when the path is empty. So `&B::x` as `int B::*` is `{x, false,
[]}`, the same converted to `int D::*` is `{x, false, [D]}` (two values
with one member, [expr.eq]/4 tells them apart by class), and
`static_cast<int B::*>(&D::y)` is `{y, true, [B]}`. A null pointer to
member is `Null`.

**Not representable, by design.** A reflection gives only its kind,
its text, its `decl` and a value or object, not its structure. A client
that needs more computes it in the transform and returns that.

**What cannot cross is a run error.** A value that is not a constant
expression's (a pointer to an object of the run, heap or frame; an
uninitialized subobject; a pointer or reference to an immediate
function, [expr.const.const]/4 before P4101R1) fails the run with
`RunError::Failed` and the freezer's notes, the notes kiln gives
under `constexpr variable 'v' must be initialized by a constant
expression` (headline by kind, §7). A constant destruction that is not
a constant expression fails it the same way, with the destructor's
notes and call stack.

**Size and its cap.** A tree's size is a function of the tree alone,
never of allocation history: `size_of::<RunValue>()` (32) for the root,
every array element and every constant; `size_of::<Field>()` (64,
its value included) for every field; the `size_of` of every boxed
payload (`Entity`, `EnumValue`, `RecordValue`, `PointerValue`,
`MemberPointerValue`, `InfoValue`), of every path step and of every
member pointer path entry (an `Entity`); each `Text`'s bytes; and each
distinct string once, at its length. The cap is the budget's
**`result_bytes`** (`RunBudget::PROGRAM`: 64 MiB;
`RunBudget::with_result_bytes`), the constants table included: a tree
whose size would exceed it fails the run with
**`RunError::ResultTooLarge`** (note `the result needs more than N
bytes on the host (the run's result budget)`). The read counts as it
builds and checks before each array's allocation (`len` elements are
reserved only when their 32 `len` bytes fit), so the decision is made
on the size and no allocation is ever larger than the cap: an error,
never an allocation failure. The frozen image is transient (its
`sizeof(R)` bytes and relocations, truncated with the store).

**Determinism.** A result is a function of the run (§5): run 1 and run
N, a fresh session and a used one, and every order of entries give
equal trees (`==`, the constants table included), since equal values
freeze to equal images, the decode reads only the image and the
declarations, and `decl` is given only for what existed at open.

**A variable's value.** `EvalSession::variable_value(var: DeclId,
result_bytes) -> Result<ValueTree, ValueError>` (and
`variable_value_named("ns::v", result_bytes)`, looked up as
`entry_named` looks up a function) decodes the constant value of a
variable of the translation unit usable in constant expressions
([expr.const.init]: `constexpr`, a reference, or `const` non-volatile
integral or enumeration with a constant initializer; a reference's
value is a `Pointer` to what it binds) through the same decoder, under
the same rules. `ValueError`: `NotFound`, `NotAVariable`,
`NotAConstant` (not usable in constant expressions, or no constant
initializer), `WrongType` (not a result type, §3), `TooLarge`,
`Unreadable` (a value no tree can give: an address that designates no
object, as a GNU label address), `StackExhausted` (the read's on-demand
work exhausted the calling thread's stack, or reached work an
exhaustion stopped: `RunError::StackExhausted`'s rule, §9; WP-2510). It is
named apart from `std::meta::constant_of`, which it is not. **On
demand** (WP-2420): a variable whose initializer has not been
instantiated or evaluated (a static data member of a class template
specialization whose initializer nothing used: an in-class `constexpr`
one, deferred, or an out-of-line `const` one; a variable template
specialization named only in an unevaluated operand) has it instantiated and evaluated now, as a reference
to the variable in a constant expression would ([temp.inst]/4 and /8:
`note_var_referenced` at the variable's own location, with what that
instantiates in turn, a function template its initializer calls
included), under the translation unit's limits, without a handler, and
on the calling thread's stack (§9). That work is the translation
unit's (§5.1 "Unchanged"), kept, and so not read-only on the AST. Its
units (§5.1, `Init(v)`: an in-class initializer, an out-of-line
definition instantiated as a redeclaration, WP-2451, and a variable
template specialization's initializer, keyed by the variable's first
declaration) are recorded as a run's are, so a later run
that reaches one that failed reports its diagnostics exactly as in a
session where the run did the work (§7); a reference to the definition
such a unit left invalid reaches it too (`build_decl_ref_extra`, where
the reference is quiet as in a translation unit). The run's own error
after the replayed ones may differ from a run that did the work: a
reference made before the definition failed names the in-class
declaration, one made after names the invalid definition (a residual
of the translation unit's own order, §5.1 "Residuals"). No run is open to measure it,
so a later run that reaches it is not charged it, as for work before
the first run. Its diagnostics are captured and dropped (the answer is
`NotAConstant`): none reaches the engine's sink. A fatal error it
reports (a stack exhaustion) ends with the read, as one in a run ends
with the run (§9). Its answer is the same before and after any runs:
the value of an initializer is a function of the translation unit
(§5.1 rule 1). The tests declare `constexpr R v = f();` beside an
entry and check `run(f)` against `variable_value(v)`, on run 1 and run
N, fresh and used, and `variable_value(v)` before and after runs; each
kind also has a hand-written expected tree (both sides share the
decoder). Tests (WP-2420): `crates/kiln_parse/tests/eval_session_results.rs`
`variable_values_instantiate_on_demand` (in-class, out-of-line, class
type, an initializer calling an uninstantiated function template, a
non-constant one; read before, after and between runs) and
`a_failed_on_demand_read_fails_later_runs_alike`; WP-2451 and WP-2452:
`a_failed_on_demand_definition_fails_later_runs_alike` (an out-of-line
definition and a variable template specialization whose instantiation
fails, read first or reached by another entry's run first) and
`a_variable_template_specialization_is_read_on_demand`.

**Kept state.** Reading interns nothing and keeps nothing: the frozen
values are truncated, the name table is local to the read, and
spelling a name creates no type and interns no symbol. `kept()` after
run 1000 of a class-returning entry equals `kept()` after run 2.

## 5. Isolation and what a session keeps

Every run starts from the post-translation-unit state. **Reset at the
end of every run** (nothing of it reaches a later run): the frame and
heap spaces and every object in them (transient allocations, the
`std::vector`s a run builds), the call stack, cleanups, exceptions in
flight or caught, the run's counters and impurity marks, the host-call
buffers, the diagnostics deduplication set and counts (§7) and the
state of any union or `mutable` member of a run's object. A run can
modify no object whose lifetime began outside it ([expr.const.core]/2.18),
so the static objects of the translation unit (the values of
`constexpr` variables) are never changed by a run.

**Kept** (deterministic caches: a run reads them, and gets the same
answer whether an earlier run or this one filled them):
- template instantiations and the other semantic work a run triggers
  (an implicit member defined on demand, a class completed, a
  reflection query answered): the run's point of instantiation is the
  end of the translation unit ([temp.point]/8), so its instantiations
  are those any later point would make. Their diagnostics are
  replayed (§7);
- compiled bytecode, the run's entry stub, resolved globals and the
  static objects that hold `constexpr` variables' values, interned
  string-literal objects;
- the memoization cache (spec/oven.md "Memoization"): bounded LRU of
  pure calls' scalar results. A hit charges exactly what the call
  cost (steps, calls, bytes, depth), so whether an earlier run warmed
  it changes no counter and no budget decision (a hit is not taken
  where it would cross a pending step check, §5.1). `clear_memo()` drops
  every entry and returns how many it dropped;
- not the result: a run's result is frozen above a mark and the store
  truncated back after the read (§4.1), so reading it keeps nothing
  (`variable_value`'s on-demand evaluation is the translation unit's
  work, §4.1);
- values interned in the AST by content (`std::meta::reflect_constant`
  template parameter objects, `define_static_string`/`define_static_array`
  objects, reflection handles): equal content gives the same entity in
  every run, so they carry no information between runs beyond what a
  run computes itself. They are the only kept state whose size can grow
  with the data runs see (`kept()` reports it).

**Accounting is a function of the run.** `steps` counts the run's own
execution; nested work is charged by §5.1. A run's steps, nested steps
and calls, and whether it exhausts a budget, are the same on run 1 and
run 1000 and on a fresh or a used session, whatever other entries ran
before and in whatever order (the residuals, kept work that is not a
unit, are listed in §5.1 "Residuals").

### 5.1 Nested work (WP-2073)

**Terms.** A *nested evaluation* is an evaluation context opened while
a run is in progress by semantic work, not by the run's own code: every
context `Oven::eval_nested` opens (`kiln_oven/src/lib.rs`), and the
Oven's own initialization of a variable it reads (`eval_var_init`,
`kiln_oven/src/value.rs`). A *unit* is a piece of kept semantic work
that is done once per session and may open nested evaluations. The
units (`Key` in `kiln_sema/src/session.rs`):

| unit key | the work | reached at |
|---|---|---|
| `Func(f)` | the Oven's compilation of function `f`, with the semantic work it needs: an instantiation, an implicit or defaulted member, a metafunction thunk (`SemaBridge::ensure_body`) | every call of `f` in a run (`Oven::session_compile`) |
| `Var(g)` | the Oven's evaluation of global `g`'s initializer for a read (`eval_var_init`) | every first read of `g` in a run (`Oven::session_kept_var`) |
| `Init(v)` | a variable's initializer instantiated and checked (`instantiate_var_spec_init`, `subst_static_member_init`) | each reference to `v` that may need its value (`note_var_reference`) |
| `Inst(f)` | a function template specialization's definition instantiated (`instantiate_function_definition`: a member function of a class template specialization, and a function template's own specialization, `instantiate_function_spec_definition`, a unit since WP-2501: before, its work was only its parent unit's, so a run that called it after another run instantiated it replayed nothing) | each request to instantiate it, done or not (its early return, `note_used_for_instantiation`), and each compilation of it that finds its body instantiated (`SemaBridge::ensure_body`, WP-2072) |
| `Meta(op, operands)` | a reflection query (`SemaBridge::meta`) | each query with the same op and operands |
| `Nothrow(f)` | whether `f` is non-throwing, which may instantiate an exception specification (`SemaBridge::is_nothrow`) | each query, the early `known_nothrow` answer included |
| `Class(c)` (WP-2075) | the definition of class template specialization `c` instantiated ([temp.inst]/2-3: its member declarations, `static_assert`s, bit-field widths, alignments, nested types), or of a member class or member enumeration `c` of one ([temp.inst]/3: an enumeration's enumerator values); `instantiate_class_as`, `instantiate_member_class`, `instantiate_member_enum`. Its member function definitions are `Inst`, its static data members' initializers `Init`, its default member initializers `Nsdmi` | each request that instantiates `c` when it is undefined and finds it defined: the `require_complete` family (`undefined_spec_at`: `require_complete`, `require_complete_silent`, `instantiation_fails`, `require_complete_nns`; `instantiate_for_init`, `require_member_enum`), a class named in a nested-name-specifier, `instantiate_class_as`'s early return (base matching in deduction, explicit instantiation), every class associated with an argument-dependent lookup, and a reflection query that completes a type (`meta_undefined_spec`) |
| `Exc(f)` (WP-2075) | the exception specification of `f` (its first declaration): a `noexcept(e)` or dependent dynamic specification instantiated ([except.spec]/13; `substitute_exception_spec`), or the implicit one of a special member, inheriting constructor or defaulted comparison computed from the functions it calls ([except.spec]/7-8, 10; `resolve_deferred_exc`) | each resolution that finds it resolved (`resolve_exc`, `instantiate_exception_spec`'s early return): the `noexcept` operator, a use ([except.spec]/13), a conversion to a pointer to a non-throwing function, an overriding check, a trait, and the Oven's `is_nothrow` with its early `known_nothrow` answer |
| `Sat(t, args)` (WP-2075) | a satisfaction check cached by the satisfaction cache (`sat_cache`): a concept-id `t<args>` (`check_concept`) or the associated constraints of template `t` with one level of arguments (`check_constraints`), `args` canonical; its atoms are children | each check with the same key that hits the cache |
| `Atom(e, args)` (WP-2075) | the satisfaction of atomic constraint `e` with the canonical arguments of each level (the atom cache's key, `atom_cache`: two atoms are identical when they are the same expression with the same mapping, [temp.constr.atomic]/2): its substitution and evaluation ([temp.constr.atomic]/3) | each check with the same key that hits the cache, from any constrained template, concept or requires-expression |
| `Nsdmi(m)` (WP-2075) | the default member initializer of field `m` of a class template specialization instantiated ([temp.inst]/3, at its first use by a constructor, [class.base.init]/9.1; `instantiate_nsdmi`) | each use that finds it instantiated: `default_member_init` (a constructor's mem-initializers, user-provided or defaulted) and a defaulted default constructor's exception specification |

A unit is *reached* each time a run, or a unit's work in it, asks for
it: the call that does it, and every later request that finds it done.

**Rule 1: atomic under the translation unit's limits.** A nested
evaluation runs to completion under the translation unit's
constant-expression limits (`-fconstexpr-steps=` and friends, fixed for
the session), counted from zero, never cancelled, never under the
run's budget, and with no host handler (§6). Its outcome, and so the
kept work, is a function of the translation unit alone. The reason is
that semantic work cannot be rolled back: what a nested evaluation
computes flows into kept state (an `if constexpr` branch instantiated,
a template argument naming a specialization, a constraint choosing an
overload, a `constexpr` variable's value), and kiln has no transaction
over semantic work. Stopping one part-way by the run's budget or token
would keep a failure that a later run with a larger budget would
see; so nested work is never budget- or cancel-dependent. Nested work
that exceeds the translation unit's own limits fails as it would in the
translation unit: that failure is kept, and every run that reaches it,
on a fresh or a used session, fails the same way (`RunError::Failed`
with the same diagnostics, §7).

**Rule 2: recorded once.** While a unit's work runs (in whichever run
first reaches it), the session records with it, beside its diagnostics
(§7): its *own cost*, the cost of the nested evaluations it opened
itself (`NestedCost`: steps and calls summed; peak live bytes, peak
address space and call depth, each counted from zero by rule 1, as
maxima), and its *children*, every recorded unit reached during its
work, in order. A child reached already done is a child all the same.
A unit that opened nothing, reached no recorded unit and has no
diagnostics has no record and costs nothing to track.

**Rule 3: charged at the reach, once per run.** The first time a run
reaches a recorded unit from its own code, it is charged `cost(u)`:
`u`'s own cost plus `cost(c)` of each child `c` not yet charged in
this run, recursively (steps and calls summed, the rest as maxima).
Fresh work is charged the same way, from its record, when its work
ends. A reach inside a unit's work is only recorded (as a child); its
charge is part of the enclosing unit's. Steps and calls go to
`nested_steps` and `nested_calls`. The run's **budget decision is made
on its own cost plus its charged nested cost**, alike for a run that
does the work and one that replays it:
- `steps + nested_steps` above `budget.steps` fails the run with
  `StepBudget` (the run's own steps may use what its nested work
  left);
- the address space in use at the reach plus the charged peak address
  space above `budget.bytes` fails it with `MemoryBudget`; the live
  bytes at the reach plus the charged peak live bytes enter
  `peak_heap`;
- the active calls at the reach plus the charged depth above
  `budget.depth` fails it with `DepthLimit`.

The failure is reported at the run's next step check (the next
statement executed), with the step-limit, memory-limit or depth note
and the call stack of that point; a charge after the run's last step
check fails the run at its end, with the note at the entry. A
memoized call is not taken where it would cross a pending step check
(`c.steps + v.steps > step_stop` in a session), so a run charged over
its budget stops at the same step whether or not an earlier run warmed
the memo.

**Why it is deterministic.** The set of units a run reaches from its
own code is a function of the run's execution; each unit's record is
a function of the translation unit (rule 1); a unit's children are
what its work asks for, which does not depend on what was done before
(a child done earlier is still reached); and charging a set of units
charges each once, so neither order nor history changes the sum.
Checked by `crates/kiln_parse/tests/eval_session_nested.rs`, where a
fresh session per run is the reference:
- **Run 1 and run N, fresh and used, A then B and B then A**, with
  nested work inside nested work (a static member's initializer whose
  evaluation instantiates a function that reads another static
  member) shared by two entries, memoized calls and a diagnostic:
  steps, `nested_steps`, calls, `nested_calls`, the result and the
  diagnostics are identical.
- **A budget the nested work exceeds.** The work completes under rule
  1 and is kept; the run is charged it and fails with `StepBudget`. A
  sequence of runs with that budget, then a budget one step short of
  the total, then the exact total, gives the fresh session's report
  each time; nothing is charged part-way, nothing is kept half-done,
  nothing is redone.
- **Depth and bytes**: nested work's depth and address space count on
  top of the run's, alike fresh and used.
- **The memo.** A function whose compilation was recorded stays a stub
  while the session is open, so each of its calls goes through the
  compile path, which reaches the record and makes every call in
  progress impure: no memoized result skips a charge or a replay.

**Reach observation.**
- `Func`: a recorded function's real state is kept aside
  (`SessionState::tracked`); every call reaches it and bumps
  `run.impure`, and the first call in a run outside nested work, or
  any call inside it, reports the reach.
- `Var`: a global whose initializer's evaluation was recorded keeps its
  object and is made unresolved again at the end of every run, so the
  next run's first read reaches the record.
- The sema units: the hooks test one `Option` (`Sema::eval_replay`,
  set only inside a run).

**Hooked by WP-2075.** Before WP-2075 the last five units were not
units: their work, its nested evaluations and its diagnostics belonged
to the unit whose work first did them, so a later unit (in the same or a
later run) that needed the work done was charged neither its nested cost
nor shown its diagnostics. Each is now recorded by its own window
(`Sema::sess_unit_begin`, opened after the instantiation frame it pushes,
or `sess_unit_begin_frameless` for an `Atom` and a computed `Exc`, which
push none: a reach replays those in the reach's context) and reached by
`Sema::sess_reach_at` wherever a request finds the work done. Every
hook is behind `Sema::eval_replay` (one `Option` test, no allocation,
without a run; an `Atom` key's argument list is copied only in a run).
The keys are content (§7): a `DeclId`, the atom's `ExprId`, and interned
canonical argument lists (`ArgListId`, a bijection with their content
for the session). A cached satisfaction keeps a failure (an atom whose
evaluation exceeds the translation unit's step limit is not a constant
expression, `sema_non_constant_constraint`): the `Sat` record keeps the
error, so every run that reaches the check, through any unit, fails the
same way. Not units, still: a class template specialization's partial
specialization selection that fails (ambiguous: the class is made
invalid before its frame is pushed, and a later request finds it invalid
without the error, as in the translation unit itself); a satisfaction
check with more than one level of arguments (not cached: redone, and so
charged, at each check; its atoms are cached and are units).

**Residuals.** Other kept work that is not a unit (a default argument's
instantiation, an alias substitution's cache, a reflection table entry)
is charged only to the unit that first did it (WP-2076). So is the
reflection-bytes charge (spec/oven.md "Limits": a value already
interned charges 0), which makes a run's peak bytes depend on what
earlier runs interned (WP-2077). The nested limits are the translation
unit's; a session-level setting for them is WP-2078. A reference made
after an out-of-line static data member definition's instantiation
failed (and left the definition invalid) is quiet where the reference
that did the work was not (it named the in-class declaration and
reported the read of its unknown initializer): the unit's diagnostics
are replayed alike, the run's own failure notes differ (§4.1, WP-2451).

**Unchanged.** The translation unit's own nested evaluations, before
any run, run under its limits and its handler, are not units and are
never charged to a run. Without a session nothing here runs: the sema
hooks test one `Option`, the Oven's step check and compile path are
as before, and bench/oven counters stay identical.

## 6. Host calls

A run is not speculative: host calls are allowed (spec/host-calls.md
§4 "Runs"). The handler is **given per run**, borrowed (`&mut dyn
HostCalls`, any lifetime, no `Send` bound): the Oven reaches it through
the run's bridge (`SemaBridge::host_calls`), not through a registered
handler, and nothing keeps it after `run` returns. Nested
evaluations of semantic work the run triggers (§5.1) have no handler:
a host call there is refused, as what they compute is kept across runs
and must not depend on one run's handler. The translation unit's
handler (given to the parse) is not used by runs. A refusal in a SFINAE
context (a requires-expression a run's instantiation evaluates, a
deduction) is a hard error, never a substitution failure, in the
translation unit and in a run's nested work alike (spec/host-calls.md
§4, WP-2501).

The refusal's note says it in the author's terms (WP-2325): `a host
call cannot be evaluated here: this expression is evaluated when the
code is instantiated, not when it runs`, under the error of the
evaluation that made the call (`constexpr variable 'v' must be
initialized by a constant expression` for a static member's
initializer), whose context notes are the ones the diagnostic engine
gives any instantiation (`in instantiation of static data member
'H<int>::v' requested here`, `in instantiation of member function ...`;
on replay, those of the reach, §7), and above the call stack (`in call
to 'hc(1)'`). The refused call bumps `run.impure` as every refused call
does (spec/host-calls.md §5), so no call around it is memoized.

**Requirement: every run that reaches the work fails identically.** The
refusal is a structured failure of the unit (§5.1 rule 1), kept and
replayed like any other diagnostic (§7). Every run that reaches the
work, from the first that triggers it to any later one, on a fresh or a
used session alike, returns the same `RunError` (`Failed`) and the same
diagnostics: records equal field by field, so their rendering (§7) is
the same text. A static member's initializer that makes a host call
reports its error with that note once, records it, and every run that
reaches the initializer replays it. The run's read of the variable
re-folds the failed initializer without reporting the call again (no
second `host call during speculative evaluation`, spec/host-calls.md
§5, WP-2330): the run has the initializer's error and the entry's,
two errors. Test:
`crates/kiln_parse/tests/eval_session_nested.rs`
`every_run_reaching_a_refused_host_call_fails_identically` (five runs,
the instantiation first reached in run 1, rendered by the public
renderer) and `a_host_call_in_nested_work_fails_every_run_alike` (fresh
and used sessions, two entries in both orders). A client that needs the handler
calls it from the run's own code (the entry's body and what it calls),
never from a `static_assert`, a `constexpr` variable or a template
argument of an instantiation.

Not chosen: re-doing such work with the run's handler in every run.
The value of a nested evaluation flows into kept state that cannot be
re-derived per run (§5.1 rule 1); kiln has no transaction over semantic
work (the Oven's trials undo only abstract memory, spec/oven.md "Trial
evaluation").

## 7. Diagnostics

A run's diagnostics are returned in `RunReport::diagnostics` as
`kiln_diag::Diagnostic` records (the same records a sink receives:
id, level, message, span, notes), never printed.

**Rendering (WP-2324).** `SessionFrontEnd::render(&records)` renders
records against the session's source manager exactly as the driver
renders diagnostics on standard error for the command line, in its
format (text, or JSON under `-fdiagnostics-format=json`), every note
and call-stack frame under its diagnostic. It is
`kiln_diag::render_diagnostics(sm, &diag_options, records)`, the one
rendering path: `kiln_diag::format_sink(out, opts, prefix)` is the sink
every driver engine renders with (`make_engine`, the library entry's
engines, `DiagEngine::with_stderr`), and `render_diagnostics` sends the
records through one such sink without a prefix, so an include stack is
printed once per run of diagnostics as the driver prints it. The
summary line (`1 error generated.`) is the driver's, not a record's. A
client of `parse_preprocessed_retained` calls `render_diagnostics` with
its own engine's options. Test: `crates/kiln_driver/tests/eval_session.rs`
`run_diagnostics_render_as_the_driver` (the translation unit's warning
in a header with its note, text and JSON, equal to `kiln++
-fsyntax-only`'s standard error; a run's failure with its call stack);
kiln's session tests render with it (`crates/kiln_parse/tests/eval_session.rs`,
`eval_session_nested.rs`). While a run is in
progress the session's `DiagEngine` captures every diagnostic instead
of sending it to its sink (`DiagEngine::capture_begin` /
`capture_end`): the error and warning counts and the fatal flag are
restored at the end of the run, and the run deduplicates only within
itself, so a run never sees the error limit, the counts or the
duplicates of another run or of the translation unit.

- **A failed run** reports, at the entry's declaration, the
  evaluation's notes: the reason, then the call stack (`in call to
  'g()'`, innermost first, under `-fconstexpr-backtrace-limit=`; the
  outermost frame, the entry's, is at its declaration, as a run has no
  call expression). The headline is by kind:
  - an immediate entry's, as for any failed immediate invocation:
    `sema_immediate_call_non_constant` ("call to consteval function 'f'
    is not a constant expression"; "call to immediate function 'f<int>'"
    for an escalated one, followed by the note saying why it is
    immediate, as clang's `EvaluateAndDiagnoseImmediateInvocation`);
  - a `constexpr` entry's (WP-2072): `sema_session_entry_non_constant`
    ("call to constexpr function 'f' is not a constant expression"),
    the run's wording for any entry, with the notes clang 22 and 19
    give under "constexpr variable 'v' must be initialized by a constant
    expression" for `constexpr int v = f();` (the reason, the call
    stack, `declared here` for a non-`constexpr` callee, `allocation
    performed here was not deallocated` for a leak). The notional
    variable has no name a client could use, so the headline names the
    entry.
  - **An uncaught exception** (WP-2413; `RunError::Uncaught`): the
    reason is clang's P3068 note at the throw (`unhandled exception of
    type 'E' with content {...} thrown from here`, or `unhandled
    exception: <what()>`), followed in a run by the call stack the
    exception left the entry from, as for any other failure: the frames
    active at the throw-expression, innermost first (for a rethrow,
    `throw;`, the frames active at the rethrow: the original throw's
    were left when its handler was entered), under the same
    `-fconstexpr-backtrace-limit=` elision, the entry's frame last at
    its declaration. clang gives no call stack with this note, so only
    a run's own evaluation adds it (`Oven::session_run_outermost`):
    an uncaught exception in the translation unit's evaluations, and in
    a run's nested work (§5.1, the translation unit's work, replayed
    alike in every run), keeps clang's shape. Test:
    `crates/kiln_driver/tests/eval_session.rs`
    `an_uncaught_exception_has_its_call_stack` (rendered with
    `SessionFrontEnd::render`: a shallow stack, one over the limit, a
    rethrow, and the same throw in the unit's own `constexpr` variable).
- **Replay.** Semantic work is kept (§5), but a fresh session would
  report its diagnostics in the first run that needs it. The session
  records the diagnostics emitted during each unit's work (§5.1), in
  the same tree as its cost: a unit's segments are its own diagnostics
  and its children, in order. They are replayed, in place, the first
  time a run reaches the unit, from its own code or from another
  unit's work; a unit reached again in the same run replays nothing.
  Sema holds the run's recording for the run's duration
  (`Sema::eval_replay`), so the bridges of semantic work done by a
  nested evaluation inside other semantic work record into it too: work
  nested inside nested work is a unit of its own, replayed wherever it
  is reached. A replayed error fails the run exactly as the original
  did (an invalid instantiation makes the call fail).
- **Context notes.** A diagnostic recorded inside an instantiation
  carries that instantiation's context notes (`in instantiation of
  ...`, `required from here`). On replay the recorded context is
  replaced by the context of the reach: what a fresh session would
  print at the point the run reached the unit. Recording does not use
  up the context notes of the diagnostics that follow.
- **Limits.** A reflection query's record is keyed by the query
  (below); the units its work reaches are keyed by unit, so a different
  query, or a run's own code, that needs the same already-done unit
  replays it: a class a query completed (`members_of`, `size_of`, a
  type trait) is a `Class` unit (WP-2075), reached by a later query that
  completes it (`meta_undefined_spec`) or by any other request for it.
  Work of the kinds that are still not units (§5.1 "Residuals", WP-2076)
  is replayed with the first unit or query that did it only (WP-2160). A replayed diagnostic has the location of its recording;
  only its context notes are those of the reach.
- **Keys are content.** No replay key, memoization key or kept result
  depends on an address or on what earlier runs did. A reflection
  query is keyed by its op, the context it reads (whether it is a
  trial and whether it runs inside semantic work; for the ops that
  read them only, the evaluation point's scope, the nested evaluation
  point, whether the evaluation is plainly constant-evaluated) and each
  operand as the intrinsic reads it, read through the same Oven
  accessors (`crates/kiln_sema/src/meta/key.rs`; every operand of
  every op has a row there, checked by a unit test): an `info`, an
  integer or an `access_context` by its bits; a range of `info`
  (`substitute`'s arguments, a trait's types, `define_aggregate`'s
  descriptions) by its length and elements; a `(const info *, n)`
  trait form by its `n` elements; `reflect_constant_string` /
  `reflect_constant_array`'s range by its element type, count and
  elements frozen as the op freezes them (a content-interned constant);
  `reflect_constant`'s operand frozen at its type; `reflect_object` /
  `reflect_function`'s address frozen as a pointer (the entity and
  offset, not the encoding); `is_string_literal`'s by its answer;
  `data_member_spec`'s options by what is read from them (name bytes,
  alignment, width, `no_unique_address`, annotations); a name by its
  bytes; a value op's specialization types. An operand the op rejects
  by its type alone keys as rejected; a read that fails ends the
  operand (the op stopped reading there too). The encoding is exact
  (lengths before elements, the whole key compared), so equal content
  hits the same record from any address and different content never
  does. The key is computed after the call and only when needed: the
  call reported something, did nested work or reached a recorded unit
  (§5.1), or some call of the same op has a record. A query whose key
  has a record and that records nothing new replays and is charged
  that record (`Replay::end`), as any unit reached again. What
  computing a key interns (a value the op did not freeze, as it
  failed first) is not charged to the run (`Oven::meta_uncharged`).
- **Found and fixed by WP-2073** (each with a test in
  `crates/kiln_parse/tests/eval_session_nested.rs`): a call memoized
  after a recorded function was reached let a later run skip its
  replay (the function now stays a stub, §5.1); work shared by two
  units was replayed with the first only (`Init` and `Inst` units); an
  initializer read under a small run budget kept its failure for every
  later run (`eval_var_init` now runs under rule 1). An exception
  specification resolved by one run was answered early
  (`known_nothrow`) in the next without its replay: not reproduced
  (sema resolves the specification at the use, inside the unit that
  compiles the caller, so the query finds no recorded work; the tests
  pin the behaviour for a direct call, a member pointer and a virtual
  call), fixed all the same: every reach of a recorded unit must be
  seen, and the early answer now reaches a recorded `Nothrow` unit.
  The entry stub compiled before the run is armed does no semantic
  work (`compile_call` only resolves the callee's index): not a bug.
- **Found and fixed by WP-2072** (test
  `a_function_instantiated_by_other_work_is_reached_by_its_compile`):
  a member function whose definition another unit's work instantiated
  (`c`'s instantiation instantiates `a`, which `c` calls) was compiled
  later in the same run without a reach of its `Inst` unit, so it was
  not recorded and a later run calling it first was charged nothing and
  replayed nothing. The Oven's request for a body that exists
  (`SemaBridge::ensure_body`) now reaches the specialization's recorded
  `Inst` unit, as a request to instantiate it does.

**Keys and caches a run reads or fills** (WP-2074). "Address" is a
frame or heap encoding (slot | generation | offset), which `reset`
reuses in every run; a static encoding names one object for the
session (static slots are reused only after a scratch object is freed,
with the next generation). Ids interned on first sight (`ReflectionId`,
`ConstValueId`, an Oven function index) are numbered by history but
are a bijection with their content for the session's lifetime, so
they key exactly.

| key or cache | site | key holds | why it is exact |
|---|---|---|---|
| replay `Key::Meta` | `kiln_sema` `session.rs` `Replay::records`; `oven_bridge.rs` `meta` | content (above) | fixed by WP-2074; tests `run_built_operands_replay_by_content`, `a_run_does_not_depend_on_the_runs_before_it` |
| replay `Key::Func` | `session.rs`; `kiln_oven` `session.rs` `tracked` | Oven function index | one index per function for the session |
| replay `Key::Var` | `kiln_oven` `session.rs` `kept_vars`, `value.rs` `eval_var_init` | Oven global index | one index per global for the session |
| replay `Key::Init`, `Key::Inst` | `kiln_sema` `templ/class.rs`, `templ/var.rs` | `DeclId` | no address |
| replay `Key::Nothrow` | `oven_bridge.rs` `is_nothrow` | `DeclId` | no address |
| replay `Key::Class`, `Key::Exc`, `Key::Nsdmi` | `kiln_sema` `templ/class.rs`, `except.rs`, `class/special/excspec.rs` | first `DeclId` (the field for `Nsdmi`) | no address |
| replay `Key::Sat`, `Key::Atom` | `kiln_sema` `templ/concepts/sat.rs` | `DeclId` or the atom's `ExprId`, canonical `ArgListId`s | the satisfaction caches' own keys; argument lists interned by content |
| memoization | `kiln_oven` `memo.rs` `MemoKey`; eligibility `compile.rs` (`memo =`) | function index, flags, up to 4 scalar arguments | only integer, `bool`, floating and `info` parameters, no `this`, no pointer result: a pointer or reference argument (an address) makes the function not memoizable; an `info` of an automatic variable is a handle to the running evaluation's frame, so `extract` and `constant_of` of one mark the calls in progress impure (fixed by WP-2074: a call given `^^x` was memoized by its bits and read a stale `x`); otherwise a memoized function reaches only static objects; tests `memoized_results_are_not_keyed_by_addresses`, `reflections_of_locals_are_not_memoized`, `tests/conformance/meta.reflection.extract/local-reflection-argument-memo.cpp` |
| `substitute` memo | `kiln_sema` `meta/templ.rs` `SubstMemo` | template and argument reflection bits | the arguments are read from the range before the lookup |
| member lists | `kiln_oven` `meta/member_cache.rs` `MemberCache` | `DeclId` and list kind | no address |
| union path memo | `kiln_oven` `vm/umemo.rs`, `Memory::union_epoch` | allocation key (slot, generation) | an entry is trusted only at the epoch it was computed at; every change to the union state bumps the epoch, and `reset` bumps it when it drops a run's union entries, so an entry a later run can hit was computed with no union state of that object, a function of the types and offset (also in the key) |
| union designators | `vm/udesig.rs` | (designator, type) interned | content |
| address/integer cells | `kiln_oven` `addr_int_disps`, `label_diff_cells`, `synth` | (space, address) | written only when folding C static initializers (`set_fold_c_static`, codegen), never in a session |
| thawed constants, literal objects, globals | `thawed`, `lit_exprs`, `lets`, `unknowns`, `provisional_objs`, `global_map` | `ConstValueId`, `ExprId`, `DeclId`, global index | values are static addresses, valid for the session |
| thunks, compiled code | `thunks`, `thunks_at`, `func_map`, `ctor_steps` | `ExprId`, `DeclId` | no address |
| member pointers, virtual calls | `mp`, `virt` | interned members, class and function types | content |
| data member descriptions | `kiln_oven` `meta/define.rs` `DmsTable` | the description (`Dms`), annotation lists | content |
| host-call taint | `kiln_sema` `consteval.rs` `host_failed` | `ExprId` | filled only when a handler call is made outside a run; nested evaluations of runs have no handler |
| entry stubs | `kiln_oven` `session.rs` `stubs` | `DeclId` | no address |
| reflection charge | `kiln_oven` `meta/intern.rs` `charge_refl` | interned growth | not a key: what an intrinsic call is charged depends on whether its result was already interned (spec/oven.md "Reflection bytes") |

## 8. Budgets, cancellation, errors

`RunBudget { steps, bytes, depth }`, given per run. Its `Default`
(`RunBudget::PROGRAM`) is the *program profile*: 2^34 steps, 1 GiB of
abstract memory, 16384 nested calls, against the constant-expression
defaults of 2^20 steps, 256 MiB and 512 calls. `RunBudget::with_steps`
etc. adjust one limit. A fourth field (WP-2071), `result_bytes`, is
the host bytes a result's tree may take (§4.1; `PROGRAM`: 64 MiB); a
client that builds the struct literally writes `..RunBudget::PROGRAM`
for the fields it does not set, so later fields are added the same
way.

`CancelToken` (`Clone + Send + Sync`, an `Arc<AtomicBool>`): `cancel()`
from any thread stops a run that is given the token; `is_cancelled()`;
`reset()` makes it usable again (a cancelled token stays cancelled:
a run given one fails at its first check). The run checks the token
at least every 65536 steps (`CANCEL_INTERVAL`) of execution, so at
every loop back-edge and call within that many steps, and before each
host call: a `for (;;) {}` stops within a fraction of a millisecond.
The check rides on the step-limit check (spec/oven.md "Dispatch"), so
an evaluation without a session runs exactly the code it ran before.

**Nested work (WP-2073).** A nested evaluation is never stopped by
the token or the run's budget (§5.1 rule 1): semantic work cannot be
rolled back, so stopping it would leave its unit half-done and keep its
failure for every later run. Budget and cancellation take effect at
the run's next step check after the nested work returns. **The
cancellation latency** is therefore bounded by the remaining nested
evaluations of the unit in progress, each capped by the translation
unit's step limit (`-fconstexpr-steps=`): a client that cancels a run
inside a large instantiation waits for that instantiation's
evaluations, never for more than their count times that limit. A run
whose charge exceeds a limit fails with `StepBudget`, `MemoryBudget` or
`DepthLimit` (§5.1 rule 3); one cancelled during nested work fails with
`Cancelled`, and the work stays done exactly as a fresh session would
have done it. The Oven's own initialization of a variable it reads
(`eval_var_init`) is a nested evaluation too and follows the same
rule.

`RunError` (`kiln_sema::session`, re-exported by `kiln_parse`;
`#[non_exhaustive]`), one kind per way a run ends:

| kind | when |
|---|---|
| `Failed` | not a constant expression (undefined behaviour, a call of a non-`constexpr` function, a leak, ...) |
| `StepBudget` | `budget.steps` exceeded |
| `MemoryBudget` | `budget.bytes` exceeded: an error, never an abort |
| `DepthLimit` | `budget.depth` exceeded |
| `Cancelled` | the token was cancelled; note `evaluation was cancelled` |
| `Host { message }` | the handler's own error: it returned `Err` (`message` is its message; note `host call failed: <message>`), or it wrote more results than a capacity (spec/host-calls.md §2.4; `message` is the note, `host call handler wrote N integers into a capacity of M`) |
| `HostArgument { message }` | (WP-2480) kiln refused a host call before its handler ran: a check of its arguments failed (spec/host-calls.md §2.2: a null or short range, an input element not initialized or outside its lifetime, an output that is not writable; `message` is the note, `'<arg>' of __builtin_kiln_host_call <reason>`). The handler was not called |
| `Uncaught { ty, what, scalars, value }` | an exception left the entry: its type (`ty`, below), its `what()` when it derives from `std::exception`, `scalars` (WP-2324, below) and its value (WP-2502, below) |
| `StackExhausted` | (WP-2500) the run's semantic work exhausted the calling thread's native stack: the fatal `sema_stack_exhausted` is among its diagnostics (§9) |
| `ResultTooLarge` | (WP-2071) the result's tree would exceed `budget.result_bytes` (§4.1); note `the result needs more than N bytes on the host (the run's result budget)` |

Every kind comes with its diagnostic (§7). After any of them the
session is usable and the next run behaves as on a fresh session
(but for what §9 "What it stopped stays stopped" keeps).

**Whose failure (WP-2503).** Of the kinds, only `Host` is the host
handler's own error; `RunError::is_host()` is true for it alone. Every
other kind is kiln's failure of the run: the program (`Failed`,
`Uncaught`, `HostArgument`: the program passed the arguments), a limit
the client set or kiln has (`StepBudget`, `MemoryBudget`, `DepthLimit`,
`ResultTooLarge`, `StackExhausted`) or the client's cancellation.
`RunError` is `#[non_exhaustive]`: a later kiln adds kinds without a
major version, so a client matches it with a wildcard arm, and **a kind
a client built against an older kiln does not know is kiln's failure of
the run, never the host's**: no new kind will report a handler's error
(a handler's error is `Host`, now and later), so the wildcard arm reads
"the run failed; the diagnostics (§7) say why", the reading of
`Failed`. A client that only needs to tell the two apart calls
`is_host()`, which a newer kiln answers for its new kinds too. The
other types of the interface (`RunValue` and its tree, §4.1) are closed
sets by design; `RunError` is open because kiln will learn new ways for
a run to end.

**`Uncaught`'s type** (WP-2480): `ty: Entity` (§4.1), the exception
object's type ([except.throw]/3: the operand's static type with its
top-level cv-qualifiers removed, an array or function type decayed),
spelled fully qualified as `std::meta::display_string_of` spells it
(`ns::v0::error`, `int`, `const char*`), and `decl`, the first
declaration of its class or enumeration, by §4.1's rule: `Some` when
the class existed when the session was opened, `None` for a class a run
instantiated and for a type that is no class or enumeration. Two
classes of one name in two namespaces differ in both. The type is the
exception object's, not the handler's: a class derived from another
exception class is itself (its base's members are in its value), a
`throw r;` of a base reference to a derived object throws a copy of the
base (the operand's static type), and `throw;` rethrows the handled
object, of its own type, whatever the handler caught it as
([except.handle]/16). Until WP-2480 the field was `type_name: String`,
spelled as the note spells it, without the enclosing namespaces.

**`Uncaught`'s value** (WP-2502): `value: Result<Box<ValueTree>, String>`,
the thrown object's value as a run's result of its type would be read
(§4.1: a `ValueTree`, the root and its constants table; the same tree
`run` returns for an entry that returns that object and that
`variable_value` returns for a variable holding it), read when the
exception leaves the entry, before the failure is rendered: the value
the note renders (`unhandled exception of type 'ns::v0::error' with
content {2, 1}`), so a client never parses the note. For `throw;` it is
the handled object as the handlers left it. It is frozen by the code
that freezes a result (§4), so it is the same value and has the same
conditions: `Err(reason)`, and no partial form, when
- the object does not freeze as a constant: a subobject not
  initialized, a pointer or reference to an object the run created (an
  object of its heap, a local, the exception object itself), a pointer
  to an immediate function (`reason` is the first note the refusal would
  give, as for a result: `pointer to heap-allocated object is not a
  constant expression`, `subobject 'b' is not initialized`);
- its tree exceeds `budget.result_bytes` (`the value needs more than N
  bytes on the host (the run's result budget)`), or it has no form for
  the client (a type outside §3's result types: `reason` says what
  cannot be read).
A partial form (the representable members, the others marked) would be
a second value model beside §4.1's, one only failed runs had: the
`scalars` above already give a value's leading integral members whatever
the rest holds, and `what` its message. A class derived from
`std::exception` whose `what()` reads a string the run allocated (a
standard library's `std::runtime_error`) has `Err` for its value and
the string in `what`. A `throw 1;` has `Ok` with the root `Signed(1)`.
The value changes nothing else: the scalars, the note and the run's
counters are those of a run before WP-2502, and a run that does not end
`Uncaught` freezes nothing.

**`Uncaught`'s scalars** (WP-2324): `scalars: Vec<u64>` holds the values
of the thrown object's first `UNCAUGHT_SCALARS` (4) non-static data
members of integral or enumeration type (`bool`, the character and
integer types, `_BitInt(N)` with N <= 128, an enumeration by its
underlying type), in declaration order, for any class type, derived
from `std::exception` or not, read when the exception leaves the entry
(before the failure is rendered):
- **Bases.** A class's direct non-virtual base subobjects come first, in
  base-specifier order, each read the same way (its bases, then its own
  members), so the members of a complete object count in the order they
  are initialized ([class.base.init]/13). A virtual base is not read.
- **Other members are skipped**, not counted: members of class, union,
  array, pointer, pointer-to-member, floating-point, `std::nullptr_t`
  or `std::meta::info` type (a class member's own members are not
  read), and static members.
- **Bit-fields** count, with their value (the field's bits, sign-extended
  for a signed type); an unnamed bit-field is no member
  ([class.bit]/2) and does not count.
- **Values.** A signed value is in two's complement, sign-extended to 64
  bits (`-2` is `0xFFFF_FFFF_FFFF_FFFE`); `bool` is 0 or 1; a value wider
  than 64 bits (`__int128`) keeps its low 64 bits.
- **Ends.** A member whose value is not initialized ends the list (the
  ones before it stay), so a position is the same member in every
  report of a type that has one.
- **Empty** for a thrown object that is not of class type (`throw 1;`:
  `type_name` says `int`), for a union, and for a class with no such
  member.

Test: `crates/kiln_parse/tests/eval_session.rs`
`uncaught_exceptions_carry_their_leading_scalars` (a `std::exception`
class, a plain one with a base, a bit-field and an unnamed one, an
empty class, a scalar, an uninitialized member).

Tests (WP-2480, WP-2502, WP-2503): `crates/kiln_parse/tests/eval_session_results.rs`
`an_uncaught_class_has_its_qualified_type_declaration_and_value`
(`ns::v0::error` against `other::error`: qualified spelling, `decl`
equal to that of the entry returning one, the value equal to that
entry's result, the note's content), `a_rethrown_exception_hands_over_the_rethrown_value`
(`throw;` after the handler changed the object, twice nested, and a new
`throw` in a handler), `a_derived_exception_type_is_its_own` (a derived
class with its base subobject, a `throw` through a base reference, a
rethrow of a derived object caught as its base) and
`an_unrepresentable_thrown_value_says_why` (a pointer to the run's
heap, a member not initialized, a pointer to a string literal into the
constants table); `crates/kiln_parse/tests/eval_session.rs`
`an_argument_check_is_not_the_handlers_error` (each argument check
against the handler's `Err`: `HostArgument` with the handler not
called, `Host`, `is_host()`), `every_failure_kind_leaves_the_session_as_fresh`
(`is_host()` of every kind) and, for `StackExhausted`,
`a_run_checks_the_stack_of_its_own_thread` and
`a_variable_value_checks_the_stack_of_its_own_thread` (§9).

## 9. Threading

`EvalSession: Send` (not `Sync`): a client may move a session to a
worker thread (a scoped thread, as it borrows the engine), one per
worker. `RunReport: Send`. `CancelToken: Send + Sync`. The
translation unit's handler, which the session keeps, is `Send`
(`Box<dyn HostCalls + Send>`); a run's handler need not be, as it is
only borrowed for the run (§6, spec/host-calls.md §3; WP-2321).

**The stack guard follows the calling thread (WP-2410).** Semantic work
a run does (an instantiation, a nested evaluation through the Oven's
bridge) checks the native stack with the stack guard
(spec/sema-templates.md P4 (c), `kiln_sema::stack_guard`), which `Sema`
copies from the thread that creates it. During `run` and
`variable_value` the guard checked is the calling thread's
(`StackGuard::current()`, the one `StackGuard::install` put there, or
none), and the creating thread's is restored when the call returns. A
copy measured on another thread reads as no stack left, so before
WP-2410 a run on any thread but the compiling one failed "stack
exhausted" at its first check, and every later run did too. A client
that runs sessions on worker threads gives each worker the stack it
needs (`std::thread::Builder::stack_size`) and installs the guard first
thing on it; on a thread without one, every check passes (deep
recursion then overflows the thread's stack, as for any library caller,
spec/sema-templates.md §10.3).

- **Exhaustion is the run's.** When a run's work exhausts the calling
  thread's stack, the run fails with `RunError::StackExhausted`
  (WP-2500; `RunError::Failed` before it) and the fatal
  `sema_stack_exhausted` ("kiln: stack exhausted during template
  instantiation; ..."), its instantiation context as notes. The kind is
  the diagnostic's: a run that would have failed `Failed` and has
  `sema_stack_exhausted` among its diagnostics is `StackExhausted`; a
  run whose evaluation failed otherwise first (a budget, a cancellation,
  an uncaught exception) keeps that kind. Within the
  run the guard's tripped flag stays set, so the recursion unwinds and
  no further check passes. When the run ends it is restored to its
  value at the run's start, which is clear (a session opens only on a
  translation unit without a fatal error), so the next run, on any
  thread, starts with the guard untripped. `variable_value` follows
  the same rule (WP-2510): a read that would be refused `NotAConstant`
  and has `sema_stack_exhausted` among its captured diagnostics (its
  own exhaustion, or the replayed one of work an exhaustion stopped) is
  `ValueError::StackExhausted` (test:
  `crates/kiln_parse/tests/eval_session.rs`
  `a_variable_value_checks_the_stack_of_its_own_thread`).
- **What it stopped stays stopped.** The instantiation the exhaustion
  cut short is kept as it failed, as any failed semantic work (§5.1
  rule 1: semantic work is not rolled back): every later run that
  reaches it fails with the same diagnostic and kind
  (`StackExhausted`: the replayed diagnostic is the exhaustion's), on
  any thread and with any stack; runs that do not reach it are
  unaffected. This is the one
  outcome of kept work that depends on the running thread (its stack
  size), not on the translation unit alone: a fresh session on a
  thread with a larger stack does the work. A client that hits it gives
  its workers more stack, or `-fkiln-stack-size=` the compiling thread
  and runs there.
- **Checked thread-bound state.** Nothing else a run reads is bound to
  the thread that compiled the session: the Oven's limits (steps,
  memory, call depth) are counts, not native stack measures; the other
  `thread_local!`s are read on the calling thread, never through a
  copy, and hold instrumentation or hints, no semantic state (the
  per-thread symbol intern cache, whose hits are verified against the
  interner; the crash reporter's current activity; the `-ftime-trace`,
  statistics and profiling recorders, which record on the thread that
  armed them).

Test: `crates/kiln_parse/tests/eval_session.rs`
`a_run_checks_the_stack_of_its_own_thread` (compiled on a guarded
thread A; a run on B instantiating templates, then one on A; a run on
a 4 MiB B that exhausts it fails with the stack diagnostic, then runs on
A and B succeed, the exhausted entry fails alike on A, and a fresh
session runs it on A) and `a_variable_value_checks_the_stack_of_its_own_thread`
(WP-2420: the same for `variable_value`'s on-demand instantiation: reads
on B and A, one exhausting a 4 MiB thread refused and later reads on
either thread fine, a run reaching the stopped initializer reporting
the exhaustion, a fresh session reading it on A).

**Limits that bound the native depth (WP-2500).** A run's own
evaluation keeps its call frames in the Oven's frame space, not on the
native stack; what recurses natively is semantic work (an
instantiation, a substitution, a nested evaluation through the bridge,
each a level of the native recursion of the one that needed it,
spec/sema-templates.md §10.3). Every kiln limit on how deep that
recursion goes, with its default, the option that changes it and its
measured native stack per level (the slope between the deepest level
that passes under two stack sizes, so the fixed cost of the levels
below is out; aarch64 Linux development host, the debug build with
line tables and the `harness` profile, `opt-level = 3`, which is the
release build's code; indicative, not a gate):

| limit | default | option | debug, per level | release, per level |
|---|---|---|---|---|
| instantiation depth: instantiation frames on the stack (spec/sema-templates.md §10.3) | 1024 | `-ftemplate-depth=N` | class template chain (`C<N>` naming `C<N - 1>::type`): 23.6 KiB; static member chain (`R<N>::v = R<N - 1>::v + 1`, an instantiation and a nested evaluation per level): 34.8 KiB; function template chain (`g<N>()` calling `g<N - 1>()`): 42.8 KiB; the reflection chain below: 53.9 KiB | 9.6 KiB; 7.5 KiB; 11.6 KiB; 23.9 KiB |
| the Oven's call depth: a run's own calls (`RunBudget::depth`, §8) and a nested evaluation's (the translation unit's) | 16384 in a run (`RunBudget::PROGRAM`); 512 in nested work | `RunBudget::with_depth`; `-fconstexpr-depth=N` | none: 40,000 nested calls of a recursive `constexpr` function pass under a 4 MiB stack | none (likewise) |
| the Oven's on-demand initializations nested in one evaluation (a variable read whose initializer was never evaluated, evaluated in place; `MAX_NESTING` in crates/kiln_oven/src/value.rs) | 512 | none (a deeper read is of an unknown value) | an Oven evaluation per level, reached in practice only through an instantiation chain (counted above) | likewise |
| the parser's nesting (parentheses, braces, declarators, ...) | none: the guard alone | `-fkiln-stack-size=` | 1.6 KiB per parenthesis | 0.8 KiB |
| the step budget (`RunBudget::steps`, `-fconstexpr-steps=`) | 2^34 in a run; 2^20 in nested work | as named | bounds work, not depth | |

The parser's nesting is not reached by a run (a run parses nothing; it
bounds only the translation unit's parse, on the thread that opens the
session). The other recursive walks of sema and the Oven (the result
decoder's 256 nested types, the type printer's 400, a splice's 1024,
the warning analyses' expression walks) are bounded by the nesting of
a type or an expression the translation unit wrote or an instantiation
made, so the rows above bound them.

**The motivating case**, a linearly recursive class template driven
through reflection (each level's static member's initializer
substitutes the next level with `std::meta::substitute`, completes it
with `static_data_members_of` and reads its member with `extract`;
tests/support/kiln_meta.h), costs 54.9 KiB per level of a run in a
debug build and 23.9 KiB in a release build (the table's 53.9 and 23.9
KiB are `-fsyntax-only` of a `static_assert` over it, on the compiling
thread). On a 4 MiB worker (the smallest `-fkiln-stack-size`, and a
common thread size) a run reaches level 46 in a debug build and 118 in
a release build (the compiling thread at 4 MiB: 47 and 114) and then
fails `StackExhausted`: the 1024 levels `-ftemplate-depth` allows would
need about 55 MiB (debug) and 25 MiB (release) of stack. Measured by
the ignored test `measure_native_stack_per_level` in
`crates/kiln_driver/tests/eval_session.rs` (a run per probe, each in a
fresh session, on guarded threads of 4, 16 and 64 MiB: levels 46, 270
and 1166 in a debug build, 118, 631 and 2685 in the `harness`
profile); the other rows by `-fsyntax-only` of the pattern under
`-fkiln-stack-size=16M` and `64M` with the limits raised.

**Sizing a worker.** A limit whose cost per level times its default
(plus the guard's 1 MiB red zone, `RED_ZONE`, and the stack the
client's own frames use) exceeds the worker's stack is answered by the
guard first: the run fails `StackExhausted`, not with the limit's own
diagnostic, at the depth the stack allows. To have the limit answer, a
client gives the worker `cost x limit + 1 MiB` and more (for the
default `-ftemplate-depth=1024`: about 25 MiB for the reflection chain
in a release build, 55 MiB in a debug one; the driver's compiling
thread has 1 GiB, `-fkiln-stack-size`), or lowers the limit
(`-ftemplate-depth=` on the session's command line) to what the
stack holds. A per-level cost depends on the pattern (the table's
spread) and on the build, so a client sizes for its own deepest
pattern, measured as the test above measures.

## 10. Cost

- **No session, no cost.** Without a session the Oven's dispatch is
  unchanged but for the step check comparing against
  `Oven::step_stop` (equal to the step budget outside a run) instead
  of the budget itself, and the compile path tests one `Option`;
  bench/oven counters (steps, evaluation bytes, accounted memory) are
  identical to the base's on all 29 entries, and wall times are within
  host noise (interleaved min-of-12: `wide-struct-read` 289 vs 284 ms,
  `vector-alloc` 414 vs 405 ms, `-fsyntax-only` of `<bits/stdc++.h>`
  1254 vs 1018 ms, base vs WP-1995).
- **Retained memory and per-run overhead**, measured by the ignored
  test `measure` in `crates/kiln_driver/tests/eval_session.rs` (a
  release build, `-- --ignored --nocapture measure`; WP-1995, aarch64
  Linux development host; indicative, not a gate):

  | | small transform | `<bits/stdc++.h>` + transform |
  |---|---|---|
  | parse and open | 49 ms | 1.16 s |
  | kept (`kept().total()`) after open | 35 KiB | 60 MiB (AST 52 MiB, Oven 8 MiB, Sema 0.6 MiB) |
  | process RSS growth to open | 11 MiB | 77 MiB |
  | first run of an empty entry (open to first instruction) | 30 us | 14 us |
  | an empty entry's run | 71 ns | 76 ns |
  | runs/s of `return 1;` | 11.4 M | 10.7 M |
  | 50,000 runs of 100 host calls | 0.44 s (8.7 us/run), kept flat | 0.44 s (8.9 us/run), kept flat |
  | cancellation of `for (;;) {}` from another thread | 0.8 ms | 0.1 ms |

  A `std::vector<int>` of 10^6 `push_back`s: 0.46 s and 3.8e7 steps
  per run, peak 6 MiB; `kept()` after run 1000 equals `kept()` after
  run 2 and the process RSS does not move between them.
- **Content keys** (WP-2074), measured by the ignored test
  `measure_content_keys` (same file; `--profile harness`, aarch64 Linux
  development host, min of 20 runs): a run making 2000 `substitute`
  calls, each over a `std::vector<info>` it built, takes 4.65 ms while
  no `substitute` call has a record (no key is computed), 4.76 ms (+2.4%,
  about 55 ns per call) once one has (every call computes its content
  key), and 4.92 ms when half the calls also hit the record and replay
  it. A record's key words count in `kept().replay`. Without a session
  no key is computed: bench/oven counters (steps, evaluation bytes,
  accounted memory) are those of the base on all 29 entries (none
  reflects; the only change outside sessions is that `extract` and
  `constant_of` of an automatic variable mark the calls in progress
  impure, spec/oven.md "Reflected locals").
- **Flat over runs.** A run records no evaluation statistics
  (`-fconstexpr-report` rows) and its entry stub is compiled once; the
  frame and heap spaces' buffers are released after a run down to
  1 MiB each (`RELEASE_FLOOR`), so a run that built a large structure
  does not hold its memory. `kept()` after run 1000 equals `kept()`
  after run 2 for an entry whose allocations do not depend on the run.

## 11. Precompiled headers and modules

A session never writes a precompiled header or a module file, and a
session's translation unit is parsed without recording either.

## Tests

- `crates/kiln_parse/tests/eval_session.rs`: every
  rule above, with a Rust handler: isolation (a host call returns
  different data each run; nothing a run builds is visible to the
  next), each budget (N-1 fails, N passes, fresh and used), memo
  hit or miss same steps on run 1 and 1000, cancellation from another
  thread, every failure kind followed by a clean run, host calls in
  runs, diagnostics and their replay, kept memory flat, every entry
  error, a refused translation unit.
- `crates/kiln_parse/tests/eval_session_constexpr.rs`: `constexpr`
  entries (WP-2072): `int`, `bool`, `unsigned long long` and `void`
  entries, a static member, specializations by `DeclId`, `NotConstexpr`;
  `if consteval`, `std::is_constant_evaluated()` and
  `__builtin_is_constant_evaluated()` true and the plain bit false;
  a `consteval` call in a `constexpr` entry (read from the translation
  unit, or charged as the instantiation's nested work) against the same
  in a `consteval` entry; an escalated entry and its diagnostic; the
  failures (undefined behaviour, a non-`constexpr` call, a leak, a
  `void` entry, an exception, a host-call-dependent one) with clang's
  notes; a `consteval` wrapper; run 1 and run N, fresh and used, three
  orders.
- `crates/kiln_parse/tests/eval_session_results.rs` (WP-2071): §4.1, each kind of value (an integer, `bool`, `_BitInt`, an
  enum with and without an enumerator, a float NaN payload and `-0.0`,
  `nullptr`, a pointer to a static object, to a member subobject, to
  an array element and one past the end, to a function, into a string
  literal and a `define_static_string`, through a union, a pointer to
  member, an `info` of a type, a declaration and a value), nested
  classes and arrays, a class with bases, an empty member, a union and
  an anonymous one, a bit-field, a large array just under and just over
  `result_bytes`, each refused type (`WrongReturnType`) and each
  refused value (a pointer to a run's object, an uninitialized member,
  a constant destruction that fails), the constants table (a span-like
  pair into one string literal: one index; two equal literals written
  apart: two indices; one literal expression evaluated twice: one;
  equal `define_static_string`s: one; a template parameter object
  reached 64 times: one, decoded once; pointers inside a constant: the
  next indices, in order), `decl` for an
  entity of the translation unit and `None` for a specialization a run
  instantiated, `run(f) == variable_value(v)` on run 1 and run N, fresh
  and used, `variable_value` before and after runs and on demand, and
  `kept()` flat over 1000 runs. `crates/kiln_ast/tests/consts.rs`
  `truncate_to_a_mark`: `ConstStore::mark` / `truncate`. WP-2210 and
  WP-2212: a class with one virtual base, a diamond with a shared one
  (decoded once), a virtual base beside non-virtual ones and one
  reached only through a member, pointers into virtual base
  subobjects, member pointers to a base's member as `int B::*` and as
  `int D::*`, a derived member cast to the base, a null member pointer
  and a member function pointer through a base, each equal to the
  translation unit's own `constexpr` variable on run 1 and run N.
- `crates/kiln_parse/tests/eval_session_nested.rs`: §5.1 (WP-2073):
  each fixed bug of §7, the determinism criterion (run 1 and run N,
  fresh and used, both orders, nested inside nested work, the memo),
  the budget decision on own plus charged nested steps, depth and
  bytes, cancellation, a host call refused in nested work, and nested
  work over the translation unit's limit. WP-2075: each of the `Class`
  (a specialization, a member class and enumeration, a class a
  reflection query completed), `Exc` (a `noexcept(e)` instantiated, a
  defaulted constructor's computed specification), `Nsdmi`, `Sat` and
  `Atom` units reached through two different units, fresh and used, in
  both orders, with the same `nested_steps`, `nested_calls` and
  diagnostics; a constraint over the translation unit's step limit
  failing every run alike.
- `crates/kiln_driver/tests/eval_session.rs`: `with_eval_session` from a
  command line (an include path and a define, a refused translation
  unit); content keys (WP-2074): a query over operands a run built
  (`substitute` of a vector, of a `define_static_string`, of a filtered
  `members_of`) replays its warning in a later run that builds them at
  another address and not in one that builds other content at the same
  address, a run after others reports what a fresh session's would, and
  the memoization cache skips pointer and reference arguments. A `constexpr` entry over `<vector>`
  and `<string>` (WP-2072) with `std::is_constant_evaluated()` true and
  a leak reported with the `constexpr` headline.
- `crates/kiln_sema/src/meta/key.rs` (unit tests): every operand of
  every reflection op has a key row.
