# spec: C++20 modules (part A: in-process modules, header units, `import std;`)

Standard: [module] (module.unit, module.interface, module.import,
module.global.frag, module.private.frag, module.context, module.reach),
[basic.link]/8-11, [basic.lookup.argdep]/4, [cpp.module], [cpp.import],
[cpp.pre]/1 (import-seq / directive tokenisation), [dcl.module] grammar.
Reference: clang 22 (`clang++-22 -std=c++20`) for diagnostics, mangling and
`__cpp_modules`; libc++ 19's `std.cppm` / `std.compat.cppm`.

## 1. The design: no binary module interface in part A

A kiln invocation that imports a module **builds that module in-process
from its interface source**. There is no BMI and no serialisation: the
imported unit is preprocessed, parsed and analysed by the same
`Preprocessor` / `Parser` / `Sema` / `Ast` as the importer, its tokens
spliced into the token stream at the import, with every declaration tagged
with the module unit that owns it. The module cache is per-invocation
memory: a unit is built once per invocation, and a second import of it only
records the import edge (and, for a header unit, re-applies its macros).

This is clang's "implicit module build" kept inside one compilation. The
cost of importing a module is the cost of parsing its source; for
`import std;` with libc++ 19 that is about the cost of including every
standard header once.

Part B (a later WP) adds a BMI. The ownership model below is designed so
that a serialiser can be added without reworking lookup: everything lookup
and codegen consult is the `kiln_ast::modules::ModuleTable` (units, the
owner of every declaration, the import graph), which a BMI writer can
serialise with the AST and a reader can rebuild.

Crate placement. Data lives in `kiln_ast::modules` (read by sema, the
mangler in `kiln_abi` and codegen); the semantic rules live in
`kiln_sema::modules`; the directives in `kiln_pp` (`modules.rs`); the
directive events and export declarations in `kiln_parse`. No new crate is
needed: no dependency edge is added.

## 2. Activation and driver flags

Modules are enabled for `-fsyntax-only` and code generation under
`-std=c++20` and later (`PpOptions::modules = Some(ModuleOptions)`). `-E`
keeps printing directives as text (as today), and before C++20 a directive
stays "unsupported".

Module sources are found in this order (spec/driver.md, "Modules"):

1. `-fmodule-file=<name>=<path>`: the interface source for module `<name>`
   (a partition is named `M:P`). A `-fmodule-file=<path>` without `=` is
   accepted and ignored (it names a BMI; there is none in part A).
2. `-fprebuilt-module-path=<dir>` (repeatable, in order): `<dir>/<name>.cppm`,
   then `.ixx`, then `.cpp`; a partition `M:P` is looked up as `M-P.*`.
3. `std` and `std.compat`: `<prefix>/share/libc++/v1/std.cppm` and
   `std.compat.cppm`, where `<prefix>/include/c++/v1` is the libc++ header
   directory `-stdlib=libc++` resolved to (spec/driver.md; symlinks
   resolved, so Debian's `/usr/include/c++/v1` yields
   `/usr/lib/llvm-19/share/libc++/v1`); when no `share/libc++/v1` sits
   beside those headers, `<llvm>/share/libc++/v1` of `KILN_LLVM_DIR`
   (default `/usr/lib/llvm-19`; the llvm-22 layout is the same).

A module that is not found is `fatal error: module 'M' not found`. A file
that cannot be read is the include "file not found" error.

Activation inside a TU. The preprocessor scans the main file's text once
for a line starting with `module`, `import` or `export`; if there is none
the TU pays nothing (no ownership records, no visibility checks). An
import first reached through an `#include` still works; only the dedup of
§6 for declarations before that point is lost.

## 3. Directives (kiln_pp)

A directive is recognised exactly where [cpp.pre]/1 says: `module`,
`import`, `export module`, `export import` at the start of a logical line,
followed on the same line by an identifier, `:`, `;` (for `module`) or a
header name / `<` / string (for `import`). Elsewhere `import` and `module`
are ordinary identifiers (`int import = 1;` keeps working).

