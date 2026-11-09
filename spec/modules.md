# spec: C++20 modules (part A: in-process modules, header units, `import std;`; part B: module files)

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

Part B (§11, WP-917) adds a module file, so that an interface is compiled
once and imported by separately compiled TUs; its first format is a
*source capsule* from which the importer rebuilds the unit exactly as
above. The ownership model below is designed so that a serialised-AST
format can be added later without reworking lookup: everything lookup
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

1. `-fmodule-file=<name>=<path>`: the interface source, or the module
   file (§11), for module `<name>` (a partition is named `M:P`). A
   `-fmodule-file=<path>` without a name must name a module file, whose
   header gives the module (§11.6).
2. `-fprebuilt-module-path=<dir>` (repeatable, in order): `<dir>/<name>.pcm`
   (§11), then `<dir>/<name>.cppm`, `.ixx`, `.cpp`; a partition `M:P` is
   looked up as `M-P.*`.
3. `std` and `std.compat`: `<prefix>/share/libc++/v1/std.cppm` and
   `std.compat.cppm`, where `<prefix>/include/c++/v1` is the libc++ header
   directory `-stdlib=libc++` resolved to (spec/driver.md; symlinks
   resolved, so Debian's `/usr/include/c++/v1` yields
   `/usr/lib/llvm-19/share/libc++/v1`); when no `share/libc++/v1` sits
   beside those headers, `<llvm>/share/libc++/v1` of the selected LLVM
   installation (spec/driver.md "LLVM discovery"; the llvm-19 and llvm-22
   layouts are the same).

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
and 4 UNSUPPORTED, and since WP-984 all 6 PASS: the 4 `.sh.cpp`
self-tests of lit's module plumbing grep `%{compile_flags}` for the
`-fmodule-file=std=%T/std.pcm %T/std.pcm` that libc++'s `format.py` adds
after its `--precompile` step, which the libc++ driver now reproduces
(spec/libcxx.md "RUN lines"; under clang++-22 too).

## 9. Part B hooks

Part B is §11. Step 1 (WP-917) delivers the source-capsule module file,
`--precompile`, `-fmodule-output`, module-file `-fmodule-file=` and
`-fprebuilt-module-path=` lookup, `.pcm` inputs, the mismatch diagnostics
and the harness's two-step mode (§11.1-§11.8). The serialised-AST format,
in which a reader appends a unit with fresh `DeclId`s and its `UnitInfo`
and ODR merging replaces §6 dedup, is step 2 (§11.10), as are header-unit
files; dependency scanning and CMake are §11.9 (WP-982).

## 10. Deviations

1. A module file is a source capsule (§11): importing it re-parses the
   interface unit, so an import costs the unit's parse (clang's BMI is a
   serialised AST, §11.1). `--precompile` of a unit that is not an
   importable module unit is an error rather than a PCH (§11.6).
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

## 11. Part B: module files and separate compilation (WP-917)

Part B adds a module file (a *BMI*) so that a module interface is compiled
once and its importers are compiled separately, with clang's driver
spellings. Step 1 (this section's §11.1-§11.8, WP-917) is the format, the
driver and the harness; step 2 is dependency scanning and CMake (§11.9,
WP-982), then header-unit files and a serialised-AST format (§11.10).

### 11.1 Decision: a staged source capsule

The three candidates were: (a) a *source capsule*, the interface unit's
inputs, from which the importer rebuilds the unit in-process as part A
does; (b) a serialised AST plus `ModuleTable` (the old §9 plan); (c) (a)
now, in a format versioned so that (b) can replace it. **kiln takes (c).**

Measured on aarch64 with the `harness` profile (peak RSS from
`/usr/bin/time -v`):

| run | time | peak RSS |
|---|---|---|
| part A: `import std;` + small `main`, `-fsyntax-only` | 1.41 s | 94 MiB |
| part A: the same, `-c` | 2.04 s | 94 MiB |
| `std.cppm -fsyntax-only` (the interface alone) | 1.23 s | 90 MiB |
| `#include <vector>` TU, `-fsyntax-only` | 0.43 s | 32 MiB |
| clang 19 `--precompile std.cppm` (31.2 MB BMI) | 1.87 s | 261 MiB |
| clang 19 importing that BMI, `-fsyntax-only` | 0.27 s | 140 MiB |

`std.cppm` reads 1247 files and 8.1 MB of text, so its capsule is about
8.5 MB, and importing it costs part A's parse plus reading and checking
8.5 MB: measured in §11.8 at 1.05x part A's peak memory and the same
time, within the 1.2x budget (the capsule's texts become the source
manager's buffers, so nothing is held twice). Option (b)
would bring `import std;` towards clang's 0.27 s, but needs a serialiser
for every AST node, type, template and constant-evaluator object, and
makes every later AST change a format change. It is the stage-2 format
(§11.10), behind the same magic with a different kind byte; the driver,
the lookup order, the diagnostics and the harness of this section do not
change when it lands.

A capsule unit is rebuilt by the same `Preprocessor`/`Parser`/`Sema` as
part A, so everything part A guarantees holds unchanged: ownership and
visibility (§5), cross-unit dedup (§6), codegen and initializers (§7).

### 11.2 What a capsule holds

A capsule records exactly the inputs of the interface unit's
preprocessing, so that replaying it is the original compile:

- the module key (`M`, `M:P`) and unit kind (interface, interface
  partition, implementation partition);
- the predefines text: the `<built-in>` buffer of the compile that wrote
  it (`-D`, `-U` and `-include` included), so the unit sees the macros it
  was built with whatever the importer defines (the isolation [module]
  requires and clang's BMI has);
- the flags fingerprint (§11.4) and the kiln version (`CARGO_PKG_VERSION`
  plus the git sha when the build recorded one);
- the named-module imports of the unit, each with the path it was found
  at (a source or a module file);
- the file table: the path and text of the unit's main file and of every
  file its preprocessing read (headers, header units nested in it), in
  first-read order, the main file first;
- the include log: every `#include`, `#include_next`, `#import`,
  `__has_include(_next)` and header-unit lookup, keyed by (includer's file
  index or `<built-in>`, name, `<>`/`""`, kind, `lookup_from`), with its
  result (file index, `lookup_index`, system, `extern "C"`, path) or "not
  found";
- the embed log: every `#embed`/`__has_embed`, keyed by (includer, name,
  `<>`/`""`, `limit`, `offset`, query-only), with its bytes or "not
  found".

Recording (`kiln_pp::bmi`, `ModState`). When the driver asks for a module
file, every resolver call made while the innermost *named-module* unit
being read is the main unit is logged; header units nested in it are part
of its capsule, while a named module it imports is not (it has its own
module file, or is rebuilt from its source as in part A).

Replay. A capsule unit (an import resolved to a module file, or a module
file given as the main input) gets a *replay resolver* instead of the
header search: each request is looked up in the include log and answered
with the logged file. A file is loaded lazily, through
`SourceManager::load_regular_file_expecting(path, text)`: when the file
exists, is not loaded yet and has the recorded text's size, the recorded
text becomes its buffer under the file's identity without reading it;
when it is already loaded or its size differs, the loaded or read text is
compared with the recorded one. Either way an unchanged file keeps one
`FileId` (so the unit and an importer that includes the same header share
the `FileId` that §6 dedup and `#pragma once` key on); a changed or
missing file is added as a virtual buffer under the recorded path, which
keeps the build hermetic when headers have changed or moved. (A same-size
edit made after the module file was written is not detected; clang checks
size and mtime and reports the file as modified.) The unit enters its own `<built-in>` buffer, holding the recorded
predefines, instead of the importer's. A request missing from the log
(possible only for a hand-made file) is "file not found". The resolver of
the enclosing named-module unit also serves header units nested in a
capsule unit. Diagnostic suggestions (`missing '#include <h>'`, §5) still
use the real header search.

A named-module import inside a capsule unit is looked up first through the
importer's own flags (§11.6) and then at the path the capsule recorded, as
clang does with the module files a BMI names.

### 11.3 Byte format

All integers are little-endian. A file is:

```
magic      8 bytes   "KILNBMI\0"
format     u32       1
kind       u8        1 = source capsule, 2 = capsule plus sections (§11.11)
header     version string, sha string, module key, unit kind (u8)
body       predefines, fingerprint, imports, files, include log, embed log
sections   kind 2 only: u32 count (at least 1), then per section
           u32 tag and its bytes (u32 length), tags strictly increasing
checksum   u64       FNV-1a 64 of every preceding byte
```

A kind-1 file has no section list and is byte for byte what it was before
kind 2 existed; the reader accepts both kinds, and the format version
stays 1. Tag 1 is the carried constant results (§11.11); tag 0 and
unknown tags are "corrupt" (a newer kiln writes a newer kiln version,
which is a version mismatch before any section is read).

A string is a `u32` byte length and UTF-8; a list is a `u32` count and its
elements; an optional value is a `u8` tag (0 absent, 1 present) and the
value; a file index is a `u32` (`0xFFFF_FFFF` is `<built-in>`, `0xFFFF_FFFE`
no includer).

- **Deterministic.** The same inputs give the same bytes: lists are in
  first-occurrence order (no hash-map iteration), there is no timestamp,
  and paths are recorded as the compile spelled them (no
  canonicalisation). Duplicate log keys are recorded once.
- **Total reading.** The reader (`bmi::read_file`/`read_from`, safe
  Rust) checks the magic and format version, then decodes the body as it
  streams in, every length and count checked against the bytes that
  remain (known from the file's size) before anything is allocated,
  validates UTF-8 and every file index, rejects trailing bytes, and
  finally checks the checksum of everything read. A body that fails to
  decode is reported as "checksum mismatch" when the checksum is wrong
  too (damage), and by the decoding failure otherwise. Any failure is a
  `BmiError`, reported as a diagnostic (§11.4); nothing user-derived
  panics. Streaming matters for memory: a file's text is read straight
  into the buffer the source manager keeps (§11.8). FNV-1a's step is a bijection of its state
  for a fixed input byte, so changing any single byte always changes the
  checksum: a flipped byte is always "corrupt", and a truncated file
  fails the length or checksum check.
- **Written atomically**: to `<path>.tmp.<pid>` and renamed, so a failed
  or interrupted compile never leaves a half-written module file. A
  compile with errors writes no module file (and removes nothing).

The format version changes whenever the layout does; the reader accepts
only its own. `kiln_pp::bmi` holds the format because a capsule is
preprocessor input; no new crate is needed. A section's contents belong
to the crate that reads them: the carried constants are
`kiln_sema::bmi_ast` (they key on sema's evaluation contexts and intern
into its constant store), and `kiln_pp::bmi` frames them as opaque bytes
(§11.11 "Encoding").

### 11.4 Fingerprint and mismatch diagnostics

The fingerprint is a list of (key, flag spelling) pairs for the options
that change what the unit means or its ABI, each resolved to its
effective value: `-std=`, the target triple, `-f[no-]exceptions`,
`-f[no-]rtti`, the signedness of `char`, `-f[no-]char8_t`,
`-f[no-]sized-deallocation`, `-ffreestanding`, `-f[no-]reflection`,
`-fkiln-reflection-mangling=` and `-fclang-compat=`. Options a unit is
isolated from (`-D`, `-I`, warnings) or that only affect the object file
(`-O`, `-g`, `-fPIC`) are not part of it.

On import (and for a module file given as the main input), each field
that differs is an error, then one fatal error, as clang 19/22 do:

```
error: module file 'm.pcm' was built with '-std=c++20' but the current compilation uses '-std=c++23'
fatal error: module file 'm.pcm' cannot be loaded due to a configuration mismatch with the current compilation
```

The other diagnostics, at the import's module name (or without a location
for a driver input):

| condition | diagnostic |
|---|---|
| no module file or source found | `fatal error: module 'M' not found` (§2, unchanged) |
| `-fmodule-file=` names a missing file | `fatal error: module file 'x.pcm' not found` |
| the file holds another module | `fatal error: module file 'x.pcm' holds module 'N', not 'M'` |
| built by another kiln version, or another format version | `fatal error: module file 'x.pcm' was built by kiln V1; this is kiln V2` / `... has format version F, expected 1` |
| bad checksum, truncated, malformed | `fatal error: module file 'x.pcm' is corrupt: <reason>` |
| clang's PCM (`CPCH` magic) | `fatal error: module file 'x.pcm' was built by clang; kiln reads only its own module files` |
| a `.pcm` input that is not a kiln module file | `fatal error: 'x.pcm' is not a kiln module file` |

A capsule is source, so a git-sha difference is harmless and is not
checked; the sha is shown only in `-v` output.

### 11.5 Ownership and codegen across TUs

Nothing new is needed: §7 already treats a declaration owned by an
imported named-module unit as clang treats one from a BMI. The interface
TU's object file (from `-c m.cppm`, or `-c m.pcm`) defines the module
initializer `_ZGIW...` and every non-inline function and variable with
module or external linkage, the in-class member definitions of purview
classes and the purview vtables; an importer declares them and emits only
inline functions, template specialisations and vtables it needs by the
vague-linkage rules. §6 dedup works across the capsule boundary because
the capsule's headers share `FileId`s with the importer's (§11.2); a
header changed on disk since the module file was written becomes a
separate virtual buffer, so its declarations are merged by entity (WP-460)
or diagnosed as redefinitions, which matches clang's ODR checking for that
case. Module mangling (`W` prefixes, `WP` initializer partitions,
substitutions) was compared on `export module a.b:part` with namespace,
class, template, variable and vtable entities: kiln, clang 19 and clang 22
agree symbol for symbol (`_ZGIW1aW1bWP4part`, `_ZN2nsW1aW1b1S1mEv`,
`_ZW1aW1b1c`, `_ZW1aW1b2tfIiET_S2_`, `_ZTVN2nsW1aW1b1SE`), so kiln and
clang objects of one module program link together.

### 11.6 Driver surface

clang's spellings (spec/driver.md, "Modules"):

- `--precompile`: compile a module interface or partition to a module
  file only (default `<stem>.pcm`, or `-o`). The input must be an
  importable module unit; anything else is
  `error: --precompile requires a module interface or partition unit` (clang
  writes a PCH; kiln has none).
- `-fmodule-output` / `-fmodule-output=<path>`: with `-c`, `-S`,
  `-emit-llvm` or a link, also write the module file: to `<path>`,
  or to the `-o` object path with `.pcm`, or `<stem>.pcm` in the working
  directory. For a unit that is not importable it is clang's
  `warning: argument unused during compilation: '-fmodule-output'`.
- `-fmodule-file=<name>=<path>`: `<path>` may be a module file or, as in
  part A, a source. A file is a module file when it starts with the magic;
  otherwise it is a source.
- `-fmodule-file=<path>`: the module name is read from the file's header
  (clang 19/22 also accept this form, with
  `warning: the form '-fmodule-file=<BMI-path>' is deprecated for standard C++ named modules;consider to use '-fmodule-file=<module-name>=<BMI-path>' instead [-Weager-load-cxx-named-modules]`,
  which kiln gives too). A file that is not a kiln module file is an
  error. It is never ignored.
- `-fprebuilt-module-path=<dir>`: `<dir>/<name>.pcm` first (`M-P.pcm`
  for partition `M:P`), then the sources of §2.
- A `.pcm` input is compiled from the module file: `-c m.pcm`
  rebuilds the unit from its capsule and writes its object, as clang does
  in its two-phase build (libc++'s `format.py` adds `%T/std.pcm` as an
  input). The fingerprint is checked against the command line.
- `-x c++-module`, `.cppm`, `.ixx`: a C++ source, as in part A (whether a
  unit is an interface is decided by its module declaration, not its
  suffix).
- `-fmodule-header`, `-fmodule-header=user|system`, `.pcm` header units:
  step 2 (§11.10); still `error: unsupported option`.
- `-fkiln-bmi-kind=1|2` (hidden, WP-985): the module file kind written,
  for tests and measurements (§11.11 "Fallback").

Part A's single invocation (`kiln++ -fmodule-file=m=m.cppm m.cppm
main.cpp`) is unchanged.

### 11.7 Harness: two-step mode

`cargo xtask test` (spec/harness.md) drives separate compilation for both
the compiler under test and the clang oracle:

- `module: <name>=<path>` is allowed for `run` tests. Each unit is
  compiled in listed order, with the earlier units' `-fmodule-file=` flags,
  to an object and a module file
  (`-c -fmodule-output=<scratch>/<name>.pcm -o <scratch>/<name>.o`;
  clang: `--precompile` then `-c` of the `.pcm`), the test file is
  compiled to an object against the module files, and all objects are
  linked. A non-inline function defined only in an interface therefore
  links exactly once or the test fails.
- `unit: <path>`: an implementation unit (or any other TU) compiled with
  the same module flags after the module units and linked (`run` only).
- `two-step`: kiln, too, gets module files in a compile-kind test (it is
  given sources otherwise, so the existing tests keep testing part A).

The oracle's module files and objects are `oracle-<name>.pcm`/`.o` in the
same scratch directory. A module unit the compiler under test rejects (or
crashes on) fails the test; one the oracle rejects makes it broken. The
GCC oracle is not run for tests with `module:` or `unit:` lines (GCC's
module flags differ).

### 11.8 Cost of a capsule import

Measured after the implementation landed (release profile, aarch64, one
session, so the ratios are like for like; the absolute numbers are lower
than §11.1's, taken on a busier host), for the `import std;` program of
§11.1 against libc++ 19's `std.cppm`:

| run | time | peak RSS | vs part A |
|---|---|---|---|
| part A, `-fsyntax-only` | 1.01 s | 84.8 MiB | |
| capsule import (`-fmodule-file=std=std.pcm`), `-fsyntax-only` | 1.02 s | 89.6 MiB | 1.06x |
| part A, `-c` | 1.15 s | 85.4 MiB | |
| capsule import, `-c` | 1.12 s | 89.5 MiB | 1.05x |
| `--precompile std.cppm` (writes the 8.9 MB `std.pcm`) | 1.07 s | 96.8 MiB | |
| `-c -x c++-module std.cppm -fmodule-output=std.pcm` | 1.16 s | 96.8 MiB | |

The import is within the 1.2x budget; the remaining 5 MiB is the include
log (9012 entries for `std.cppm`) and its replay maps. Time is part A's:
a capsule import is O(parse) (§11.1). The importer's object defines only
`main`; `std`'s object owns the rest, and the two link and run. The writer
holds a second copy of every text (the recorder's) until the file is
written, hence the precompile's extra 12 MiB.

A first reader that read the whole file into one buffer peaked at
98-101 MiB (1.18x): freeing that 8.9 MB buffer raised glibc's dynamic
mmap threshold to its size, so the parser's later large allocations came
from the heap and were never returned. With the threshold pinned
(`MALLOC_MMAP_THRESHOLD_=131072`) the same reader measured 86 MiB. The
streaming reader (§11.3) never allocates a whole-file buffer.

### 11.9 Dependency scanning and CMake (WP-982)

CMake 3.28+ identifies kiln as clang (it defines `__clang__`; the version
it computes, `CMAKE_CXX_COMPILER_VERSION`, follows `__clang_major__`, so 22
by default and 19 under `-fclang-compat=19`) with the GNU front-end
variant, and for clang 16+ builds `FILE_SET CXX_MODULES` targets (Ninja
generator) in three steps per source, as `Modules/Compiler/Clang-CXX.cmake`
and `Clang-FindBinUtils.cmake` of CMake 3.31 spell them:

1. scan: `${CMAKE_CXX_COMPILER_CLANG_SCAN_DEPS} -format=p1689 --
   <compiler> <flags> -x c++ <src> -c -o <obj> -resource-dir <dir> -MT
   <obj>.ddi -MD -MF <obj>.ddi.d > <obj>.ddi.tmp`, P1689r5 on stdout;
2. collate: `cmake -E cmake_ninja_dyndep --modmapfmt=clang`, which writes
   one module map per source;
3. compile: `<compiler> <flags> -MD -MT <obj> -MF <obj>.d @<obj>.modmap
   -o <obj> -c <src>`, the map holding `-x c++-module` and
   `-fmodule-output=<bmi>` for an interface or partition and one
   `-fmodule-file=<name>=<bmi>` per module the source needs, transitively.

CMake finds the scanner with `find_program` for
`clang-scan-deps-<major>.<minor>`, `clang-scan-deps-<major>`,
`clang-scan-deps<major>` and `clang-scan-deps`, looking first in the
compiler's resolved directory and its spelled directory, then on `PATH`.

**Decision: one binary, dispatching on `argv[0]`.** `kiln-scan-deps` is
not a second executable: like `kiln++`, it is a link to the one `kiln`
binary, whose driver (`kiln_driver::scan_deps`) runs the scanner when the
program name is `kiln-scan-deps`, `clang-scan-deps` or
`clang-scan-deps-<version>`. A separate binary would duplicate the whole
front end (the scan is a run of kiln's own preprocessor, with its include
search, predefines and `-fclang-compat`, so the scan sees what the compile
sees) and double the install size; a link costs nothing and cannot drift
from the compiler it scans for. The install tree (`cargo xtask dist`,
spec/driver.md "Installation layout") and the dev tree (`kiln_driver`'s
build script, in the cargo profile directory) both hold `kiln-scan-deps`,
`clang-scan-deps-22` and `clang-scan-deps` beside `kiln++`, so CMake's
search finds kiln's scanner before any system clang's with nothing set.
To name it explicitly (another compat major, or a wrapper as
`CMAKE_CXX_COMPILER`), pass
`-DCMAKE_CXX_COMPILER_CLANG_SCAN_DEPS=<kiln prefix>/bin/kiln-scan-deps`.

The scanner behaves as clang-scan-deps 22 for the options CMake and
build tools pass (spec/driver.md "Dependency scanning"):

- The command after `--` is parsed by kiln's driver; it must be one
  compile of one input (otherwise clang's `unable to handle compilation,
  expected exactly one compiler job in '<command>'`).
- `-format=p1689`: the P1689r5 document, byte for byte clang's: keys
  sorted, two-space indent, `revision` 0, `version` 1, one rule with
  `primary-output` (the `-o` value, else `<stem>.o`), `provides` (an
  importable unit: `logical-name` (`M` or `M:P`), `is-interface` (false
  only for an implementation partition), `source-path` as given) and
  `requires` (each imported module once, in source order: `import :p;`
  in `M` requires `M:p`, an implementation unit `module M;` requires
  `M`); an empty array is omitted. `-format=make` (the default) prints
  the make rule. `-o <file>` writes the output to a file.
- `-MT`/`-MD`/`-MF` in the command write the dependency file of the files
  the scan read (spec/driver.md "Dependency output"), only after a clean
  scan.
- The scan runs the full preprocessor in module-scan mode
  (`kiln_pp::ModuleOptions::scan`): module and import directives are
  recorded instead of performed (no module file is read), and `#error`
  and `#warning` are skipped, as clang-scan-deps's source minimizer drops
  them; everything else (`#include`, conditionals, macros, `__has_include`)
  is evaluated, so a scan sees exactly the imports the compile will.
- A failed scan (a missing input, an unterminated conditional, an
  argument error) prints `Diagnostics while scanning dependencies for
  '<src>':` and the diagnostics (no caret snippet, no error count) to
  stderr, the empty document `{"revision": 0, "rules": [], "version": 1}`
  to stdout, writes no dependency file and exits 1.
- `import "h";` / `import <h>;` (a header unit) is `error: header unit "h"
  cannot be imported: kiln-scan-deps does not support header units yet`
  at the header name, a scan failure; header units are §11.10.
- No `--` is clang's `The compilation command line must be provided
  either via '-compilation-database' or after '--'.` (exit 1); an unknown
  `-format` value is `<prog>: for the --format option: Cannot find option
  named '<v>'`.

Deviations: the `-format=make` output names files as kiln's `-M` does
(`hdr.h`) where clang-scan-deps normalises them (`./hdr.h`); the
header-unit and one-job messages are kiln's (clang's header-unit error
says the header "is not known to be a header unit"; its one-job message
quotes its own expanded command); system headers in the dependency file
are spelled with kiln's toolchain paths. `-compilation-database` and
`-format=experimental-full` are not supported (an error).

Every flag CMake 3.31 puts on these lines was already kiln's (WP-917,
WP-799): `-x c++` / `-x c++-module`, `-fmodule-output=`,
`-fmodule-file=<name>=<bmi>`, the `@<file>.modmap` response file,
`-MD`/`-MT`/`-MF`, `-resource-dir`, `-std=gnu++20`, and at link
`-Xlinker --dependency-file=`. `-fmodules-reduced-bmi` (clang 18+; CMake
3.31 does not pass it, other build systems do) is accepted and ignored:
kiln's module file has one form, the source capsule (§11.1), which is
already as small as a reduced BMI gets.

The end-to-end check is `scripts/oss/selftest.sh` `cmake_modules`: an
interface, an interface partition, an implementation unit and a consumer
built through `FILE_SET CXX_MODULES` with Ninja and `kiln++` as the
compiler; the program runs, the scanner CMake found must be kiln's link,
and touching the interface recompiles it, the implementation unit and the
consumer but not the partition. `import std;` through CMake
(`CXX_MODULE_STD`) is §11.12.

### 11.10 Step 2 (follow-up work packages)

- **`import std;` module file.** Landed as §11.12 (WP-984).
- **Header units as files.** `-fmodule-header[=user|system]` writes a
  capsule of kind "header unit" (its exported macros are those of its
  replay, §3), `-fmodule-file=<hdr.pcm>` imports it, and `import "h";`
  finds it by the header's path.
- **Serialised-AST format (kind 2).** Designed in §11.11 "Kind 2: the
  serialised-AST module file (WP-985)". Part 1 (WP-985) carries the
  unit's self-contained constant results beside the capsule, so an
  importer does not run the Oven again for them. Part 2 is the
  declaration, type and expression image that replaces the replay (a
  reader that appends a unit's declarations with fresh `DeclId`s and its
  `UnitInfo`, with ODR merging in place of §6 dedup; lookup does not
  change, §1), and with it carried pointer, reflection and
  instantiation results (§11.11 "Part 2").

### 11.11 Kind 2: the serialised-AST module file (WP-985)

Kind 2 is the module file that lets an importer skip work the interface
unit already did. A full AST image (option (b) of §11.1) needs a
serialiser for every declaration, type, expression, statement, template
and evaluator object, and a reader that remaps every id. That is several
work packages. **Part 1 (WP-985) is a clean cut**: a kind-2 file is the
kind-1 capsule plus *sections*, and the one section it defines carries the
unit's self-contained constant-evaluation results. The importer still
replays the capsule (§11.2), so every declaration, lookup and diagnostic
is exactly part A's. It no longer runs the Oven for an evaluation whose
result the file carries. That is the work kiln most wants to avoid
repeating, and the constexpr report shows when it is avoided. Part 2
replaces the replay with the image designed below, behind the same
section framing.

#### Inventory

What an image must hold, the id it is keyed by, and where it stands:

| state | id / owner | part 1 | part 2 |
|---|---|---|---|
| declarations, redeclaration groups, `UnitInfo` (owner, attachment, visibility) | `DeclId`, `RedeclId`; `Ast`, `Ast::modules` | replayed | image, appended (see "Identity") |
| types | `QualType` into the type context | replayed; a carried value's type is its expression's | type table, re-interned |
| expressions, statements, attributes, lists | `ExprId`, `StmtId`, `AttrId`, `ListId` | replayed | bodies the importer can use (inline, constexpr, templates), per declaration |
| template specializations and implicit instantiations | sema's instantiation tables | replayed | carried, merged by (template, arguments) |
| constant values | `ConstValueId` in the `ConstStore` | **carried**: integers, enumerators, `bool`, floating point, null pointers, aggregates without pointers or unions | pointers, references, member pointers, reflections (they name declarations) |
| Oven state (frozen heap objects, memo tables) | the Oven | not carried | frozen objects as constant values; memo tables stay per TU |
| source positions | `FileId`, `Loc` | keys only (see "Identity") | location table over the capsule's file table |
| identifiers | `Symbol` (`kiln_source::Interner`) | replayed | string table, re-interned |
| macros, include state, pragmas | `kiln_pp` | replayed (the capsule) | replayed: a named module exports no macros (§3), and the capsule stays the source of truth for diagnostics |
| lookup tables, scopes | sema | rebuilt by the replay | rebuilt from the appended declarations (§1: lookup does not change) |

#### Identity and remapping

Part 1 needs no id remapping. A carried result is keyed by *where* the
evaluation happens, which the replay reproduces exactly:

- `kind`: one byte, the `EvalKind` (static assertion, initializer,
  template argument, immediate invocation, `if constexpr`, constraint,
  other) with bit 4 = the enum range check and bit 5 = "evaluates an
  object's initializer". These are the context fields that change a
  result. A consteval block (it can inject declarations) and a `plain`
  evaluation are never carried.
- two positions, the construct's (`EvalContext::loc`) and the evaluated
  expression's first token, each as (path index, offset) of its expansion
  location and then of its spelling location. Paths are the source
  manager's names, stored once in the section's path table. A file named
  `<...>` (the `<built-in>` buffer, scratch buffers) or with no name
  gives no key: the replay does not reproduce its offsets.

The value's type is not stored. It is the canonical, unqualified type of
the evaluated expression; the writer checks that it is, and the reader
takes it from its own replayed copy of the expression. A result is used
only while the importer reads the unit the file holds
(`module_current_unit` is that unit's and no instantiation is in
progress), so the same header text read by the importer itself never
matches.

Part 2's remapping, one table per id space, built as the image loads:

- `DeclId`: the file numbers the unit's declarations 0..n. The reader
  appends them, so local index i becomes `base + i`. A reference to
  another unit's declaration is (module key, local index), resolved
  through that module's load; a reference to a global-module-fragment or
  header entity is an ODR key (enclosing context's key, name, kind,
  signature), resolved by lookup.
- `QualType`: a type table in dependency order. Each entry is the type's
  structure with remapped operands, re-interned through the type context,
  so the importer's canonical types stay canonical. cv bits stay on the
  `QualType`.
- `ExprId` / `StmtId`: appended with a base offset, written only for
  bodies an importer can use, and loaded per declaration.
- `FileId` / `Loc`: (file index, offset) into the capsule's file table.
  Files load through `load_regular_file_expecting` as today (§11.2), and
  macro expansions are rebuilt from recorded expansion records only when
  a diagnostic needs them.
- `Symbol`: a string table, interned once per load.
- Constant values: part 1's encoding plus pointer (remapped base
  declaration and path), member pointer and reflection (remapped entity).

**§6 dedup becomes ODR merging.** Today an importer's and a unit's
copies of a GMF entity are merged because both come from the same
`FileId` and offset (§6, "Merging by entity"). With an image there is no
second parse to compare against. A GMF declaration in the file merges
into the importer's redeclaration group with the same ODR key; two
definitions are compared by a structural hash and, if they differ, give
clang's "different definitions in different modules" error. Implicit
instantiations merge the same way, keyed by (template, remapped
arguments), and keep the first result. This is also how a carried
instantiation's result reaches an importer that instantiates the same
specialization.

#### Encoding

All in safe Rust in existing crates (no new crate, no new dependency).
`kiln_pp::bmi` frames sections (§11.3: a count, then strictly increasing
tags, each with its length) as opaque bytes. Their contents belong to the
crate that reads them: the carried constants are `kiln_sema::bmi_ast`.
Section 1, little-endian:

```
paths    u32 count, then each path (u32 length, UTF-8)
entries  u32 count (at least 1), then each entry:
  kind   u8      (see "Identity"; low nibble 0..6, bits 6-7 zero)
  at     4 x u32 (expansion path index, offset, spelling path index, offset)
  expr   4 x u32 (the same, for the expression's first token)
  value  u8 tag, then
         0 integer:   u8 signed, u32 width (1..=2^24-1), ceil(width/64) x u64 words
         1 bool:      u8 0|1
         2 float:     u8 kind (FloatKind), u128 bits (no bits above the kind's width)
         3 null pointer
         4 aggregate: bytes (u32 length), u32 padding count, then
                      (u32 start, u32 length) ranges, sorted, non-empty,
                      not adjacent, within the bytes
```

- **Deterministic.** Paths are in first-use order and entries in
  first-evaluation order (no hash-map iteration), so the same inputs give
  the same bytes, and the driver test checks this.
- **Hardened reader.** `bmi_ast::decode` checks every count against the
  bytes left before sizing anything by it, validates UTF-8, flags,
  widths, words (`IntBits::from_words`: no bits above the width),
  float bits, padding ranges, path indices, duplicate keys and trailing
  bytes. Any failure is a `SectionError`, turned into §11.4's
  `fatal error: module file 'x.pcm' is corrupt: carried constants:
  <reason>`. The framing errors (`a kind-2 file without sections`,
  `unknown section`, `sections out of order`) are `BmiError`s with the
  same diagnostic. A decoded value that does not fit its expression's
  type (width, signedness, float kind, `bool`, pointer type, aggregate
  size), or that the constant store rejects as non-canonical, is not
  used: the evaluation runs. The whole file is still covered by the
  checksum (§11.3), so a damaged section is only seen when the checksum
  was recomputed. Nothing panics on file contents (the fuzz tests below).

#### Laziness, measured

Part 1 decodes the section eagerly, once per imported unit, when the
replay enters the unit, into one hash map. Laziness does not pay off
here: the replay makes every evaluation the table can answer, in order.
For libc++ 19's `std` the section is 139,200 bytes (1.5% of the file,
2900 entries, 48 bytes each). Its decoding cost is below the measurement
noise: with kind-1 and kind-2 imports interleaved, kind 2 is never slower
(`-c` median of 5 runs, 1.43 s against 1.46 s), and its peak memory is
the same within 1 MiB. Part 2 must be lazy: an offset table per
declaration, with bodies, specializations and constant values loaded on
first use. Its target is clang's import cost (0.23 s, measured below),
and it is measured the same way.

#### Constant state keyed by the fingerprint

A carried result is valid only for the compile that produced it. The
section travels inside the module file, whose fingerprint (§11.4) and
kiln version are checked before any section is read. So a result is used
only under the same `-std=`, target, exceptions, RTTI, `char`
signedness, `char8_t`, sized deallocation, freestanding, reflection
flags, `-fclang-compat=` and kiln build, and with the writer's
predefines and file texts (the replay is hermetic, §11.2). The kiln
version check covers the Oven: a kiln that evaluates differently is a
different version. Two things outside the fingerprint are deliberately
the writer's, as with clang's BMI, which stores evaluated values:

- the evaluation limits (`-fconstexpr-steps=`, `-fconstexpr-depth=`): a
  result the writer computed is used even if the importer's limit is
  lower;
- `__DATE__` / `__TIME__`: a carried value derived from them keeps the
  writer's.

#### What is carried (the writer)

The driver turns recording on only for a compile that writes a module
file and does not force kind 1. Sema then records every evaluation
`eval_const` makes while it reads the main unit (`module_current_unit()
== 0`), and keeps a result only if all of these hold:

- no template instantiation is in progress (one position, many results);
- a key exists (see "Identity");
- the evaluation is pure: it added no declaration, expression or
  statement and emitted no error or warning (a carried result would
  silence a diagnostic the importer must give, and an evaluation that
  instantiated a template must still instantiate it);
- it succeeded, its value's type is its expression's, and the value is
  self-contained (see "Inventory");
- no other evaluation had the same key.

The rest are evaluated again by the importer, as in kind 1.

#### Fallback

The default writes kind 2 when at least one result is carried, and kind 1
otherwise. The hidden `-fkiln-bmi-kind=1` always writes kind 1, and `=2`
is the default. The choice is never a warning. Under `-v` the driver
prints one of:

```
note: wrote module file 'm.pcm' as kind 2, carrying N constant results
note: wrote module file 'm.pcm' as kind 1: no constant result to carry (first not carried: <reason>)
note: wrote module file 'm.pcm' as kind 1, as -fkiln-bmi-kind=1 requests
```

The `<reason>` is the first evaluation that was not carried, one of: `an
evaluation inside a template instantiation`, `a consteval block`, `an
evaluation at a position a replay does not reproduce`, `an evaluation that
changed the AST`, `an evaluation that emitted a diagnostic`, `an
evaluation that failed`, `a value whose type is not its expression's`, `a
pointer or reference value`, `a pointer-to-member value`, `a reflection
value`, `an aggregate holding a pointer`, `an aggregate holding a union`,
`two evaluations at one position`. When the unit made no evaluation at
all, the parenthesis is left out. The reader accepts both kinds, so
mixing them in one build is fine.

On libc++ 19's `std.cppm`, 2900 results are carried. Of the evaluations
that are not carried, 10142 happen inside a template instantiation, 141
are two evaluations at one position, 74 failed, 23
are at positions a replay does not reproduce, 13 have a value whose type
is not its expression's, 6 are aggregates holding a pointer, and one
changed the AST. libc++ 22 gives the same mix (2961 carried, 9637 inside
instantiations). Of the 28 interface-unit builds in the 16 `module:` tests, 3 are kind 2
(`gmi_inner`, `gmi_outer`, and `two-step-visibility`'s `m`). The other 25
make no constant evaluation at all and are kind 1.

#### Measurements

Harness profile, aarch64, one session, `/usr/bin/time -v` (wall time and
peak RSS, median of 3; kind-1/kind-2 `-c` imports median of 5,
interleaved). Workloads: the `import std;` program of §11.1 against
libc++ 19's `std.cppm`, and a synthetic constexpr-heavy interface (eight
`constexpr long` initializers of 500,000 loop iterations each and a
512-entry prime table as a `constexpr` aggregate, with a
`static_assert`) whose importer reads them in a `static_assert` and
`main`.

| run | `import std;` (libc++ 19) | constexpr-heavy module |
|---|---|---|
| kiln `--precompile`, kind 1 | 1.05 s, 97.7 MiB, 8,990,697 B | 0.25 s, 10.8 MiB, 23,384 B |
| kiln `--precompile`, kind 2 | 1.06 s, 99.5 MiB, 9,129,897 B | 0.25 s, 10.7 MiB, 25,966 B |
| kiln import, kind 1, `-fsyntax-only` | 1.12 s, 95.9 MiB | 0.25 s, 10.5 MiB |
| kiln import, kind 2, `-fsyntax-only` | 1.09 s, 95.6 MiB | **0.02 s**, 10.3 MiB |
| kiln import, kind 1, `-c` | 1.46 s, 96.1 MiB | 0.27 s, 60.6 MiB |
| kiln import, kind 2, `-c` | 1.43 s, 97.0 MiB | **0.05 s**, 60.6 MiB |
| kiln part A (`-fmodule-file=std=std.cppm`), `-fsyntax-only` | 1.13 s, 91.1 MiB | |
| clang 19 `--precompile` | 1.55 s, 261.7 MiB, 31,255,932 B | 4.80 s, 85.3 MiB |
| clang 19 import, `-fsyntax-only` | 0.23 s, 141.1 MiB | 0.06 s, 84.6 MiB |
| clang 19 import, `-c` | 0.46 s, 158.9 MiB | |
| clang 22 `--precompile` (libc++ 19 headers) | 1.71 s, 294.8 MiB, 32,373,840 B | 4.67 s, 88.0 MiB |
| clang 22 import, `-fsyntax-only` | 0.40 s, 169.3 MiB | 0.06 s, 86.5 MiB |
| clang 22 import, `-c` | 0.56 s, 184.9 MiB | |

The constant state carried, from the importer's `-fconstexpr-report=`:

- constexpr-heavy module: kind 1 re-evaluates 4,074,689 Oven steps, and
  kind 2 evaluates nothing (0 steps, 0 evaluations), so the import drops
  from 0.25 s to 0.02 s and becomes faster than clang's 0.06 s;
- `std`: an importer makes 17,222 evaluations instead of 20,119. `std`'s
  own constant work is small (618 steps, 17 ms in all), so its import
  time is still the replay's (§11.8), which is part 2's to remove.

With libc++ 22's `std.cppm`, kind 2 carries 2961 results (10,655,331 B
against 10,485,735 B), and the imports again match kind 1 within noise
(`-c` 1.94 s against 2.02 s, 105 MiB).

#### Part 2

Next, in order:

1. carried results for evaluations inside instantiations, keyed by the
   instantiation (template, remapped arguments) rather than the position:
   the 10,142 above, and the bulk of a template-heavy constexpr workload;
2. pointer, reference and reflection values (they need `DeclId`
   remapping);
3. the declaration and type image with ODR merging (see "Identity"),
   loaded lazily, which retires the replay and brings `import std;`
   towards clang's 0.23 s.

The section framing, the fallback note, the harness variant and the fuzz
tests are reused as they are.

### 11.12 `import std;` module files: the cache and CMake (WP-984)

`import std;` and `import std.compat;` worked from the start (§8): with
nothing on the command line naming the module, kiln builds libc++'s
`std.cppm` in-process. WP-984 gives that path a module file. It is the
same file `kiln++ --precompile std.cppm` writes, cached per toolchain
configuration. The two-step and build-system routes are also covered:
`-fmodule-file=std=` with a kiln-built `std.pcm`, and CMake's
`CXX_MODULE_STD`.

**Where the cache applies.** Only at lookup step 4 (spec/driver.md
"Modules"): `std` or `std.compat` not supplied by `-fmodule-file=`, a
prebuilt path or a replayed module file. `kiln_pp` asks the hook
`ModuleOptions::std_cache` (`StdModuleCache`, a closure the driver
builds in `module_cache::for_job`) with the module's name and libc++
source. A returned path is imported as a module file; `None` means
building from source, unchanged (part A). A module file records the
source path of `std` it imports, never the cache path. So an importer of
a cached `std.compat` looks `std` up in the cache again and does not
depend on where an earlier compile put it. The cache is off during
dependency scanning, which imports nothing.

**Key.** The entry directory is a 128-bit hash, as 32 hex digits. It is
two FNV-1a-64 variants, and it covers everything that decides the
module file's bytes:

- kiln's identity: version, commit, and the executable's path, size and
  mtime, so a rebuilt kiln never reads an older one's files;
- the §11.4 fingerprint pairs;
- the predefines text (every `-D`, `-U`, `-include` and target macro);
- the header search list (the `-v` text);
- the libc++ module directory and the bytes of `std.cppm` and
  `std.compat.cppm`.

Every write ends with a separator, and the variable-length fields are
length-prefixed, so no two configurations collide by concatenation.
`-std=` and `-fno-exceptions` change the key through the fingerprint.
`-O` changes it through the predefines (`__OPTIMIZE__`), so `-O0` and
`-O2` builds have separate entries. `-g`, `-fPIC` and warnings do not
change it.

**Stamp.** Beside `<name>.pcm`, `<name>.stamp` holds a magic line, the
module file's name, and one `<size> <mtime> <path>` line for every file
the build read. These are the dependency list of an `-MD` with system
headers, plus the source. A hit needs the stamp valid, every file
unchanged, and the module file's first bytes kiln's magic. So a libc++
header edited in place, or a damaged module file, is a miss. This is
clang's module-cache validation (size and mtime, not content).

**Miss.** The driver runs `--precompile` of the libc++ source in-process
on its own thread, with the importer's stack size and stack guard. The
nested job copies the importer's arguments and changes only these:

- the action becomes `--precompile`, writing to `<name>.pcm` in the
  entry;
- inputs, module files and prebuilt paths are cleared;
- dependency output, `-verify`, dumps, statistics and `-v` are off.

`std.compat` imports `std` through the same hook, so the first
`import std.compat;` fills both entries. Diagnostics of the nested
build go to a buffer. A build with errors removes its empty entry and
falls back to part A, which reports the module's errors in the
importer's context, as before WP-984.

**Concurrency.** The module file goes to a unique temporary file
(`.tmp.<pid>.<thread>.<nanos>`) and is renamed into place, then the
stamp the same way. A reader reads the stamp first and the module file
second, so it never sees a torn file. Compiles that miss together all
build, all rename, and all succeed. The builds are deterministic, so
any of the files is right. No lock is taken, so a stuck compile cannot
block others.

**Fallback.** An unwritable root (a read-only directory, a path through
a regular file), a failed build, or a failed stamp write is never an
error. The compile builds the module in-process (part A) and says so in
one `-v` line (spec/driver.md "Module cache").

**Cost** (aarch64, `harness` profile, `/usr/bin/time -v`, median of
three). The program is `import std;` plus a `main` using `vector`,
`accumulate` and `println`, compiled with `-std=c++23 -stdlib=libc++ -c`:

| route | wall | peak RSS |
|---|---|---|
| cold cache (builds `std.pcm`, 8.6 MiB, then imports it) | 2.45 s | 139 MB |
| warm cache | 1.49 s | 98 MB |
| part A (`-fno-kiln-module-cache`) | 1.52 s | 95 MB |
| kiln two-step: `--precompile std.cppm` / importer `-c` | 1.02 s / 1.49 s | 102 / 98 MB |
| clang++-22 two-step: `--precompile std.cppm` / importer `-c` | 1.62 s / 0.47 s | 302 / 178 MB |

A warm hit costs what part A costs, because a source capsule is replayed
(§11.8). The cache becomes a speed-up with the serialised-AST format
(kind 2, §11.10), which the cache stores unchanged since it stores
whatever `--precompile` writes. Its value today is the build-system
surface: one `std.pcm` per configuration, and the hook kind 2 needs.

**CMake.** `CXX_MODULE_STD` (CMake 3.30+) reads
`<compiler> -print-file-name=libc++.modules.json`. kiln answers with the
manifest of the libc++ that `-stdlib=libc++` selects. It takes
`<prefix>/include/c++/v1`, then looks for
`<prefix>/lib/<triple>/libc++.modules.json`, then
`<prefix>/lib/libc++.modules.json`. libc++ 19 here is
`/usr/lib/llvm-19/lib/libc++.modules.json`, whose `source-path` entries
are relative to it (`../share/libc++/v1/std.cppm`). CMake then builds
`std.cppm` as a module of a synthesized target, scanned by
`kiln-scan-deps` and compiled by kiln like any other interface.

Three things are needed:

- CMake's experimental gate, set before `project()`, a per-release UUID:
  - 3.30 and 3.31: `0e5b6991-d74f-4b3d-a41c-cf096e0b2508`
  - 4.0: `a9e1cf81-9932-4810-974b-6eccaf14e457`
  - 4.1 and 4.2: `d0edc3af-4c50-42ea-a356-e2862fe7a444`
- `-stdlib=libc++`;
- `CXX_STANDARD 23` or later.

Tested with CMake 3.31.6 and Ninja 1.12.1:
`scripts/oss/fixtures/modules` `stdapp` runs, and touching the
interface or `stdapp`'s source never rebuilds the std module.

**Not done.**

- No eviction: dead entries stay until removed by hand. Each entry is
  about 9 MiB, and every kiln rebuild changes the key.
- `-MD` on a hit still lists the libc++ headers the module read, as
  part A does. This is correct for make, and a capsule replays them.

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
  (self-checking `run` tests). Since WP-984, all but `vector` name
  libc++'s sources with `module: std=%{libcxx_modules}/std.cppm` (and
  `std.compat`), so they run two-step under clang too (spec/harness.md
  "Modules"). `vector` stays the test of kiln's implicit lookup (§11.12)
  and is `skip(clang)`.
- `module.import/import-std-two-step.cpp` (WP-984): `import std;` with
  `std.pcm` built by a separate `--precompile`, under kiln and clang 22.
- `crates/kiln_driver/tests/module_cache.rs` (WP-984): the cache of
  §11.12 against a miniature libc++ sysroot. It covers:
  - a miss then a hit;
  - `std.compat` filling both entries;
  - invalidation by a header edit, a damaged module file, a new
    `std.cppm`, `-D` and `-fno-exceptions`;
  - four racing misses;
  - an unwritable root;
  - location and switches;
  - `-print-file-name=libc++.modules.json`.
- `scripts/oss/selftest.sh` `cmake_modules`: `CXX_MODULE_STD` through
  CMake (§11.12).
- libc++'s `libcxx/selftest/modules/*.sh.cpp`: `format.py`'s module
  build (§8).
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
- Part B (WP-917), two-step `run` tests (each module unit compiled to a
  module file and an object, the importer and `unit:` files compiled
  separately, then linked; under kiln and clang 19/22):
  `module.interface/two-step-run.cpp` (non-inline function and variable
  defined once, exported inline variable, exported function and class
  templates instantiated by the importers, a static data member, two
  importers), `module.unit/two-step-partitions-run.cpp` (interface and
  implementation partitions, an implementation unit),
  `module.unit/two-step-impl-unit-run.cpp`,
  `basic.start.dynamic/two-step-module-init-run.cpp` (observable
  initialization order, once only), `module.global.frag/two-step-gmf-dup-run.cpp`
  (kiln only: clang emits the GMF class's vtable in every importer,
  deviation 7), and `module.interface/two-step-visibility.cpp` (the
  visibility diagnostics through a module file).
- `crates/kiln_pp/src/bmi.rs` unit tests: round trip, foreign and
  version-mismatched files, every truncation and single-byte flip of a
  small capsule (a diagnostic, never a panic), a damaged body behind a
  valid checksum, the fingerprint comparison.
- `crates/kiln_driver/tests/modules_bmi.rs`: `--precompile` determinism
  and the three lookups (named, prebuilt path, nameless with its
  warning), `-fmodule-output` (default and explicit paths, same bytes as
  `--precompile`), `-c m.pcm` and a two-step link, the unused-argument
  warning and the `--precompile` error, the mismatch, missing,
  wrong-module and clang-file diagnostics, and 40 truncated or flipped
  module files through the driver (each a fatal error, exit code 1).
- Dependency scanning (WP-982, §11.9): `crates/kiln_driver/tests/scan_deps.rs`
  compares the P1689r5 document with clang-scan-deps 22's bytes for a
  primary interface importing its partitions (with a GMF `#include`
  and the dependency file), an interface and an implementation
  partition, an implementation unit, a consumer and a plain TU
  (`<stem>.o`), a skipped `#error`, the header-unit and missing-input
  failures (stderr, empty document, exit 1, no dependency file), `-o`,
  the default make format, the missing `--` and bad `-format` messages,
  through each scanner name; `kiln_driver::scan_deps` unit tests cover
  the names, option parsing and JSON quoting. `xtask/src/dist.rs`
  checks the links in the install layout and scans a unit with the
  installed `clang-scan-deps-22` in its smoke test.
- CMake (WP-982): `scripts/oss/selftest.sh` `cmake_modules` builds
  `scripts/oss/fixtures/modules` (§11.9).
- Kind 2 (WP-985, §11.11): `crates/kiln_sema/src/bmi_ast.rs` unit
  tests (the section's round trip, no bytes without a carried entry,
  trailing bytes and an out-of-range path index rejected, and a fuzz
  test: every truncation rejected, then five flips of every byte, four
  `u32` values (large, zero) written at every offset and 4000
  deterministic random multi-byte mutations, each decoded without a
  panic); `crates/kiln_pp/src/bmi.rs` `kind2_*` (the framing round trip,
  every truncation and byte flip of a kind-2 file, an empty section list,
  tag 0, an unknown tag, a repeated tag, sections after a kind-1 body, an
  unknown kind);
  `crates/kiln_driver/tests/modules_bmi.rs` `kind2_carries_constant_results`
  (the `-v` notes, the kind byte, determinism, the importer's constexpr
  report showing no evaluation of the unit under kind 2 and its full
  re-evaluation under kind 1, linked programs and `-c m.pcm` under both
  kinds), `kind2_falls_back_to_kind1` (the fallback reason and the forced
  kind) and `kind2_damaged_files_are_diagnosed` (truncated and flipped
  kind-2 files are fatal errors, and 24 damaged sections behind a valid
  checksum give a diagnostic or a fallback evaluation, never a panic);
  and every `module:` test under `cargo xtask test --bmi-kind 1` and
  `--bmi-kind 2` (spec/harness.md "Module-file kinds"), both in `ci`.