With modules enabled, the preprocessor consumes the directive (tokens up to
`;`, unexpanded; module names are never macro-expanded and an object-like
macro of that name is diagnosed) and queues a `Marker::Module` carrying a
parsed `ModuleDirective`:

- `module;` - global module fragment introducer;
- `[export] module M[.N][:P] [attrs];` - module declaration;
- `module :private;` - private module fragment;
- `[export] import M;` / `import :P;` / `import M:P;` / `import <h>;` /
  `import "h";`.

Attributes on module and import declarations are skipped (no attribute
applies to them in C++26).

**Performing an import.** After an import directive (and after `module M;`,
which implicitly imports the primary interface of `M`, [module.unit]/8), the
preprocessor resolves the source (§2; header units through the
`IncludeResolver` as for `#include`). If the unit is already built or being
built it does nothing more (a cycle is an error). Otherwise it queues
`Marker::UnitBegin`, saves its unit-local state (macro table, `#pragma once`
files, include guards, poisoned identifiers, `push_macro` stacks, include
depth), starts the unit with only the predefines (`<built-in>`, re-entered)
and pushes the unit's file. When that file ends, the importer's state is
restored and `Marker::UnitEnd` is queued. The unit's tokens therefore reach
the parser between the two markers.

**Header units** ([cpp.import]/5, [module.import]/5). The header is built as
an implicit header unit in-process: no `-fmodule-header` is needed. When it
ends, the macros it defined (all definitions whose location is outside
`<built-in>`) become visible in the importer at the import, and are
re-applied on every later import of the same header. An `#include` of an
importable header stays a textual include: the include-to-import
translation of [cpp.include]/10 is implementation-defined and not done.

Named module units export no macros.

## 4. Unit kinds and the preamble (kiln_parse, kiln_sema::modules)

Directive events reach the parser keyed by the index of the token they
precede and are processed at the start of the next namespace-scope
declaration (never inside one). Sema tracks the current unit's phase:
`Start` → (`module;`) `Gmf` → (module declaration) `Purview` →
(`module :private;`) `Private`. Errors (clang 22 wording):

| Rule | Diagnostic |
|---|---|
| [module.global.frag]/1 `module;` first | `'module;' introducing a global module fragment can appear only at the start of the translation unit` |
| [module.unit]/1 one module declaration, first or after the GMF | `module declaration must occur at the start of the translation unit` (+ note `add 'module;' to the start of the file to introduce a global module fragment`); `translation unit contains multiple module declarations` |
| [module.private.frag]/1 | `private module fragment declaration with no preceding module declaration`; `private module fragment in module implementation unit`; `private module fragment redefined` |
| [module.import]/1 imports before other declarations | `imports must immediately follow the module declaration` |
| [module.import] partitions | `module partition imports must be within a module purview` |
| [module.interface]/1 | `export declaration can only be used within a module interface`; `export declaration appears within another export declaration`; `export declaration appears within anonymous namespace`; `anonymous namespaces cannot be exported` |
| [module.interface]/3 (C++20 wording) | `declaration of 'x' with internal linkage cannot be exported`; `cannot export redeclaration 'x' here since the previous declaration has module linkage`; `using declaration referring to 'x' with internal linkage cannot be exported` / `... with module linkage ...` (+ note `target of using declaration`) |
| [module.global.frag]/1, [cpp.pre] (P3868, `-fclang-compat=22`) | a preprocessing directive other than null, line-marker, `#line`, `#pragma`, `#error`, `#warning` or `#ident`, in the main file before `module;`, makes `module;` the first declaration too late: the two errors above, with clang 22's note at the first such `#` (kiln_pp records it in `ModuleDirective::after_directive`; sema `module_note_pp_directive`) |
| [cpp.module]/2 (P3034) | `module name component 'X' cannot be an object-like macro` (clang 19 and 22 expand the macro instead) |
| import at top level only | `import of module 'M' appears within namespace` |
| missing module | `module 'M' not found` (fatal) |
| [basic.link]/11 | `declaration of 'x' in module M follows declaration in the global module`, `... in the global module follows declaration in module M`, `... in module N follows declaration in module M` |

Locations follow clang 22: preamble errors (`module declaration must
occur ...`, `multiple module declarations` and its note) are at the
`module` keyword; a missing module at its name; unnamed-namespace errors and
notes at the namespace's `{`; `export block begins here` is attached only
when the export is a `{ ... }` block. P2615 (meaningful exports): `export
static_assert(...)`, `export using namespace n;`, `export { ; }`, exported
explicit specializations and instantiations are accepted, as clang 22.

Imports are allowed at the start of the private module fragment
([module.import]/1, "and of the private-module-fragment"), as clang does.

`export` declarations build `DeclKind::Export` contexts (transparent, like
linkage specifications). `export import M;` re-exports `M`.

`__cpp_modules` is defined as `1` from C++20 on: clang 22's value
(verified with `clang++-22 -std=c++20 -dM -E`; it does not claim `201907L`
while its implementation is incomplete; kiln follows the reference
compiler). clang 19 does not define it, and no libc++ or libstdc++ header
tests it.

## 5. Ownership and visibility

`ModuleTable` (kiln_ast) holds the units (`UnitInfo`: kind, module name,
partition, imports, exported imports) and the owner of every declaration as
change points `(first DeclId, Owner)`: `Owner { unit, purview, exported }`.
Sema pushes a change point when the current owner changes (unit begin/end,
module declaration, private fragment, export begin/end), so the owner of a
declaration is a binary search on its id. Unit 0 is always the main file.

Attachment ([module.unit]/7): a declaration in a purview is attached to the
named module unless it is in a linkage specification; everything else is
attached to the global module. `Ast::module_attachment` walks lexical
contexts, so a namespace first opened in the purview and reopened inside
`extern "C++"` attaches that reopening's members to the global module
(WP-389).

**Visibility** of declaration `D` (owner `O`) from the current unit `C`
(`Sema::module_visible`), consulted by `Sema::filter` (every ordinary,
qualified and class-scope name lookup), by ADL, and by redeclaration lookup:

1. modules inactive, or `D` not at namespace scope, or `D` a namespace:
   visible (members are filtered one by one);
2. `O.unit == C`: visible;
3. `D` is in a range `C` merged (§6): visible;
4. `O.unit` not imported by `C`: not visible. Imports are transitive
   through `export import` ([module.import]/7), and a unit of module `M`
   importing another unit of `M` also imports what that unit imports;
5. `O.unit` is a header unit: visible;
6. `D` in the purview and exported, or in the purview and `C` is a unit of
   the same named module: visible; otherwise (the GMF of an imported unit,
   non-exported purview declarations of another module) not visible.

A using-declaration is filtered by its own owner, not its target's: `export
namespace std { using std::vector; }` makes `std::vector` visible to an
importer although the class template, declared in std's GMF, is not.

ADL ([basic.lookup.argdep]/4, `Sema::module_adl_visible`) uses the same
predicate plus: a function that is *exported*, attached to a named module
`M` and only reachable (its unit is not imported by the current one) is
visible when it has the same innermost enclosing non-inline namespace as
an associated entity attached to `M` (an associated class or enumeration;
clang 22 and 19 consider classes only: deviation 10); hidden friends of associated classes are always visible. The
standard's /5 example: `f(x)` finds `R::f` through `R::X`, `f(x, S::Z())`
does not consider `S::f` although `S` is associated
(`basic.lookup.argdep/module-example.cpp`, `module-enum.cpp`). `R::f(x)`
is `no type named 'f' in namespace 'R'` (`R::f` is not visible to
qualified lookup). Its invalid declaration of `x` does not hide the valid
`x`, so the example's later errors follow, in the standard's order (spec/sema.md
§3, "Invalid declarations"). A
non-exported function of the module's interface is not found by an
importer's ADL: clang 22 reads "in the interface of M" as "exported", and
kiln follows it (`module.interface/visibility.cpp`). Inside an
instantiation every reachable declaration is visible (the instantiation
context, [module.context]), except to ADL a TU-local (internal-linkage)
declaration of another named-module unit, which naming would make
ill-formed ([basic.link]/14) and which clang 22 does not find (`g(t)` in
the /5 example's `apply`; kiln_driver `module_adl_example`).

**Missing imports.** When a lookup finds nothing visible, it is redone
ignoring visibility (`Sema::module_hidden_lookup`) at the sites that would
report the failure: an id-expression (qualified or not), an unqualified
call after ADL, and a type-name in a decl-specifier (qualified or not;
only a type is offered there). What it finds is reachable but not
visible, and is reported as clang 22's `diagnoseMissingImport` does:

- a declaration in the purview of module `M` (exported from a unit or
  partition that is not imported, or with module linkage):
  `declaration of 'x' must be imported from module 'M' before it is
  required`; a partition's declaration names the primary module;
- a global-module declaration (an imported unit's GMF):
  `missing '#include'; 'x' must be declared before it is used`, or
  `missing '#include <h>'; ...` when a header is known (clang
  `getHeaderToIncludeForDiagnostics`): walking out from the declaration's
  file along its `#include`s (stopping at the module unit; an `#include`
  written with a macro is walked through, as in clang), the first
  include-guarded file (a controlling macro; not `#pragma once`), named as
  kiln_pp suggested (spec/pp.md `suggest_include`): relative to the
  longest *absolute* search directory holding it, else to the main file's
  directory, else its whole path (clang 22 and 19 read the header's path
  back from the BMI as absolute and compare the search directories as
  written, so a relative `-I` never matches; WP-396);
- either way with `note: declaration here is not visible` at the
  declaration.

An unqualified lookup does not offer a module-linkage declaration of
another module: that stays `use of undeclared identifier` / `unknown type
name` (clang 22 does too until a qualified lookup has loaded the
namespace's declarations from the BMI, an artifact kiln does not model).
The use then recovers with the declaration found; for an exported one its
unit (and what that re-exports) becomes visible for the rest of the unit,
so it is reported once (clang's `createImplicitModuleImportForErrorRecovery`),
while a GMF or module-linkage declaration is reported at every use
(`module.reach/missing-import.cpp`).

Redeclaration matching ([basic.link]/8) excludes non-exported purview
declarations of another named module (a declaration with module linkage of
another module is a different entity); internal-linkage declarations of
other units are excluded as well. A matched previous declaration attached
to a different module is the [basic.link]/11 error.

**Instantiation.** While any template is being instantiated
(`inst.stack` non-empty) the filter is off: the instantiation context
([module.context]) is approximated as "every declaration reachable in the
invocation". This is a documented deviation: it can accept a program that
names a non-visible entity from a template instantiated in the importer
only when the same name is also findable by the template's own definition
context, which is what clang's instantiation-context rule allows anyway in
every case libc++ exercises.

**Reachability.** Every declaration of an imported unit is reachable
([module.reach]/3 allows it): member access, class completeness, default
arguments and inline / constexpr / template definitions all work through
import, since they never go through the namespace-scope filter.

GMF discarding ([module.global.frag]/3-4) is not done: it only shrinks a
BMI, and part A has none. Not discarding is observable only as accepting
some programs that name a discarded declaration through a template
instantiated in an importer (the instantiation deviation above).

## 6. Cross-unit deduplication (the substitute for ODR merging)

An importer and its imports routinely see the same header text (std's GMF
includes every header; the importer may `#include <vector>` too; std.compat
re-includes them all). Since there is no deserialiser to merge
definitions, the parser deduplicates by source position.

Each namespace-scope declaration parsed while modules are active records
`(start key, end key, token count, DeclId range, unit)`, where a key is the
(file, offset) of a token's expansion location and of its spelling
location. At the start of a namespace-scope declaration, if a record from
**another unit** has the same start key, and the token `count - 1` ahead
has the same end key, the declaration is skipped (its tokens consumed, its
pragma events still processed) and the DeclId range of **every** record
from another unit with the same start key, end key and length is merged
into the current unit (visible as if declared there). Merging all of them
matters when one text was parsed several times, as `<stddef.h>`'s
`__need_size_t` sections are under std.compat: lookup can find any of the
resulting redeclarations. Re-parsing the same text
in the *same* unit is never skipped, so genuine redefinitions are still
diagnosed.

The check compares positions and length, not the tokens: two expansions of
the same text under different macro definitions with the same length would
be merged. A mismatch falls back to finer records (a namespace block that
differs is entered and its members checked one by one); a declaration that
still differs is parsed and can then be a redefinition error, where real
ODR merging would accept it. Both are documented deviations.

**Merging by entity (WP-460).** Where the positions differ (another
header path, an importer's GMF that declares the entity itself), the
declarations still merge as entities ([module.global.frag]/1: the global
module is one module; [basic.def.odr]/15), as clang's
`makeMergedDefinitionVisible` does. A declaration of another named unit's
global module fragment is never visible to the current unit except by a
merge (`module_hidden_global`), so:

- *Lookup* takes the found declaration when it is visible, else a visible
  redeclaration of the same entity (`module_visible_redecl`, clang
  `findAcceptableDecl`; a template's through its pattern's chain): the
  lookup table holds only the latest redeclaration, which may be the
  hidden one.
- *A redefinition* whose previous definition is such a hidden GMF
  declaration is not diagnosed: the previous definition (and the template
  it describes) is merged into the current unit (`module_merge_hidden_
  definition`). A class or enumeration body is then skipped like a
  redefinition's (§4.4 of spec/sema.md); a variable, a variable template
  specialization and a function keep the new definition as a
  redeclaration of the entity; a concept is created out of lookup.
- *An opaque enum redeclaration* (which creates no declaration of its
  own) merges the hidden enumeration it redeclares
  (`module_merge_hidden_redecl`).
- *A scoped enumerator* is visible whenever it is found: it is reached only
  by a qualified lookup naming its enumeration, which is then reachable
  ([module.reach]/5). An unscoped enumerator follows its enumeration.

The new definition is not checked against the old for ODR equivalence
(clang checks with its ODR hash): a documented deviation (§10, item 11).

## 7. Code generation and mangling

Codegen treats a declaration owned by an imported named-module unit
(its purview or its global module fragment; not a header unit) like
clang treats one from a BMI: a non-inline function, variable,
constructor or destructor definition is emitted as a declaration only
(the owning unit's object file defines it, so two importers of one
interface link without duplicate symbols); inline functions, templates
and internal-linkage entities are emitted on use as usual. Compiling the
module unit itself emits everything, its global module fragment
included.

**Classes attached to a named module** ([class.mfct]/1,
[class.friend]/7): a member or friend function defined in the class
definition is implicitly inline only when the class is attached to the
global module (`kiln_sema` `enter_definition_scopes`), so a purview
class's in-class definitions are ordinary definitions: the owning unit
emits them strongly, used or not, right after the namespace-scope class
that contains them (in member order, nested classes in place; clang
`HandleInlineFunctionDefinition` / `EmitDeferredDecls`), and importers
only declare them. Defaulted functions and implicit members stay inline.
**Vtables** (clang `ActOnTagFinishDefinition`, `isVTableExternal`): a
dynamic class completed in the purview of an interface or partition unit
has its vtable used there even when nothing uses it (`kiln_sema`
`mark_module_vtable_used`, which also defines its virtual members, so
a defaulted virtual destructor is emitted `linkonce_odr`), and that
unit's vtable is strong whether or not the class has a key function;
every other unit declares it `external`. A global-module-fragment class
with a key function gets its vtable from the unit defining the key
function, the module unit itself (deviation 7). The implicit global
`operator new`/`operator delete` declarations are attached to the global
module ([basic.stc.dynamic.general]/2) even when the first use is in a
purview, so they are never `_ZW`-mangled.

**Module initializers** (Itanium ABI §3.2.6, clang 22
`EmitCXXModuleInitFunc`; `kiln_codegen/src/module_init.rs` and
`global_init.rs`). [basic.start.dynamic]/5 orders a variable after those
of every unit its unit has an interface dependency on; the initializer
functions implement that across object files:

- An interface or partition unit (interface or implementation partition)
  defines `define void @_ZGIW<module>[WP<partition>]()`: external, not
  `dso_local`, the initialization-function attributes, in
  `.text.startup`, an `@llvm.global_ctors` entry (priority 65535) in
  place of `_GLOBAL__sub_I_<file>`. When it calls anything, its body is
  guarded by `@<name>__in_chrg = internal global i8 0, align 1`: load,
  `icmp eq 0`, a branch weighted `!{"branch_weights", i32 1, i32 1023}`,
  then `store i8 1` before the calls, so that it runs once however many
  importers call it. It calls the initializers of its imports, the
  `export import`ed ones first, then the others in source order (each
  once), then its own ordered `__cxx_global_var_init`s. With nothing to
  call the body is just `ret void` and there is no guard.
- Any other unit (non-module, implementation unit) calls its direct
  imports' initializers, in import order (an implementation unit's
  implicit import of its primary interface first), at the start of
  `_GLOBAL__sub_I_<file>`, which is then emitted even with no
  initializers of its own.
- An imported unit's initializer is declared (`declare void @_ZGIW...()`,
  no attributes, created just before the calling function) and called
  only when it does something (clang `NamedModuleHasInit`): the unit, in
  any fragment, defines a namespace-scope variable that clang must emit
  (`DeclMustBeEmitted`: a definition that is not discardable, or one with
  a dynamic initializer or a non-trivial destructor), or one of its
  named-module imports' initializers does something. A unit with only
  functions, such as libc++'s `std` (whose GMF is all inline/template
  code), has an empty initializer that importers do not call. Header
  units have no initializer.

Mangling (Itanium `<module-name>`, clang 22): an entity at namespace scope
attached to named module `A.B` with linkage gets `W1AW1B` before its
unqualified name (`_ZW1AW1B1fv`, `_ZN2nsW1M1kEv`, class members through
their class: `_ZNW1M1S1mEv`); each module-name prefix is a substitution
candidate. Partitions do not appear in entity names. Entities in
`extern "C++"` and internal-linkage entities carry no module name.
Variables attached to a module are always mangled, and a `const` variable
attached to a named module has module linkage (P2788), so it is
`_ZW1M1c`, not `_ZL1c`. `kiln_abi::mangle_module_initializer` gives the
initializer's name, `_ZGIW1M` / `_ZGIW1MWP1P` (spec/abi.md §8.19).
The standard abbreviations `Sa`/`Sb`/`Ss`/`Si`/`So`/`Sd` do not apply to
`std` templates attached to a named module (including a module-attached
`char_traits<char>` / `allocator<char>` argument); `St` still does:
`_ZW1M2g1StS_9allocatorIiE` (spec/abi.md §8.20, WP-389).

## 8. `import std;` (libc++)

`std.cppm` is a GMF including every header, then `export module std;` and
`#include "std/*.inc"`, each `export namespace std { using std::X; }`.
`std.compat.cppm` has its own GMF (deduplicated against std's, §6), then
`export import std;` and the `::` re-exports. Both work under `-std=c++20`
and later. The libc++ 22 layout adds partitions-free `std.cppm` too.

Cost (aarch64): `import std;` plus a small `main` takes about 15 s with a
debug kiln++ and 1.8 s with the `harness` profile, and 100 MiB, roughly the
cost of including every standard header once; link and run are instant, so
neither libc++ test needs the long run limit (`slow_tests.txt` governs only
the run step, spec/libcxx.md). libc++'s own modules tests (`std/modules/`,
`libcxx/selftest/modules/`, 6 tests) went from 6 UNSUPPORTED to 2 PASS
and 4 UNSUPPORTED. The 4 are `.sh.cpp` self-tests of lit's module plumbing:
`libcxx/lit.local.cfg` (a `%{python}` substitution) makes the whole
`libcxx/` subtree `driver: lit.local.cfg`, and behind that their RUN lines
grep `%{compile_flags}` for the `-fmodule-file=std=%T/std.pcm %T/std.pcm`
that libc++'s `format.py` adds after a `--precompile` step, which needs
part B (§9).

## 9. Part B hooks

- A BMI writer serialises the AST plus `ModuleTable` for one unit; a reader
  appends a deserialised unit with fresh `DeclId`s and change points, and
  its `UnitInfo`; lookup does not change.
- §6 dedup is replaced by real ODR merging when deserialising.
- `-fmodule-output`, `--precompile`, and a two-step clang oracle mode in
  the `cargo xtask test` harness (module initializers are emitted from
  the in-memory model, §7).

## 10. Deviations

1. No BMI; modules are built from source per invocation (§1).
2. Visibility is not enforced during template instantiation (§5).
3. Cross-unit merging by source position and length, not by ODR (§6).
4. (Retired: module initializers are emitted, §7.)
5. `#include` is never translated into an import (§3).
6. Attributes on module / import declarations are ignored (§3).
7. **Deliberate, at every compat level (WP-396).** A class in a module
   unit's global module fragment, whose key function that unit's fragment
   defines, has its vtable, `type_info` and type name emitted by that
   unit, and importers declare the vtable `external` (§7; Itanium ABI
   §5.2.3, [basic.def.odr]/15).
   - clang 22 and clang 19 do the same thing as each other, which is the
     opposite. The module unit omits `_ZTV1G`/`_ZTI1G`/`_ZTS1G`. Every
     importer defines them `dso_local`, with no comdat, together with
     G's deleting destructor.
   - So with either clang, an interface linked with two importers fails:
     `multiple definition of 'vtable for G'` (and `typeinfo for G`,
     `typeinfo name for G`). This was probed with `--precompile`, `-c`
     of the BMI and both importers, and one link (WP-396).
   - Defining the key function in the purview instead is ill-formed in
     both clangs (`declaration of 'f' in module m follows declaration in
     the global module`), so this is the only form of the case.
   - kiln keeps the placement that links. Tests:
     `module.global.frag/gmf-dup-run.cpp` (links and runs) and
     `tests/codegen/module_gmf_vtable.cpp` (importer IR against clang 22;
     the difference is an `Allow::Abi` row in `ir_vs_clang.rs`).
8. (Retired: a reachable but non-visible declaration is reported as a
   missing import, §5.)
9. A named module's `thread_local` variables are defined (with their
   `__tls_init`, `_ZTH` aliases and `weak_odr` `_ZTW` wrappers,
   spec/codegen.md §7p) by the unit that owns them only; an importer
   declares them `external thread_local` and reaches them through a
   `linkonce_odr` `_ZTW` wrapper that calls the `extern_weak` `_ZTH`
   initializer when it is present, as for any `extern thread_local`
   declaration. clang 22 instead re-defines the variables, their
   initializers and `_ZTH` aliases in every importer, so a program of a
   module unit and an importer fails to link (`multiple definition of
   x@tm`); kiln follows the Itanium rule
   (`basic.stc.thread/module-tls-run.cpp`).
10. **Deliberate, at every compat level (WP-396).** Module ADL
    ([basic.lookup.argdep]/4) counts an associated *enumeration* attached
    to `M`, as the standard says. clang 22 and 19 count associated
    classes only. Both clangs agree, and spec/sema.md §1.1 keeps the
    accepted language the same at every level, so there is no compat
    gate. The difference is observable:
    - `basic.lookup.argdep/module-enum.cpp`: `h(get_en())`, where `h` is
      reachable through a scoped `P::En` of a non-visible module E, is
      `use of undeclared identifier 'h'` in clang.
    - `basic.lookup.argdep/module-enum-overload.cpp`: with an unscoped
      `P::Un` and a visible `::h(int)`, kiln calls the exact match
      `P::h(Un)` and exits 1, which is the standard's result. clang
      calls `::h(int)` and exits 7. The same shape changes which
      operator a flag enumeration's `|` uses in a module consumed
      transitively.
    - libc++ is unaffected: the `std` module exports declarations of its
      global module fragment, which are attached to the global module,
      so the /4 rule does not apply to them.
11. **No ODR check on a merged GMF definition (WP-460).** A definition
    merged with another unit's GMF definition of the same entity (§6,
    "Merging by entity") is accepted without comparing the two; clang
    compares ODR hashes and reports a mismatch. Bodies that differ are
    ill-formed, no diagnostic required ([basic.def.odr]/15), so kiln
    conforms; the reference repro is
    `module.global.frag/gmf-merge-importer.cpp`.

## Tests

- `tests/conformance/module.unit/`, `module.global.frag/`,
  `module.private.frag/`, `module.interface/`, `basic.link/module-*`,
  `cpp.module/`: preamble, export and attachment diagnostics
  (strict-diags, clang 22 wording; single-file tests run under clang too,
  `requires: clang>=22` where clang 19's wording differs).
- `module.reach/missing-import.cpp` (+ kiln_driver
  `module_missing_import_notes` for its notes): the missing-import
  errors (§5), identical to clang 22's output;
  `module.reach/missing-include.cpp`: the header named in
  `missing '#include <h>'` (guarded, nested, `#pragma once`, unguarded
  and system headers), `module.reach/missing-include-dirs.cpp`
  (relative and absolute `-I`, a macro-written `#include`), and
  `module.import/import-std-hidden.cpp`
  (`<__memory/pointer_traits.h>` through `import std;`).
- `module.interface/export-run.cpp`, `visibility.cpp`,
  `module.unit/partitions-run.cpp`, `module.private.frag/private-run.cpp`,
  `module.import/export-import-run.cpp`: multi-unit programs whose module
  sources are in `Inputs/` (passed as extra inputs and with
  `-fmodule-file`); `skip(clang)` because clang needs a two-step BMI
  build the harness does not drive.
- `module.global.frag/gmf-merge-importer.cpp` (+ `Inputs/gmi-*.cppm`):
  entities declared in both an importer's and an imported unit's GMF
  merge (§6, "Merging by entity"; WP-460), compared with clang.
- `module.import/header-unit*.cpp`: header units (macros visible).
- `module.import/import-std-{vector,format,ranges,compat,hidden}.cpp`:
  `import std;` / `import std.compat;` under C++20, 23 and 26
  (self-checking `run` tests; `skip(clang)` for the same reason).
- `module.import/import-identifier*.cpp`, `import-not-found.cpp`,
  `import-order.cpp`: [cpp.pre] tokenisation and import rules.
- `tests/mangle/modules.cpp`, `module_partition.cpp`: `_ZW` names,
  differential against clang (spec/abi.md §8.19).
- `basic.start.dynamic/module-init-run.cpp`: four module units and an
  importer; the order and once-only property of their dynamic
  initialization ([basic.start.dynamic]/5, §7).
- `module.global.frag/gmf-dup-run.cpp`: an interface with GMF and
  purview definitions (a function, a variable, a key-function class,
  in-class non-inline members, a defaulted virtual destructor) linked
  with two importers that both use them: no duplicate symbol, one
  variable ([module.global.frag]/1, [class.mfct]/1, §7).
- `tests/codegen/module_gmf_vtable.cpp`: an importer of a GMF class
  with a key function (vtable declared `external`, deviation 7), a GMF
  class with inline virtuals (`linkonce_odr`) and a purview class.
- `tests/codegen/module_class_defs.cpp`: IR golden of a module
  interface's in-class definitions (order, strong linkage), its unused
  vtables and the global `operator delete` against clang 22.
- `tests/codegen/module_init_*.cpp`: IR goldens of the module
  initializers against clang 22 (interface, partition, empty, importer,
  import-only, implementation unit), using the IR-diff corpus's
  `//@ module:` two-step mode (spec/codegen.md §6).
- libc++'s `std/modules/` through `cargo xtask libcxx` (§8).
