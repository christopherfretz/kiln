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
parsed `ModuleDirective`. The directive ends at its `;`: the rest of that
line is ordinary text, as clang lexes it (`import M; int x;` declares
`x`; GCC 16 rejects it, "expected end of line"). Only an invalid
directive discards the rest of its line (WP-1198, which fixed kiln
dropping it silently):

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
| [module.interface]/1, /6: an export declares namespace members only (WP-1159) | `cannot export 'x' as it is not at namespace scope`, at the declarator-id (a tag's name), for a qualified definition or redeclaration of a class member (static data member, member function, member function template, nested class or enumeration, of a class or class template) directly in an export (through `export { }` and `extern "C++"`, not through an exported namespace); a template-id declarator (`export template<> void S::g<int>()`) is exempt, a namespace member's qualified redeclaration is checked by the module-linkage rule below instead (with no `-Wextra-qualification`), and `export void ::f()` (also `export struct ::X {}`, which is then still checked as an exported redeclaration, WP-1174) is `definition or redeclaration of 'f' cannot name the global scope`, as clang (`diagnoseQualifiedDeclaration`). The member is still declared and defined as written; clang drops a member function or variable (so its body's uses of the class's members are `use of undeclared identifier`) and an enumeration's enumerators, kiln does not cascade. Deviation: `namespace B { export void A::g() {} }` keeps kiln's `cannot define or redeclare 'g' here because namespace 'B' does not enclose namespace 'A'`, which clang 19/22 miss |
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
importer although the class template, declared in std's GMF, is not. A
purview shadow is filtered the same way, so the shadow of a non-exported
using-declaration is invisible even when its target is visible (WP-1068).
A global-module shadow (a header's `using ::size_t;`) is exempt, like every
implicit declaration, since an importer's own `#include` of the guarded
header does not redeclare it (`module.import/import-std-compat.cpp`).

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
to a different module is the [basic.link]/11 error. A declaration an
instantiation made (a befriended function template of `It<char>`
instantiated in an importer) is attached as the declaration it was
instantiated from (`instantiated_from`), not as the unit performing the
instantiation (WP-1314; `module.unit/friend-template-instantiated-attachment.cpp`).

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
merge, and neither is a global-module declaration of a non-module unit or
header unit the current unit does not import: the importer's own
declarations before the `import` that builds a module unit in-process
belong, for that unit, to another translation unit (WP-1156). Both are
*hidden global* declarations (`module_hidden_global`), so:

- *Lookup* takes the found declaration when it is visible, else a visible
  redeclaration of the same entity (`module_visible_redecl`, clang
  `findAcceptableDecl`; a template's through its pattern's chain): the
  lookup table holds only the latest redeclaration, which may be the
  hidden one.
- *A redefinition* whose previous definition is such a hidden global
  declaration is not diagnosed: the previous definition (and the template
  it describes) is merged into the current unit (`module_merge_hidden_
  definition`). A class or enumeration body is then skipped like a
  redefinition's (§4.4 of spec/sema.md), and so is the body of a class
  template's explicit or partial specialization ([temp.expl.spec]/1,
  [temp.spec.partial.general]/1; WP-1156: libstdc++'s
  `numeric_limits<_Float32>`, whose tokens differ between units once
  glibc's `<math.h>` has defined `__builtin_nansf32` as a macro, so the
  position check above cannot match it); a variable, a variable template
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
every other unit declares it `external`, at every optimization level:
an importer never copies it `available_externally` (clang
`canSpeculativelyEmitVTable` refuses a class whose definition comes from
a named module, `Decl::shouldEmitInExternalSource`; WP-1198). A
global-module-fragment class with a key function gets its vtable from
the unit defining the key function, the module unit itself (deviation
7); at -O1 an importer may copy it `available_externally`, and its
destructor slots then name the `D1`/`D0` declarations. Any reference to
a constructor or destructor variant whose non-inline definition an
imported unit owns is such a declaration from its first use (never a
complete-object alias this unit would define later). The implicit global
`operator new`/`operator delete` declarations are attached to the global
module ([basic.stc.dynamic.general]/2) even when the first use is in a
purview, so they are never `_ZW`-mangled.

**Explicit instantiations** ([temp.explicit], [temp.spec.general]/5,
WP-1354; the PCH and module-file image side is WP-1307 in §11.11). The
unit that writes an explicit instantiation definition owns the
specialization's definition, whichever unit declared the template or an
`extern template` for it. Sema records it (`ModuleTable::explicit_defs`,
keyed by the first declaration of the function, variable or class
specialization, with the current unit; `kiln_sema`
`module_note_explicit_inst_def`). Codegen asks
`explicit_def_imported` for the governing explicit instantiation (the
entity's own, or its enclosing class specialization's,
`governing_explicit_inst`): an imported named unit (purview or GMF)
means declare only, at every optimization level (never
`available_externally`, even for an inline member at -O1, as clang);
this unit or a header unit means define `weak_odr`. With no record (a
module-file import) the owner is the unit that owns the governing
explicit instantiation declaration. The table, for a function template,
member function, static data member, variable template and whole class,
in a named-module purview, its GMF or a header unit (clang 22 matches
every row except those marked; tests in `module.interface/explicit-inst-*`
and `codegen/module_explicit_inst_importer.cpp`):

| module | importer | importer emits |
|---|---|---|
| `extern template` | definition | `weak_odr` definition |
| `extern template` | declaration, or implicit use | declaration (ill-formed NDR unless defined elsewhere) |
| nothing | definition | `weak_odr` definition |
| nothing | declaration | declaration |
| nothing | implicit use | `linkonce_odr` |
| definition | implicit use | declaration; the module object has `weak_odr` (a header unit: `weak_odr` in the importer) |
| definition | definition | error: duplicate explicit instantiation ([temp.spec.general]/5) |
| definition | declaration | error: explicit instantiation declaration follows definition |

The two errors also hold for a static data member outside modules
(they used to be accepted silently). clang 22 rejects valid code in two
rows that kiln accepts: an importer's explicit instantiation definition
(and clang 19, any implicit instantiation) of a named-module static data
member or whole class ("declaration of 'v' in the global module follows
declaration in module"), and an explicit instantiation naming a function
template through an exported using-declaration of a GMF template.
Ordering: the vtables of imported explicit-instantiation classes are
emitted after the deferred definitions (clang's order), so
`run_tu` drains `imported_explicit_vtables` after `emit_deferred`.
Known difference: for a GMF class whose explicit instantiation
definition is in the module unit, kiln's module object also defines the
vtable `weak_odr` where clang's does not (follow-up).

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
  compile with errors writes no module file (and removes nothing). An
  output path that exists and is not a regular file (`-o /dev/null`, a
  FIFO) is written in place, as clang does: renaming over it would
  replace the device with a regular file (WP-1281). The same holds for
  the `-emit-pch` record and the module cache's stamps.

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

### 11.9 Dependency scanning and CMake (WP-982, WP-1148)

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
`clang-scan-deps-22`, `clang-scan-deps-19` (the name CMake looks for
under `-fclang-compat=19`, WP-1148) and `clang-scan-deps` beside
`kiln++`, so CMake's search finds kiln's scanner before any system
clang's with nothing set. To name it explicitly (a wrapper as
`CMAKE_CXX_COMPILER`, as the OSS harness's kiln-cxx), pass
`-DCMAKE_CXX_COMPILER_CLANG_SCAN_DEPS=<kiln prefix>/bin/kiln-scan-deps`.

**Flavour.** Under the program name `clang-scan-deps-19` (or
`clang-scan-deps-19.<minor>`) the scanner behaves as clang-scan-deps 19
where 19 and 22 differ: the failure header (`Error while scanning
dependencies for <src>:`, unquoted, in place of 22's `Diagnostics while
scanning dependencies for '<src>':`) and the `-format=experimental-full`
shape below. Every other name behaves as 22.

The scanner behaves as clang-scan-deps 22 for the options CMake and
build tools pass (spec/driver.md "Dependency scanning"):

- The command after `--` (or each entry of `-compilation-database`) is
  parsed by kiln's driver; it must be one compile of one input (otherwise clang's `unable to handle compilation,
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
- No `--` and no `-compilation-database`, or both, is clang's `The
  compilation command line must be provided either via
  '-compilation-database' or after '--'.` (exit 1); an unknown `-format`
  value is `<prog>: for the --format option: Cannot find option named
  '<v>'`.
- `-compilation-database <file>` (WP-1148) scans every entry of a JSON
  compilation database, as clang-scan-deps 19 and 22 do. Each entry is
  an object with `directory`, `file` and either `arguments` (a list,
  which wins when both are present) or `command` (one string, split as a
  POSIX shell does: quotes, backslash escapes, no expansion), and
  optionally `output`; a missing key is checked in the order `file`,
  `command`/`arguments`, `directory`, and an unknown key is an error,
  with clang's wording (`Error while opening JSON database: <reason>` on
  stderr, exit 1, nothing on stdout: `Expected array.`, `Expected
  object.`, `Missing key: "file".`, `Missing key: "command" or
  "arguments".`, `Missing key: "directory".`, `Expected sequence as
  value.`, `Expected string as value.`, `Unknown key: ""<k>""`, or the
  open error). Trailing commas are accepted, a scalar is taken by its
  text and a repeated key keeps its first value, as LLVM's YAML-based
  JSON reader does; `[]` is a clean empty scan. Each entry is scanned
  in its `directory` (relative to the scanner's working directory),
  which is restored before the output is written. The outputs, in
  entry order: `make` concatenates one rule per entry, its files
  spelled as the directory joined with the path (so `./p.cppm` for
  `"directory": "."`); `p1689` is one document whose rules are sorted
  (stably) by `primary-output` (the entry's `output`, else empty), each
  `source-path` is the entry's `file`, and a `requires` of a module
  another rule provides gets that rule's `source-path`;
  `experimental-full` is one translation unit per entry. A failed entry
  prints its diagnostics under the header, is left out of the output
  (an empty `commands` in `experimental-full`), and makes the exit
  status 1; the other entries are still scanned.
- `-format=experimental-full` (WP-1148) is clang's build-system JSON:
  `{"modules": [], "translation-units": [...]}` with one element per
  input, each `{"commands": [{...}]}` holding `clang-context-hash`,
  `clang-module-deps` (always `[]`: kiln scans no Clang module maps),
  `command-line` (the compile's arguments after the program name),
  `executable` (the program, resolved on `PATH` when it has no `/`),
  `file-deps` (every file the scan read, absolute: relative ones joined
  to `<cwd>/.` after `--`, as clang spells them, or to the entry's
  directory), and `input-file` (as given); for clang 22 also
  `named-module` (the unit's module, `M` or `M:P`, for an interface or
  any partition) and `named-module-deps` (the imports, as in `requires`;
  omitted when empty). The 22 flavour prints the keys in that streaming
  order, `named-module` and `named-module-deps` before
  `clang-module-deps`, with no newline at the end; the 19 flavour sorts
  the keys, has no named-module fields and ends with a newline. Both use
  LLVM's two-space JSON layout. `clang-context-hash` is kiln's: the
  FNV-1a 64-bit hash of the kiln version and the compile's arguments
  without its inputs, `-o`, the dependency-file options and their
  values, printed in uppercase base 36, so two compiles that would build
  the same module agree.

Deviations: the `-format=make` output names files as kiln's `-M` does
(`hdr.h`) where clang-scan-deps normalises them (`./hdr.h`); the
header-unit and one-job messages are kiln's (clang's header-unit error
says the header "is not known to be a header unit"; its one-job message
quotes its own expanded command); system headers in the dependency file
are spelled with kiln's toolchain paths. With `-compilation-database`:
a JSON syntax error is reported as `Error while opening JSON database:
<file>:<line>:<col>: error: <what>` (clang's text is its YAML parser's);
an entry whose directory does not exist fails with `error: cannot change
to directory '<dir>': <reason>` under the header (clang scans through a
virtual working directory and reports the missing input instead); the
make rules come in entry order (clang's come in completion order, the
same only with `-j 1`); and each module or import is listed once, where
clang repeats a duplicated `import M;` (kiln's preprocessor records an
import once, `kiln_pp::modules`). In `experimental-full`, `command-line`
is the compile as given, where clang prints its own expanded `-cc1`
line, and an interface unit is one command, where clang 19 prints two (a
`-cc1` that builds the module file, then the object compile).

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

Real builds go through the OSS harness (WP-1148, spec/harness.md): a
`modules = true` row of `scripts/oss/projects.toml` is generated for
Ninja and given `-DCMAKE_CXX_COMPILER_CLANG_SCAN_DEPS=` (kiln-scan-deps
beside the kiln++ under test, or clang-scan-deps-19 for the clang 19
control), with `-DCMAKE_CXX_FLAGS=-stdlib=libc++` under `--stdlib
libc++` because the scanner is run directly, not through kiln-cxx.
`fmt_module` builds {fmt} 12.1.0 as the named module `fmt` with an
importer, and `cmake_modules` builds the fixture above (with `import
std;` under libc++); their results are docs/status/oss/fmt_module.md and
docs/status/oss/cmake_modules.md.

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
section framing. Its item 1 (WP-1046) is done: the section also carries
results of evaluations inside template instantiations (see
"Instantiation keys").

#### Inventory

What an image must hold, the id it is keyed by, and where it stands:

| state | id / owner | part 1 | part 2 |
|---|---|---|---|
| declarations, redeclaration groups, `UnitInfo` (owner, attachment, visibility) | `DeclId`, `RedeclId`; `Ast`, `Ast::modules` | replayed | image, appended (see "Identity") |
| types | `QualType` into the type context | replayed; a carried value's type is its expression's | type table, re-interned |
| expressions, statements, attributes, lists | `ExprId`, `StmtId`, `AttrId`, `ListId` | replayed | bodies the importer can use (inline, constexpr, templates), per declaration |
| template specializations and implicit instantiations | sema's instantiation tables | replayed | carried, merged by (template, arguments) |
| constant values | `ConstValueId` in the `ConstStore` | **carried**: integers, enumerators, `bool`, floating point, null pointers; pointers, references and pointers to members by the entities they name, reflections, and aggregates holding any of these (WP-1047); pointers to string literals, lifetime-extended temporaries and labels, and reflections of alias template specializations (non-type arguments as converted values, WP-1168), data member specifications and annotations (WP-1160); not an annotation `define_aggregate` made | the same encoding, its entities remapped; the values part 1 leaves out |
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
takes it from its own replayed copy of the expression. The one exception
is a reference's value (a reference variable's initializer, a reference
template argument), whose type is the reference type while the
expression's is the referee: it is written `typed`, with the reference
type, and both sides check that its referee is the expression's type. Outside any instantiation the two positions are the whole key
(instantiation index 0), and a result is used only while the importer
reads the unit the file holds (`module_current_unit` is that unit's), so
the same header text read by the importer itself never matches. Inside
an instantiation the positions are within the pattern, shared by every
instantiation of it, so the key also names the instantiation (see
"Instantiation keys"). Such a result is used whenever the importer makes
that evaluation in that instantiation, first from the table of the unit
being read and then from any other: a unit's pending instantiations
(function bodies used outside a constant context, [temp.point]/7) run at
the importer's end of TU, after the replay. That the importer's
instantiation gives the same result as the writer's is [temp.point]/7's
rule, whose violation is ill-formed, no diagnostic required.

#### Instantiation keys (Part 2 item 1, WP-1046)

An evaluation made while an instantiation is in progress is keyed by
the innermost instantiation frame (spec/ast.md §8.4, `InstFrame`),
written as a byte string that names no `DeclId`, so the
producing compile and the importer's replay build the same bytes for
the same specialization and compare them as they are:

- **Entity frames** (class, member class, member function, function
  specialization, static data member, variable template and its
  initializer, enumeration, default member initializer, default function
  argument, exception specification, lambda): the frame's entity.
  - `S` template, canonical arguments: a specialization (a class,
    function or variable template's).
  - `M` context, pattern: a member of a specialization instantiated
    from member `pattern`; `P` context, pattern: any other declaration
    instantiated from a pattern (a local class, a closure type, a
    local variable).
  - `D` kind, name kind, name (identifier bytes or operator code),
    position: a declaration that was not instantiated, inside nothing
    that was (a template, the namespace scope). Its position is the
    replay-stable (path index, offset) pair of "Identity". Implicit
    declarations have no `D` key, and a top-level key is never `D`.
  - `U`: the translation unit itself (the global namespace, as a
    reflection or a `D` key's context).
- **Substitution frames** that record their arguments (concept check,
  constraint check, deduced, explicit and prior argument substitution,
  default template argument): `F`, a one-byte frame code, the template,
  the entity (the template or the parameter), the canonical arguments.
  Only a template whose parameters are its outermost level (depth 0) is
  keyed; then the arguments are all the substitution uses.
- **Arguments**: a count, then per argument `T` type, `V` type and value
  (part 1's value encoding, extended by WP-1047 to the values that
  name entities, see "Encoding": a pointer or reference argument is its
  object's entity plus its byte offset, a reflection argument its
  reflectee's entity, type or value, so `X<&g>` and `Wrap<^^S>` merge),
  `N` template (an entity), `K` pack (an argument list). A value naming
  something without a key (a string literal, an unkeyed entity) gives
  the argument list no key.
- **Types** structurally: the canonical type's cv byte, then a tag and
  its operands (builtin, `_BitInt`, pointer, references, member pointer,
  arrays, function with parameters, flags and a `noexcept` bit, record
  and enumeration as entities, `_Complex`, `_Atomic`, vectors). A
  dependent or other type gives no key.
- Keys nest at most 48 levels and 16 KiB; deeper or longer keys are not
  formed.

The frames this leaves out are not keyed, and their evaluations stay
uncarried: alias template substitution and default-argument checking
(they record no arguments), and the other substitution frames
(constraint substitution, partial specialization deduction).

The writer interns each distinct key once and caches the key per frame
(entity, template, arguments), so a frame's key is built once however
many evaluations it makes. The reader does the same per table, and an
unknown key costs one hash lookup per frame. The evaluation's two
positions stay part of the key: within one instantiation they tell its
evaluations apart.

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
marker   u32 0xffffffff, then u32 version (4)
paths    u32 count, then each path (u32 length, UTF-8)
insts    u32 count, then each instantiation key (u32 length, bytes,
         see "Instantiation keys"), numbered from 1 in first-use order
entries  u32 count (at least 1), then each entry:
  inst   u32     (0: outside any instantiation; else an index into insts)
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
         the values naming entities (version 3, WP-1047); `var` is an
         unsigned LEB128 varint (at most 10 bytes), `ty` a type and `ent`
         an entity as in "Instantiation keys":
         5 member null:    u8 NullKind (a null pointer to member)
         6 pointer:        u8 flags (bit 0 past the end, bit 1 designated),
                           u8 base, then
                             0 declaration (a variable): ent
                             1 function: ent
                             2 typeid object: ty
                             3 template parameter object: ty, value
                             4 constant object: ty, value
                             (version 4, WP-1160:)
                             5 string literal: at (its expression's
                               position, as an entry's `at`)
                             6 lifetime-extended temporary: ent the
                               extending declaration, var its mangling
                               index, ty the temporary's type, u8 0|1,
                               then if 1: ty, value (its value)
                             7 label: ent the label, at (the
                               `&&label` expression's position)
                           var byte offset, [ty designated subobject]
         7 member pointer: u8 flags (bit 0 function, bit 1 derived),
                           ent member, var count, count x ent path
         8 reflection:     u8 reflectee tag (`Reflectee::tag`), then
                             type: u8 0 then ty, or 1 then ent (an alias),
                               or (version 4) 2 then ent (an alias
                               template), args (as in a key), ty (the
                               aliased type): an alias template
                               specialization
                             declaration, namespace, namespace alias,
                             template: ent
                             null: nothing
                             value, object: ty, value
                             base, parameter: ent, var index
                             (version 4, WP-1160:)
                             data member specification: ty, u8 flags
                               (bit 0 name, 1 alignment, 2 bit width,
                               3 no_unique_address), [name: bytes
                               (u32 length, an identifier)], [u32
                               alignment], [u32 width], var count,
                               count x reflectee (its annotations)
                             annotation: u8 0 (of the entity) or 1 then
                               var base index (of that base specifier),
                               ent the entity (the class, for a base),
                               at (the annotation's position), var
                               ordinal among that list's annotations at
                               that position
         9 aggregate+:     bytes (u32 length), var padding count, then
                           (var start, var length); var reloc count, then
                           (var at, ty, value); var union count, then
                           (var at, ent active member)
        10 typed:          ty (a reference type), then the value
```

A value part 1 could carry keeps part 1's bytes (tags 0-4), also inside
an aggregate+ or a key; tags 5-10 appear only in version 3 and 4
sections, and the pointer bases 5-7 and reflection forms above marked
version 4 only in version 4 sections.
Values nest at most 48 levels, like keys (an entry's value has no length cap).

- **Versioned.** Version 1 (WP-985) had no marker, version or
  instantiation table, and no `inst` field; its first word is the path
  count, never `0xffffffff`. The reader reads version 1 as written (every
  entry at index 0) and rejects any version but 1 to 4 (`unsupported
  section version`). Version 2 (WP-1046) is version 3 without tags 5-10
  and the `U` entity; version 3 (WP-1047) is version 4 without the forms
  marked version 4 above. Only carried keys are written: an instantiation key
  no kept entry uses is dropped and the rest renumbered.
- **Deterministic.** Paths are in first-use order and entries in
  first-evaluation order (no hash-map iteration), so the same inputs give
  the same bytes, and the driver test checks this.
- **Hardened reader.** `bmi_ast::decode` checks every count against the
  bytes left before sizing anything by it, validates UTF-8, flags,
  widths, words (`IntBits::from_words`: no bits above the width),
  float bits, padding ranges, path indices, duplicate keys and trailing
  bytes. Each instantiation key is parsed in full against the grammar of
  "Instantiation keys" (tags, nesting depth, path indices, values),
  duplicate keys and out-of-range `inst` indices are rejected, and so is
  every value of tags 5-10 (tags, flags, varints over 10 bytes or
  64 bits, offsets and counts beyond `u32`, entities, types). An entity
  in a value resolves to the importer's own declaration: a `D` entity
  by its kind and position (among the first, non-implicit declarations;
  two at one position resolve to neither), an `S` among its template's
  specializations, an `M`/`P` through the specialization's members.
  Every resolved declaration is checked by writing its key again and
  comparing the bytes, so a value names exactly the writer's entity or
  is not used (`bmi_ast/value.rs`). The version-4 forms resolve the same
  way (WP-1160): a string literal or label address to the importer's
  own `StringLiteral` or `&&label` expression at the position (two at
  one position resolve to neither; a label must be the label the value
  names), a temporary to the importer's lifetime-extended temporary of
  that extending declaration and mangling index whose type is the
  written one, an annotation to the ordinal-th of the entity's (or base
  specifier's) annotations at that position, unless the entity's
  annotations are still to be instantiated; a data member specification
  is checked as `data_member_spec` checks its options (an identifier,
  a power-of-two alignment, a name or a width, not both alignment and
  width, nor width and `no_unique_address`, no name on a zero width;
  annotations that are values or objects) and interned in the Oven. Any failure is a `SectionError`, turned into §11.4's
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

- no template instantiation is in progress, or the innermost one has an
  instantiation key (see "Instantiation keys");
- a key exists (see "Identity");
- the evaluation is pure: it added no declaration, expression or
  statement and emitted no error or warning (a carried result would
  silence a diagnostic the importer must give, and an evaluation that
  instantiated a template must still instantiate it);
- it succeeded, its value's type is its expression's (or it is a
  reference's value, see "Identity"), and every entity, type and value it
  names has a key (see "Encoding", and "Identity of the WP-1160 values"
  below for the ones that have none);
- no other evaluation with the same key gave a different result (the
  same evaluation made again, a constraint checked twice, keeps its
  result).

The rest are evaluated again by the importer, as in kind 1.

#### Identity of the WP-1160 values

Each carried value must be the object or entity the importer's replay
would have produced, so a pointer compares equal to the importer's own
pointer to the same object and a reflection to its own reflection of the
same entity:

- **String literal.** The replay creates the literal's object from the
  literal's expression; the value names that expression by position, and
  the importer's own expression there is the replay's ([lex.string]/9
  lets two evaluations of one literal give one object; kiln always does).
  A non-dependent literal in a template body is one expression every
  specialization shares (substitution keeps it), so `name<int>()` and
  `name<long>()` both carry. Two literal expressions at one position (a
  template expression formed again by an instantiation) resolve to
  neither, and the importer evaluates. `__func__` and the other
  predefined names are carried as the string literal they denote.
- **Lifetime-extended temporary.** The replay makes the temporary when it
  analyses the extending declaration, numbered by its mangling index; the
  value names it by both ([class.temporary]/6), so the importer's pointer
  is to its own temporary (the one its IR emits). The temporary's value
  travels with it and is given to the importer's temporary if it has none
  yet, as the replay's evaluation would; one that has a different value
  is not used. A temporary that is not lifetime-extended cannot be a
  constant's result ([expr.const]), so there is nothing to carry.
- **Label.** The value names the label and the `&&label` expression's
  position; the importer's own expression there must name the same
  label.
- **Alias template specialization.** `^^Ptr<int>` keeps the alias
  specialization with its canonical arguments and aliased type
  (`Ast::reflection_type`), and the type store interns it, so the
  importer's rebuilt type is its own `^^Ptr<int>`. A non-type argument is
  its converted value there (sema-templates.md §4.10, WP-1168), so
  `^^Arr<char, 1 + 2>` is carried and equals the importer's
  `^^Arr<char, 3>`
  (`tests/conformance/module.interface/carried-alias-value-arg-run.cpp`). A specialization whose list still is not canonical
  falls back (`a reflection of an alias template specialization with no
  key`).
- **Data member specification.** The Oven interns specifications by
  their contents ([meta.reflection.define.aggregate]: two calls with
  equal arguments give one reflection), so the importer interning the
  same contents gets the reflection its own `data_member_spec` call
  gives. One whose type or annotation values have no key falls back.
- **Annotation.** An annotation is a property of its entity
  ([dcl.attr.annotation]); the value names the entity, its position and
  its ordinal among the entity's annotations at that position (a macro
  argument expanded twice puts two at one position), so it is the
  importer's own annotation.
  One `define_aggregate` made has no position (`an annotation reflection
  made by define_aggregate`), and one whose entity has no key or whose
  position is outside the replayed source falls back too.

#### Fallback

The default writes kind 2 when at least one result is carried, and kind 1
otherwise. The hidden `-fkiln-bmi-kind=1` always writes kind 1, and `=2`
is the default. The choice is never a warning. Under `-v` the driver
prints one of:

```
note: wrote module file 'm.pcm' as kind 2, carrying N constant results[, M inside template instantiations][ (first not carried: <reason>)]
note: wrote module file 'm.pcm' as kind 1: no constant result to carry (first not carried: <reason>)
note: wrote module file 'm.pcm' as kind 1, as -fkiln-bmi-kind=1 requests
```

The `<reason>` is the first evaluation that was not carried, one of: `an
evaluation in a substitution that is not an instantiation` (a frame
"Instantiation keys" leaves out), `an instantiation whose arguments have
no key`, `a consteval block`, `an
evaluation at a position a replay does not reproduce`, `an evaluation that
changed the AST`, `an evaluation that emitted a diagnostic`, `an
evaluation that failed`, `a value whose type is not its expression's`, `a
pointer or reference value whose object has no key`, `a pointer-to-member
value whose member has no key`, `a reflection value with no key`, `an
aggregate holding a value with no key`, `a reference value` (a reference's
value naming something without a key), `two evaluations at one
position`, and (WP-1160) `a pointer to a string literal outside the
replayed source`, `a pointer to a temporary whose declaration has no
key`, `a label address whose label has no key`, `a reflection of an
alias template specialization with no key`, `a data member
specification with a value that has no key`, `an annotation reflection
made by define_aggregate`, `an annotation reflection whose entity has no
key`, `an annotation reflection outside the replayed source`. The reason
is printed whenever any evaluation was not carried, also beside a kind-2
count (WP-1160; before, only when nothing was carried). When every
evaluation was carried, or the unit made none, the parenthesis is left
out. The reader accepts both kinds, so
mixing them in one build is fine.

On libc++ 19's `std.cppm`, part 1 carried 2900 results, and part 2
item 1 carries 8774, 5744 of them inside template instantiations
(WP-1047 adds one: the file grows by 167 bytes, from
9,840,072 to 9,840,239) (most
of the rest in alias template substitution and default-argument checking
frames, which record no arguments). Before part 2, of the evaluations
that were not carried, 10142 happened inside a template instantiation, 141
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

**Part 2a, the declaration image (WP-1048).** Same method (harness
profile, aarch64, `/usr/bin/time`, median of 3, `-std=c++26`), image off
(`-fno-kiln-bmi-image`, the default) against on (`-fkiln-bmi-image` at
both the precompile and the import). Three workloads: `import std;`
(libc++ 19, a `vector`, `ranges::sort`, `string` and `println` program),
the constexpr-heavy module above, and a synthetic interface inside 2a's
coverage: 200 exported namespaces, each with a scoped enumeration of 8
enumerators, an alias, a `typedef`, a variable and 10 non-inline
functions with loop bodies (3201 lines, 4800 image records), whose
importer calls two of the functions.

| run | `import std;` (libc++ 19) | constexpr-heavy module | image-eligible module |
|---|---|---|---|
| kiln `--precompile`, image off | 1.13 s, 98.1 MiB, 9,129,636 B | 0.24 s, 10.9 MiB, 26,075 B | 0.11 s, 15.4 MiB, 385,446 B |
| kiln `--precompile`, image on | 1.16 s, 98.2 MiB, 9,129,636 B (no image) | 0.24 s, 10.9 MiB, 26,075 B (no image) | 0.11 s, 17.0 MiB, 763,088 B |
| kiln import, image off, `-fsyntax-only` | 1.25 s, 93.7 MiB | 0.04 s, 10.8 MiB | 0.11 s, 14.5 MiB |
| kiln import, image on, `-fsyntax-only` | 1.21 s, 96.0 MiB (replay) | 0.04 s, 10.5 MiB (replay) | **0.03 s, 12.2 MiB** |
| kiln import, image off, `-c` | 1.57 s, 96.5 MiB | 0.06 s, 60.9 MiB | 0.13 s, 60.9 MiB |
| kiln import, image on, `-c` | 1.67 s, 95.7 MiB (replay) | 0.06 s, 60.9 MiB (replay) | **0.05 s, 60.9 MiB** |
| clang 19 `--precompile` | 1.72 s, 259.0 MiB, 31,179,192 B | 3.96 s, 85.6 MiB, 26,556 B | 0.11 s, 91.3 MiB, 1,827,160 B |
| clang 19 import, `-fsyntax-only` | 0.20 s, 143.3 MiB | 0.05 s, 84.8 MiB | 0.01 s, 84.4 MiB |
| clang 19 import, `-c` | 0.37 s, 160.6 MiB | 0.06 s, 91.7 MiB | 0.01 s, 91.9 MiB |
| clang 22 `--precompile` | 1.79 s, 291.9 MiB, 32,250,900 B | 4.17 s, 88.3 MiB, 30,024 B | 0.11 s, 94.6 MiB, 1,861,336 B |
| clang 22 import, `-fsyntax-only` | 0.39 s, 175.5 MiB | 0.05 s, 87.0 MiB | 0.01 s, 86.5 MiB |
| clang 22 import, `-c` | 0.56 s, 190.6 MiB | 0.06 s, 94.0 MiB | 0.02 s, 94.0 MiB |

- `std` and the constexpr-heavy module fall back (a global module
  fragment; `constexpr` functions), so "on" is the replay, and the rows
  match "off" within noise. `std`'s gain waits for S2–S5.
- The image-eligible module's import drops from 0.11 s to 0.03 s
  (`-fsyntax-only`) and from 0.13 s to 0.05 s (`-c`, whose 60.9 MiB is
  LLVM's), and the importer's peak RSS from 14.5 MiB to 12.2 MiB: the
  loader holds the section once (moved out of the capsule) and maps only
  the main file. The image grows the file by 377,642 B, about 79 B per
  record, and doubles this module file (its functions' bodies are kept in the
  capsule for `-c m.pcm`).
- Materialised fraction: 100% (4800 of 4800 records; the importer names
  4 of them). The eager load is the remaining gap to clang's 0.01 s
  (S-lazy).
- Commands: `kiln++ -std=c++26 [-fkiln-bmi-image] --precompile m.cppm -o
  m.pcm`, then `kiln++ -std=c++26 [-fkiln-bmi-image] -fmodule-file=m=m.pcm
  -fsyntax-only|-c use.cpp`; for `std`, add `-stdlib=libc++
  -Wno-reserved-module-identifier` and use
  `/usr/lib/llvm-19/share/libc++/v1/std.cppm`. clang the same, without
  the kiln flag.

**Part 2a, stage S2: bodies (WP-1064).** Same method (load ~22, so
compare columns, not absolute times). Four workloads: `import std;` as
above; the corpus's largest module (`module.interface/Inputs/img-bodies.cppm`,
65 image records); a constexpr module (a sieve plus an FNV hash,
imported by `static_assert(work(20000))`, also included textually); and a
synthetic interface inside S2's coverage: 200 exported namespaces of 10
`constexpr`/`inline` functions with loop bodies, a scoped enumeration and
initialized variables (13,201 lines, 8800 image records), whose importer
calls two of the functions in a `static_assert` and at run time.

| run | `import std;` (libc++ 19) | largest corpus module | constexpr module | S2-eligible module |
|---|---|---|---|---|
| kiln `--precompile`, image off | 1.18 s, 99.1 MiB, 9,129,636 B | 0.06 s, 9.8 MiB, 25,872 B | 0.05 s, 9.8 MiB, 23,309 B | 0.17 s, 18.3 MiB, 671,566 B |
| kiln `--precompile`, image on | 1.28 s, 97.6 MiB, 9,129,636 B (no image) | 0.05 s, 11.1 MiB, 41,403 B | 0.04 s, 10.7 MiB, 26,929 B | 0.21 s, 40.8 MiB, 4,595,063 B |
| kiln import, image off, `-fsyntax-only` | 1.26 s, 96.0 MiB | 0.06 s, 10.2 MiB | 0.07 s, 10.3 MiB | 0.16 s, 16.0 MiB |
| kiln import, image on, `-fsyntax-only` | 1.28 s, 96.7 MiB (replay) | 0.05 s, 11.3 MiB | **0.04 s, 11.0 MiB** | **0.09 s**, 25.8 MiB |
| kiln import, image off, `-c` | 1.64 s, 95.4 MiB | 0.07 s, 61.9 MiB | 0.09 s, 60.6 MiB | 0.20 s, 61.3 MiB |
| kiln import, image on, `-c` | 1.63 s, 96.5 MiB (replay) | 0.07 s, 61.8 MiB | 0.08 s, 60.6 MiB | **0.13 s**, 61.4 MiB |
| kiln textual, `-fsyntax-only` | | | 0.06 s, 11.1 MiB | |
| clang 19 `--precompile` | 1.62 s, 261.6 MiB, 31,179,192 B | 0.01 s, 85.8 MiB, 32,708 B | 0.02 s, 84.2 MiB, 23,524 B | 0.13 s, 91.2 MiB, 1,811,184 B |
| clang 19 import, `-fsyntax-only` | 0.21 s, 142.9 MiB | 0.01 s, 84.5 MiB | 0.33 s, 84.2 MiB | 0.01 s, 84.0 MiB |
| clang 19 import, `-c` | 0.38 s, 160.4 MiB | 0.02 s, 93.3 MiB | 0.34 s, 90.4 MiB | 0.02 s, 92.0 MiB |
| clang 19 textual, `-fsyntax-only` | | | 0.36 s, 84.8 MiB | |

- `std` still falls back (its global module fragment, S5), so "on" is
  the replay and matches "off" within noise; the rest of what it needs
  is listed in "What `import std;` still needs after S3".
- An imported `constexpr` function evaluates no slower from the image
  than textually included: 0.04 s against 0.06 s textual and 0.07 s for
  the replay (clang 19: 0.33 s and 0.36 s with
  `-fconstexpr-steps=100000000`).
- The S2-eligible module's import drops from 0.16 s to 0.09 s
  (`-fsyntax-only`) and from 0.20 s to 0.13 s (`-c`), but the file grows
  6.8x (671,566 B to 4,595,063 B, about 0.5 KB per record and 2 KB per
  small body) and the importer's peak RSS rises from 16.0 MiB to
  25.8 MiB (precompile 18.3 to 40.8 MiB): S2's fixed-width `u32` records
  and the eager decode. The compact encoding and building straight from
  the bytes are a follow-up under WP-1064.
- Use of the image was verified by blanking the capsule's source text
  (and recomputing the checksum): the importer still compiles, links and
  runs. Two pitfalls hid it at first: a namespace named like its module
  (`import m;` interned `m`, so the gate in "ODR merging" always
  replayed; fixed by WP-1096's unseen names), and pre-interned short
  names (`c`). The S2 corpus inputs therefore use namespaces `ib` and
  `ic`.
- Commands as for Part 2a; the textual runs compile the module's body
  as a header; clang adds `-fconstexpr-steps=100000000` for the
  constexpr module.

**Part 2a, stage S3: classes (WP-1065).** Same method (harness profile,
aarch64, `/usr/bin/time`, median of 3, load ~19). Two workloads: the
corpus's largest module that takes the image
(`module.interface/Inputs/img-class.cppm`, 108 image records: bases,
constructors with ctor-initializers, a destructor, nested classes,
static members, qualified member types), imported by
`image-class-run.cpp`; and a class-heavy synthetic interface: 200
namespaces of four exported classes each (a point with operators, a
base, a derived class with a member subobject, ctor-initializers and an
out-of-line member, a class with a member enumeration, typedef and
private fields) plus a `constexpr` variable computed through them (8001
lines, 10,000 image records), whose importer evaluates members of two
namespaces in a `static_assert` and runs members of two others.

| run | largest corpus module | class-heavy module |
|---|---|---|
| kiln `--precompile`, image off | 0.03 s, 11.2 MiB, 26,458 B | 0.09 s, 16.3 MiB, 277,178 B |
| kiln `--precompile`, image on | 0.03 s, 11.4 MiB, 42,713 B | 0.12 s, 25.5 MiB, 1,888,335 B |
| kiln import, image off, `-fsyntax-only` | 0.04 s, 11.1 MiB | 0.10 s, 15.3 MiB |
| kiln import, image on, `-fsyntax-only` | 0.05 s, 10.7 MiB | **0.05 s**, 16.6 MiB |
| kiln import, image off, `-c` | 0.06 s, 61.9 MiB | 0.12 s, 61.7 MiB |
| kiln import, image on, `-c` | 0.07 s, 61.9 MiB | 0.11 s, 61.5 MiB |
| clang 19 `--precompile` | 0.01 s, 85.4 MiB, 41,572 B | 0.10 s, 93.3 MiB, 1,746,980 B |
| clang 19 import, `-fsyntax-only` | 0.01 s, 85.4 MiB | 0.01 s, 85.3 MiB |
| clang 19 import, `-c` | 0.02 s, 93.7 MiB | 0.01 s, 93.2 MiB |

- The class-heavy import halves (`-fsyntax-only` 0.10 s to 0.05 s); the
  importer's peak RSS rises 1.3 MiB (15.3 to 16.6 MiB) for the eager
  decode. Under `-c` the gain is 0.01 s, since codegen of the used
  inline members dominates. The corpus module is too small to separate
  from noise (one record decode against one tiny parse).
- The image grows the file 6.8x (277,178 B to 1,888,335 B, about 160 B
  per record) and precompile's peak RSS from 16.3 to 25.5 MiB: the
  fixed-width records of S2, whose compact encoding is WP-1095.
- On/off equivalence: identical diagnostics and LLVM IR from the image
  and from the replay for both importers (and for every module test of
  the corpus under `cargo xtask test --bmi-image`), and the same exit
  status at run time; the blank-source check (as for S2) shows the
  image was used.

**Part 2a, stage S4 first slice: function templates (WP-1066).** Same
method (harness profile, aarch64, `/usr/bin/time`, median of 3, load
~20, `-std=c++26`). Two workloads: the corpus module
`module.interface/Inputs/img-template.cppm` (43 image records), imported
by `image-template-run.cpp`; and a template-heavy synthetic interface:
200 exported namespaces of ten function templates each (type and
non-type parameters, locals, loops, compound assignments, casts,
`sizeof`; 2401 lines, 8000 image records), whose importer instantiates
templates of five namespaces in a `static_assert` and in `main`.

| run | corpus module | template-heavy module |
|---|---|---|
| kiln `--precompile`, image off | 0.08 s, 10.7 MiB, 24,170 B | 0.14 s, 11.3 MiB, 184,460 B |
| kiln `--precompile`, image on | 0.10 s, 10.8 MiB, 30,458 B | 0.13 s, 18.3 MiB, 1,258,218 B |
| kiln import, image off, `-fsyntax-only` | 0.29 s, 11.0 MiB | 0.13 s, 12.9 MiB |
| kiln import, image on, `-fsyntax-only` | 0.07 s, 11.2 MiB | **0.06 s**, 13.9 MiB |
| kiln import, image off, `-c` | 0.11 s, 54.6 MiB | 0.19 s, 54.0 MiB |
| kiln import, image on, `-c` | 0.11 s, 54.6 MiB | 0.16 s, 54.0 MiB |
| clang 19 `--precompile` | 0.08 s, 82.4 MiB | 0.11 s, 85.4 MiB |
| clang 19 import, `-fsyntax-only` | 0.02 s, 82.7 MiB | 0.03 s, 82.9 MiB |
| clang 19 import, `-c` | 0.02 s, 90.9 MiB | 0.03 s, 90.4 MiB |

- The template-heavy import halves (`-fsyntax-only` 0.13 s to 0.06 s),
  for 1.0 MiB more peak RSS (the eager decode of 8000 records); `-c`
  gains 0.03 s. The corpus module's 0.29 s image-off `-fsyntax-only`
  is load noise (its `-c` is 0.11 s); it is too small to separate.
- The image grows the file 6.8x (about 135 B per record), the same
  ratio as S3, and precompile's peak RSS from 11.3 to 18.3 MiB
  (compact encoding: WP-1095).
- On/off equivalence: identical LLVM IR for both importers, identical
  instantiation diagnostics (`kind2_declaration_image_templates`), and
  the importer's `heavy<int>` reuses the interface's carried constant
  (no module evaluation in either mode).
- Re-run: the synthetic interface is `export module th;` and 200
  copies of `export namespace n<i> { ... }` holding `add`, `scale<T,
  N>`, `square<N>`, `addr`, `twice`, `width`, `narrow`, `pick` and
  `sum_to` as in `img-template.cppm` (all `constexpr` but `addr`,
  `twice`, `narrow`) plus `template <class T, int K> constexpr T
  poly(T x)` (a K-step loop); the commands are `kiln++ -std=c++26
  [-fkiln-bmi-image] --precompile th.cppm` and `kiln++ -std=c++26
  [-fkiln-bmi-image] -fmodule-file=th=th.pcm {-fsyntax-only | -c} u.cpp`.

**Part 2a, stage S4 second slice: class templates (WP-1139).** Same
method (harness profile, aarch64, `/usr/bin/time`, median of 3, load
~20, `-std=c++26`). Two workloads: the corpus module
`module.interface/Inputs/img-class-template.cppm` (73 image records),
imported by `image-class-template-run.cpp`; and a class-template-heavy
synthetic interface: 200 exported namespaces each holding `Box`, `Arr`
(a nested class, an out-of-line member, a static data member defined
out of line), `Fixed<int N>`, `Base`/`Derived` (a dependent base),
`W`, `Def` (type defaults), `Acc` and the function templates `boxed`
and `unbox` as in the corpus module (7401 lines, 13,800 image
records), whose importer uses five namespaces in a `static_assert` and
in `main`.

| run | corpus module | class-template-heavy module |
|---|---|---|
| kiln `--precompile`, image off | 0.04 s, 11.3 MiB, 25,088 B | 0.12 s, 14.7 MiB, 303,307 B |
| kiln `--precompile`, image on | 0.06 s, 11.4 MiB, 33,981 B | 0.15 s, 24.0 MiB, 1,746,634 B |
| kiln import, image off, `-fsyntax-only` | 0.05 s, 11.4 MiB | 0.12 s, 15.4 MiB |
| kiln import, image on, `-fsyntax-only` | 0.04 s, 11.5 MiB | **0.06 s**, 16.2 MiB |
| kiln import, image off, `-c` | 0.09 s, 60.2 MiB | 0.14 s, 60.5 MiB |
| kiln import, image on, `-c` | 0.09 s, 60.2 MiB | 0.09 s, 60.5 MiB |
| clang 19 `--precompile` | 0.05 s, 85.9 MiB | 0.10 s, 96.6 MiB |
| clang 19 import, `-fsyntax-only` | 0.01 s, 85.8 MiB | 0.01 s, 87.7 MiB |

- The class-template-heavy import halves (`-fsyntax-only` 0.12 s to
  0.06 s; `-c` 0.14 s to 0.09 s), for 0.8 MiB more peak RSS (the eager
  decode of 13,800 records). The image grows the file 5.8x (about
  105 B per record) and precompile's peak RSS from 14.7 to 24.0 MiB.
- On/off equivalence: identical LLVM IR, identical member-instantiation
  diagnostics and the interface's carried constants reused
  (`kind2_declaration_image_class_templates`); both programs print the
  same output.
- `import std;` (libc++ 19 `std.cppm`): still 0 image records; the unit
  falls back at its first node, "a global module fragment" (S5), and
  its purview is using-declarations (S6). Inside the headers the next
  class-template gaps are the "Still falling back" list above (packs,
  member templates, partial specializations and non-type arguments
  are on nearly every libc++ class template).
- Re-run: the synthetic interface is `export module cth;` and 200
  copies of `export namespace n<i> { ... }` holding the class templates
  above plus `template <class T> struct Acc { T s; constexpr Acc
  &add(T x); constexpr T sum_to(int n) const; }` (a loop); the commands
  are `kiln++ -std=c++26 [-fkiln-bmi-image] --precompile cth.cppm` and
  `kiln++ -std=c++26 {-fkiln-bmi-image | -fno-kiln-bmi-image}
  -fmodule-file=cth=cth.pcm {-fsyntax-only | -c} u.cpp`.

**Part 2 item 1, results inside instantiations (WP-1046).** Same method
(harness profile, aarch64, `/usr/bin/time`, load ~20; precompile median of
3, imports median of 5, interleaved). "Part 1" is the base commit's binary
(cfa635ab3), "Part 2" this one. The harness profile has 16 codegen units
and no LTO, and the base binary happens to be about 15% faster than every
other build of this period on a plain constexpr TU (0.17 s against 0.20 to
0.21 s for five unrelated branches), so compare kind 1 with kind 2 within
a binary. Workloads: the `import std;` program of Part 2a, `import std;`
with an empty `main` (a use of the section that finds nothing), and a
template-heavy constexpr interface: 64 instantiations `H<i>` of a class
whose static member runs a 50,000-iteration loop, checked by a
`static_assert`, a function template `pick<i>` with a local `constexpr`
variable and `if constexpr`, and `Buf<i>` whose bound is `H<i>::slot`,
imported by a program that names them in a `static_assert` and at run
time.

| run | `import std;` (libc++ 19) | `import std;`, empty `main` | template-heavy module |
|---|---|---|---|
| Part 1 `--precompile`, kind 1 | 1.13 s, 98.9 MiB, 8,993,259 B | | 0.17 s, 13.4 MiB, 25,990 B |
| Part 1 `--precompile`, kind 2 | 1.06 s, 99.5 MiB, 9,131,988 B | | 0.17 s, 13.4 MiB, 32,087 B |
| Part 2 `--precompile`, kind 1 | 1.16 s, 98.2 MiB, 8,990,956 B | | 0.22 s, 12.9 MiB, 25,990 B |
| Part 2 `--precompile`, kind 2 | 1.15 s, 101.3 MiB, 9,840,073 B | | 0.23 s, 13.3 MiB, 70,695 B |
| Part 1 import, kind 1, `-c` | 1.44 s, 97.4 MiB | 1.11 s, 91.7 MiB | 0.18 s, 60.9 MiB |
| Part 1 import, kind 2, `-c` | 1.42 s, 96.6 MiB | 1.11 s, 91.2 MiB | 0.19 s, 60.9 MiB |
| Part 2 import, kind 1, `-c` | 1.52 s, 96.5 MiB | 1.19 s, 91.8 MiB | 0.24 s, 60.9 MiB |
| Part 2 import, kind 2, `-c` | 1.54 s, 98.4 MiB | 1.18 s, 92.1 MiB | **0.06 s**, 60.9 MiB |
| Part 2 import, kind 1, `-fsyntax-only` | 1.20 s, 95.6 MiB | | 0.22 s, 12.0 MiB |
| Part 2 import, kind 2, `-fsyntax-only` | 1.20 s, 98.2 MiB | | **0.04 s**, 11.7 MiB |
| clang 19 `--precompile` | | | 3.50 s, 86.4 MiB |
| clang 19 import, `-c` | 0.37 s, 160.4 MiB | | 0.01 s, 90.8 MiB |

- Carried: `std` 8774 results (part 1: 2889 from this build), 5744 of
  them inside instantiations: about 5744 of the 10,142 are now carried. The
  importer's `-fconstexpr-report=` evaluations: 20,350 (kind 1), 17,464
  (part 1), 9006 (part 2). The template-heavy module carries 513 (part 1:
  129), 384 inside instantiations, and its importer drops from 578
  evaluations and 3,329,216 steps (part 1: 449 and 3,264,640) to 65 and 0.
- Section size: `std`'s grows from 138,729 B to 849,117 B (6.1x, about
  97 B per result against 48 B: each instantiation's key is written once,
  but every entry has a `u32` index and eight `u32` position fields); the
  template-heavy module's from 6,097 B to 44,705 B. A compact encoding is
  a follow-up.
- Imports that find little in the section do not slow down: a second run
  of `std`'s `-c` import over 9 interleaved rounds gives 1.54 s for kind 1
  and 1.53 s for kind 2, and the empty `main` 1.19 s and 1.18 s. The
  table costs about 1 to 2 MiB of RSS on `std`. `std`'s own constant work
  is about 15 ms, so its import time is still the replay's. The eager
  decode stays: a lazy table would save at most the decode, which is
  within noise here.
- Kind-1 files are byte-identical between the two binaries apart from the
  resource directory's path (its length prefixes and the checksum).
- Commands as for Part 2a, with `-fkiln-bmi-kind=1|2` at the precompile;
  the report runs add `-fconstexpr-report=r.json` to the import, and the
  `-v` note gives the carried counts.

#### Part 2

Next, in order:

1. **done (WP-1046)**: carried results for evaluations inside
   instantiations, keyed by the instantiation (see "Instantiation keys");
   measured under "Measurements", "Part 2 item 1";
2. pointer, reference and reflection values (they need `DeclId`
   remapping);
3. the declaration and type image with ODR merging (see "Identity"),
   loaded lazily, which retires the replay and brings `import std;`
   towards clang's 0.23 s.

The section framing, the fallback note, the harness variant and the fuzz
tests are reused as they are.

#### Part 2a: the declaration and type image (WP-1048)

Part 2a is the first cut of item 3. It defines the image section (tag 2,
`SECTION_IMAGE`), its writer and its reader, and an importer path that
builds the imported unit's declarations from the section *instead of*
replaying the capsule. It covers a finished set of node kinds (below).
Any other node makes the writer emit no image section, and the importer
then replays exactly as in part 1. The capsule is always written, so a
file with an image can still be replayed. Everything is behind the
hidden `-fkiln-bmi-image` (spec/driver.md), **off by default**: when it
is off at precompile no section is written, and when it is off at import
a section that is present is ignored.

**Arenas serialised.** Declarations (the purview's, in id order, plus
the locals of carried bodies), the types they name, the per-context name
tables, and since S2 (WP-1064, "Bodies" below) the expressions,
statements, constants and string literals of every carried body and
initializer. The body of an inline, `constexpr` or `consteval` function,
every variable initializer and every default argument are carried as
records, because an importer evaluates and inlines them. A non-inline
function's body is not needed by an importer, because the module's
object file defines it (§7, deviation 7). The writer drops it, and the
reader gives the function an empty body `{}` at the original brace
positions, so "is defined" is as it was; codegen never emits it, since
its owner is an imported named unit. Function templates are carried as
patterns (S4, below); other templates still fall back.

**Coverage (2a).** A unit qualifies when all of these hold:

- it is a primary module interface unit (`export module M;`, no
  partition) that imports nothing;
- it has no global module fragment (no declaration from a real file
  before `module`) and no private module fragment;
- every purview declaration that is not a local or a parameter is one
  of the following:
  - a named, non-inline namespace;
  - an `export` declaration;
  - a namespace-scope function that is not a template, not deleted or
    defaulted, not a coroutine and has no contract assertions, and is
    not `main`, with an identifier name and a compound body (no
    function-try-block). Since WP-1278 it may have C linkage and the
    attributes "Attributes" lists. Since S2 it may be
    inline, `constexpr` or `consteval` (its body is carried, "Bodies"),
    have a deduced return type, and have default arguments;
  - since S3, a class and its members as "Classes (S3)" below lists;
  - a namespace-scope variable that is not a template or
    `thread_local`. Since S2 it may be `const`, `constexpr`,
    `constinit` or `inline`, and have an initializer (carried as
    records, with its evaluated value when that is a scalar);
  - a `static_assert` declaration, which is checked when the module is
    compiled and not carried (since S2);
  - an enumeration (scoped or unscoped, fixed or not) or an enumerator;
  - a `typedef` or alias declaration that is not a template;
- no declaration is invalid or implicit (since S3, except the implicit
  special members of a carried class; since WP-1278 also the builtin
  functions sema declared and their linkage specification, "Scopes and
  reserved names"), or carries an attribute "Attributes" does not list
  (before WP-1278, any attribute);
- every type is built from builtins, pointers, references, constant
  arrays (the bound's expression is dropped), arrays of unknown bound,
  function types (exception specification none, `noexcept` or
  `noexcept(true)`), the unit's own enumerations, typedefs and (since
  S3) classes, parenthesised types, elaborated types and cv-qualifiers.
  Since S3 an elaborated type may be qualified (`ns::S`, `::ns::C::T`)
  when every qualifier step is `::`, one of the image's namespaces or
  one of its classes;
- every location is invalid, or one the location records carry: since
  S7 (WP-1070) in any file and in macro expansions, except a predefined
  buffer or a file with `#line` directives (see "Locations (S7)");
- before WP-1278, no name lies in a namespace named `std` or is
  reserved. Since WP-1278 such names are carried; a declaration at
  global scope named `main`, with a builtin's name, or with a name sema
  predeclares there falls back ("Scopes and reserved names"). Inline and
  unnamed namespaces and linkage specifications (except a C one in a
  namespace) are carried too.

The first offending node's reason is the `-v` note's (see "Fallback").
Before the per-kind checks, a declaration kind 2a never carries names
its family: "a template", "a using-declaration,
using-directive or namespace alias" (before S6), "a linkage
specification" (before WP-1278), or "a declaration the image does not
carry yet" for the rest. Inside a carried
body, the expression, statement and local-declaration kinds are those of
"Bodies" below; any other node falls back with its own reason.

**Remapping per id space.** This is the "Identity" design, restricted to
what 2a carries:

- `DeclId`: records are numbered 0..n in the writer's id order, and a
  record refers only to lower numbers (context, lexical context,
  previous declaration, enumeration, types). The reader appends each
  record through `AstBuilder`, so local index i maps to the id it gets,
  and a reference to an index that is not yet built is malformed.
- `QualType`: a type table in dependency order. Each entry is a
  structural record whose operands are earlier entries, builtin indices
  or declaration indices. It is re-interned with the type context's
  constructors (`pointer_to`, `function`, `enum_type`, ...), so the
  importer's types stay canonical, and cv bits travel on each reference.
- `Symbol`: a string table, interned once per load.
- `Loc`: an offset into the unit's main file, which the reader loads
  through `load_regular_file_expecting` as the replay does (§11.2). It
  shares the `FileId` and so the positions are the replay's exactly.
  Diagnostics, `-g` and `__builtin_source_location` therefore point at
  the module source. Only that one file is mapped, and the other capsule
  files are not loaded at all.
- `ExprId` / `StmtId` (S2): one index space per arena, in post order,
  so an operand or sub-statement is always a lower index. The reader
  appends each record through `AstBuilder` and maps index i to the id
  it gets. A dropped non-inline body is a new empty `CompoundStmt`.
- `ConstValueId`: an enumerator's value, as (signed, 128-bit integer)
  re-interned with the enumerator's type; since S2 also a constant
  table (integer, `bool`, floating, null pointer) for literals, constant
  expressions and evaluated variables, re-interned through the constant
  pool.
- `StringLitId` (S2): a string table of (encoding, bytes), re-interned.

**ODR merging.** A 2a image has no global module fragment, so it has no
GMF entity to merge with other imports (WP-460's merge-by-entity case
needs one, so it falls back). It could still collide with the importer's
own declarations, or with another import's (for example, two modules
that both open `namespace util`). 2a does not merge. Instead the image
lists its *gated names* up front, and the preprocessor uses the image
only if **none of them has been seen at global scope yet** (WP-1096,
narrowed by WP-1162).

The gated names are those of the image's declarations whose context is
the translation unit, seen through export blocks, unnamed namespaces and
unscoped enumerations (whose names are visible in the enclosing scope);
the writer computes them, and the header lists them as indices into the
declared names. Every other declaration of the image sits in one of its
classes, named namespaces or functions, or is a template parameter: an
importer's declaration can meet it only inside a scope that a gated name
declares (`m::g` is a member of the gated namespace `m`, `Box::get` of
the gated class `Box`), so a gated name covers it.

A name is seen at global scope when an identifier token with that
spelling reached the parser (`Preprocessor::next_item`) outside every
opaque brace, or among a pragma's tokens (wherever the pragma is), or
when an image that is used gates it. The preprocessor tracks the braces
of the tokens it returns (`kiln_pp::name_gate`, one bit per symbol and a
few comparisons per token, only under `-fkiln-bmi-image`). A `{` is
transparent, and its names count, when it opens a linkage specification
(`extern` and string literals before it), an export block (`export {`),
an enumeration body (`enum` since the last `;`, `{` or `}`: unscoped
enumerators are declared in the enclosing scope; scoped ones count too,
which only costs a replay) or an inline or unnamed namespace (the lookup
of a namespace name searches the inline namespace set, [namespace.def]/2,
so `inline namespace v { namespace m {} }` meets the image's `m`). Every
other `{` (a class or function body, a named non-inline namespace, an
initializer, a lambda, a requires-expression) is opaque.

The rule tracks what reached the parser rather than what sema declared
because the gate runs in the preprocessor, ahead of the parser's
lookahead: sema may not yet have declared the tokens before the
`import`. Every importer declaration of a global-scope name (a
declarator-id, a namespace or class name, a using-declaration, an alias,
an enumerator) is such a token at global scope, in this file, a header
or an earlier replay, and an earlier image's gated names are marked. Sema
declares identifiers at global scope by itself only for builtins and the
hidden namespace `std`; an image never declares a name of either kind at
global scope (it falls back, or extends the namespace `std` as the
parser does: "Scopes and reserved names"). What the gate does not
count cannot meet a gated declaration:

- an identifier that never reached the parser: a macro name or an
  unexpanded macro body (`#define USE m::f`), an `#if` operand, a
  module or partition name component (`import m;` interns `m`, but
  the directive is consumed by the preprocessor);
- a member of the importer's class, a declaration in its named
  namespace (`namespace o { int twice; }` against the image's
  `::twice`), a local, an operand inside an initializer.

The only declarations inside an opaque brace that land at global scope
are block-scope `extern` and friend declarations of functions and
variables. Those cannot correspond to a declaration attached to the
imported module ([basic.link]/11: ill-formed, no diagnostic required),
and with other signatures they overload in either case. What the gate
does count without a declaration (a use, an expression, a declaration
in another scope spelled at global scope such as `using X = m::T;`) only
costs a replay.

So `export module m; export namespace m { ... }` and the dotted
`export module a.b; export namespace a { namespace b { ... } }` are
built from their images, as is a unit whose names the importer spelled
before the import only in a macro, as members of its own class, in its
own namespace, as locals, or declared in another scope (`int g;` against
the image's `m::g`). An importer that declared `namespace m`, a global
`twice` overload, `extern "C++" { namespace m {} }`, an enumerator `m`
or `inline namespace v { namespace m {} }` before `import m;`, a global
module fragment `#include` that declares `namespace m`, or an earlier
image or replay that declared `m`, still replays. A fresh name cannot
meet an earlier declaration, and later declarations and imports find the
image's declarations through ordinary lookup, as they would find the
replay's. Any overlap at global scope replays, so the semantics are part
A's by construction. With `-v` the driver notes each import whose module
file holds an image: `imported 'm' from its declaration image`, or
`replayed 'm': ` and why (the first seen gated name, an import of other
units, or another main file). The names of a carried body's locals (S2)
are not gated: they are in the parameter-name table, since no lookup
outside the body can find them. Real merging needs WP-460 semantics
without a second parse: an ODR key per GMF entity, and a structural hash
for definitions. It is follow-up S5.

**Laziness, and the deviation.** The format is lazy-ready: a per-record
offset table and a lookup table sorted by (context, name). The 2a reader
nevertheless materialises every record when the unit begins (100% of the
records), a deliberate deviation from "the top-level lookup table is
lazy":

- sema's name lookups take `&self`, and the parser classifies peeked
  identifiers through them, so a lookup miss cannot build declarations;
- the token stream reaches sema only at declaration starts (module events
  are flushed there), so no point before a lookup knows the names it
  will need.

Laziness needs a materialisation hook with `&mut` access on the lookup
path: a pending-name table that the parser drains before classifying a
token, or a lookup miss that queues the name and retries. That is
follow-up S-lazy. Since S2 the eager load also builds every carried
body; it still costs one record per node, and the replay's lexing,
parsing and checking are gone (measured in "Measurements": 0.11 s to 0.03 s for 4800
records).

**Templates as patterns (S4, WP-1066).** A template is carried as its
pattern, never as its instantiations, so the importer instantiates it
exactly as if it had parsed it:

- *Records.* Three record kinds join the table: a template type
  parameter (kind 15), a non-type template parameter (kind 16) and a
  template (kind 17). A template's parameters and its pattern precede
  it, in the order the parser created them (parameters, then the
  pattern, then the template), so the "refers only to lower indices"
  rule holds unchanged. The template record names the pattern record,
  its parameter records in order, the parameter list's depth and the
  `template`, `<` and `>` locations. The pattern is an ordinary record
  (a function for a function template) whose header carries
  `DEPENDENT_CTX`; its body, default arguments and locals are ordinary
  body records whose types may be dependent (type tag 15 names a type
  parameter record) and whose expressions carry their dependence bits
  (the `deps` byte of a v4 expression record). Unresolved names in the
  pattern are carried as their unresolved-lookup form (kinds
  `UnresolvedLookupExpr`, `DependentScopeDeclRefExpr`,
  `CXXUnresolvedConstructExpr`: the name and the candidate set found at
  definition time, [temp.dep.candidate]), so the instantiation's second
  phase sees the same first-phase result. (Not in the first slice: a
  pattern body holding any of these falls back with "a template
  expression".)
- *Rebuilding.* The reader replays the parser's sequence through the
  same builders: it opens a parameter scope (`dependent_depth`), builds
  each parameter with `template_type_parm` / `non_type_template_parm`,
  marks the next declaration as a pattern (`mark_pattern_next`, so the
  pattern gets no member-list entry and no lookup), builds the pattern
  and its body, then builds the parameter list and the template with
  `template_param_list` and `template_decl`, which links the pattern
  (`TemplLink::Pattern`) and registers the template's name. Nothing
  else differs from a parsed template: instantiation goes through the
  existing entry points (`get_function_spec`, `find_spec`) and the
  existing substitution, which only reads the pattern's AST.
  Parameters and patterns are not in the lookup table (they are not
  found by name), like locals; their names are in `pnames`.
- *Instantiations made in the interface* are not carried. The writer
  skips every implicit instantiation (a declaration with an
  `instantiated_from` link or an implicit-instantiation
  `spec_info`), with its parameters, locals and body. The importer's
  specialization table therefore starts empty for an image-built
  template, and every use, in the interface's carried bodies or in the
  importer, instantiates through `find_spec` keyed by (template,
  canonical arguments): the merge is the table's own uniqueness, and
  arguments made of image types are remapped by construction (they are
  the importer's types). A carried body that names a specialization (a
  call `add(2, 3)` resolved in the interface) needs a specialization
  reference record, (template record, argument list), resolved by the
  reader through `get_function_spec`; until that lands (follow-up) such
  a body makes the unit fall back. Constants carried for instantiations
  (WP-1046, the `-fkiln-bmi-image`-independent constant section) key on
  the template's entity key, which is DeclId-free (kind, name, position),
  so an image-built template matches the same keys as a replayed one and
  keeps them.
- *What falls back* (per declaration, with a `-v` reason; the unit then
  replays): packs, default template arguments, template template
  parameters, constraints and requires-clauses, concepts, variable and
  alias templates, deduction guides, redeclared templates, explicit and
  partial specializations, explicit instantiations (of class templates
  carried since WP-1307), members of class
  templates and class templates themselves (first slice), and any
  pattern node or side table the schema does not carry (template
  argument lists in expressions, dependent member access, dependent
  access checks, folds).

*First slice (WP-1066).* Carried: namespace-scope function templates
(depth 0) whose parameters are type and non-type parameters without
packs, defaults or constraints, whose pattern is neither a member nor
redeclared, and whose body uses only the carried expression kinds with
dependent types (a dependent call through an unresolved name, a
dependent construction `T()` and a dependent member access are "a
template expression"). A pattern's parameters and locals carry
`DEPENDENT_CTX` like the pattern; the reader raises `dependent_depth`
while it builds a record whose header has the bit (the arena's flags
are monotone, so the bit cannot be set afterwards). The validator
requires every `DEPENDENT_CTX` record to be claimed by exactly one
template record (as its pattern, one of its parameters, or a local or
parameter of its pattern), a template's parameters to be type or
non-type parameter records of its context in index order at its depth,
and its pattern an earlier unclaimed dependent function of the same
name, context and owner. An expression's `deps` byte may hold only the
type, value and instantiation bits (an unexpanded pack or an error
falls back). Implicit instantiations made in the interface are skipped
by the writer (`spec_info` kind implicit instantiation or undeclared);
a carried body or initializer that refers to one ("a body referring to
a declaration outside the image") makes the unit fall back. Every
other template kind falls back with the reasons tested in
`kind2_declaration_image_fallback`. Follow-ups, in order: class
templates and their members (records with dependent bases and
`instantiated_from` links), member function templates, specialization
reference records (bodies that call a template), unresolved-lookup and
dependent-member expression kinds, defaults and packs, then explicit
and partial specializations.

*Second slice (WP-1139): specialization references.* A carried body,
type or initializer may name a specialization of a carried template
that the unit made implicitly. The writer still skips the
specialization itself (and every member, parameter and local of it),
and writes in its place a *specialization reference record* (kind 18)
the first time something names it: the template record (an earlier
kind-17 record), a byte that is 1 when the unit completed the
specialization (a class specialization whose definition was
instantiated), and the template arguments in the converted, sugared
form the specialization keeps (`SpecInfo.args`), one per parameter: `0
qref` a type, `1 qref value:i128` an integral or `bool` value of that
type (an enumeration value, a pointer, a member pointer, a template,
a pack or any other argument falls back with "a template argument the
image does not carry"). The record has no name, context, lexical
context or previous declaration, so it is in no member list and no
lookup table; its location is the point of instantiation and its
flags the specialization's `USED` / `REFERENCED`. It is written where
it is first needed, so it may lie inside a class definition (it is not
a member: the validator and the class nesting skip it), and always
after its template and the types of its arguments. A *member
reference record* (kind 19) names a member of a class specialization:
the kind-18 record of the specialization and the pattern's member
record (`instantiated_from` for a templatable member, the
`instantiated_from_map` entry for a field or nested type); an
implicitly declared special member of a specialization (the default
constructor `A<int> a;` declares) has no pattern member and falls back
("an implicit member of a class template specialization"). A type naming a specialization is
type tag 16, `tst(template:u32, n, targ*, under:qref)`: the
`TemplateSpecializationType` as written (the template record, the
written arguments, and the underlying type (a written non-type
argument is an expression, not a value, so it falls back with "a
template argument the image does not carry"), a record type naming a
kind-18 record, or 0 for a dependent one written in a pattern, `X<T>`),
and type tag 17, `injected(template:u32)`, is the injected-class-name
type of a class template's pattern (`X<T>` inside `X`, the pointee of
`this`), and type tag 18, `subst(repl:qref, template:u32, index:u32,
pack:u32)`, is a substituted template type parameter in a
specialization's signature or member type: the replacement, the
template record whose parameter it replaced, the parameter index and
the pack element plus one (0 when not a pack element). A template name other than the template itself (qualified,
dependent, a template template parameter) falls back.

The importer rebuilds a reference by the entry points a use in source
would call, so it either finds the specialization (its own use, or the
instantiation another reference already made: `find_spec` keyed on the
canonical arguments, which an image-built template shares with a
replayed one) or makes it exactly as replay does: a function
specialization through `get_function_spec` (signature only), a class
specialization through `get_class_spec`, completed with
`require_complete` at the record when its byte says so (the unit
required it complete there or later; replay instantiates it at the
first such use, which the record precedes). A member reference is the
specialization's member made from the pattern member (`find_member`,
the specialization's `member_map`). Definitions are not carried: once
the bodies are built, each referenced function specialization or
member function marked `USED` is marked used again at its location
(`mark_used_at`), which instantiates a `constexpr` definition at once
and queues any other for the end of the importer's translation unit,
as replay's own use does. Constants the unit computed for
specializations stay in the constant section (WP-1046), so
initializers evaluated in the interface are not evaluated again.

The importer completes a specialization while it reads the records,
before the pattern's expressions (initializers, bit-field widths,
default member initializers) are set with the bodies (WP-1300). So:
a complete class specialization whose pattern has a bit-field or a
default member initializer is not carried ("a specialization of a
class template with a bit-field or a default member initializer"; it
would lose them); once the bodies and initializers are in, each
static data member of a complete class specialization still without
the in-class initializer its pattern now has is instantiated from it,
not made `inline` (`static const int w = sizeof(T);`, eager at the
class's instantiation, [temp.inst]/3); and each static data member or
member referenced or used is referenced again (`note_var_referenced`,
unevaluated when the unit only referenced it), so a constant one is
instantiated at once even when the unit's body only names it without
odr-using it (`B<2>::v`, [basic.def.odr]/4), as replay's reference
does.

*Second slice (WP-1139): class templates.* Template kind 1 is a class
template. The parser's order is kept: the template parameters, the
pattern (a class record with `DEPENDENT_CTX`, rebuilt after
`mark_pattern_next` as a function pattern is), then the template
record *immediately after the pattern record* (the class head creates
the template before the members are parsed, so the injected-class-name
and `X<T>` resolve inside the body), then the members up to the
pattern's end. The template record is the one record inside a class
definition that is not its member: its context is the pattern's.
Members of the pattern (fields, member functions defined in the class
or declared there and defined out of line, static data members and
their out-of-line definitions, typedefs and alias declarations, nested
classes and enumerations) are ordinary records carrying
`DEPENDENT_CTX`; a member function's body is always carried, as a
function pattern's is (it is instantiated, never emitted, so the
"non-inline body is braces only" rule does not apply). The validator
claims them through their class: a dependent record whose context is a
dependent class is claimed by that class, which must itself be claimed
(as a template's pattern, or as a member of a claimed class). An
out-of-line member definition (`template<class T> int X<T>::f() {}`)
carries the parameters of its own template-head as dependent
parameter records immediately before it, at depth 0 in index order;
the run's context is the definition's lexical context (the namespace
the definition is written in, or none at file scope), and the
definition claims it (there is no template record: sema
keeps no list for an out-of-line head, the body names the parameters
directly, and substitution maps them by depth and index). Since
WP-1307 the class may also be a partial specialization
(`template<class R, class... A> R F<R(A...)>::call(A... a) const`);
a member of a class nested in a template ("an out-of-line member of a
class nested in a template") still falls back. A parameter whose
late-parsed default argument ([class.mem]/7) was parsed is written as
an ordinary one (without `DEFAULT_ARG_PENDING`, which instantiation
drops too). Tests: `image-partial-member-run.cpp`. Dependent
base specifiers (a type parameter, `B<T>`) are base records whose
type is dependent; the validator accepts them in a dependent class
only. The injected-class-name is not a record: lookup finds it through
the pattern (`injected_class_for`), and type tag 17 rebuilds its type
with `injected_type_of`. A default type template argument (`class U =
W<T>`) is carried in the type parameter's record and rebuilt with the
parameter, before the pattern.

Two dependent expression kinds are carried (appended to the schema's
kind table): a dependent construction `T(a, b)`
(`CXXUnresolvedConstructExpr`; its written type and argument list, aux
1 for a list-initialization; a braced argument `T(a, {b})` falls
back) and a dependent member access `this->x`, `t.f`
(`CXXDependentScopeMemberExpr`; the base expression and the member
name, an index into `pnames` since a dependent member name is no
declaration of the image and is not gated; bits 1 for `->`, and the
operator location as cold annotation 16). Mem-initializers of a
constructor of a class template live in sema side tables
(`pattern_mem_inits`), which the image carries since WP-1310 (see
"Pattern mem-initializers" below; before, such a constructor fell back,
since instantiating it without them would silently default-initialize
its bases and members). A non-template class's
`Base` ctor-initializer names its base by the type as written (`B<int>`,
an alias), which the writer matches canonically against the base list.

Still falling back after the second slice: member templates and
templates nested in templates (depth above 0), explicit and partial
specializations (of a class or a member), friend declarations in a
class template, dependent array bounds and other types holding an
expression, non-type template arguments written in a type, default
non-type template arguments, packs, mem-initializers of class
template constructors (carried since WP-1310), implicit special members of specializations,
variable templates, unresolved-lookup expressions (a dependent call
`g(t)`), a qualified or `template`-keyword dependent member access
(`B<T>::x`, `t.T::f()`, `t.template get<int>()`), pseudo-destructor
calls, and every construct the first slice lists.

*Third slice (WP-1279): packs, defaults, alias and variable templates,
unresolved names.* Carried since the third slice:

- *Packs.* A template parameter record's flag byte has bit 1 for a
  pack (a type parameter's byte keeps bit 0 for `typename`; a
  non-type parameter's record gains a flag byte after its index, bit 0
  a pack). A function parameter pack of a pattern is a parameter
  record whose `ParamData::PACK` bit the validator accepts in a
  `DEPENDENT_CTX` record only. A pack expansion type `Ts...` is type
  tag 19, `pack(pattern:qref)` (one with a known expansion count is
  WP-1300's, "Expressions in types"); `sizeof...(Ts)` (`SizeOfPackExpr`, the pack's parameter
  record, which must be a pack), `PackExpansionExpr` (its pattern) and
  a fold `(0 + ... + f(xs))` (`CXXFoldExpr`: both optional operands,
  the operator in aux as for a binary operator, the `...`
  location as cold annotation 64) are appended to the kind table. An
  unexpanded pack in an expression's `deps` byte is accepted.
- *Placeholder types in patterns.* A dependent `auto` or
  `decltype(auto)` (a return type deduced at instantiation) is type
  tag 20, `auto(k:u8)`: bit 0 for `decltype(auto)`, bit 1 when the
  placeholder was deduced to the dependent type. A constrained
  placeholder, `__auto_type` and a deduced non-dependent placeholder
  fall back.
- *Default arguments of non-type parameters.* The non-type parameter
  record's flag byte has bit 1 when the default carries a canonical
  profile, followed by `default:u32`, an expression record (or none).
  The expression is a body root of the parameter, written with the
  bodies and patched into the record; the reader builds the parameter
  without it and, once the bodies are built, gives it the expression
  through `set_template_default`, recomputing the profile with
  `profile_expr` when the bit says the writer's argument had one (as
  `act_on_template_default` stores it). A default inherited from an
  earlier declaration is not written again: the reader merges it as
  sema does (below, "Member templates (WP-1292)").
- *Alias templates.* Template kind 2: the pattern is the dependent
  alias record (a `Typedef` record with its alias bit) immediately
  before the template record; the template record carries the `using`
  location as a fifth location word (`using`, 0 for the other kinds),
  so errors in an alias's substitution and "template is declared
  here" point where replay puts them. A type naming an alias
  specialization is type tag 16 whose template is a kind-2 record and
  whose underlying type is the substituted pattern (an alias
  specialization is no declaration, so there is no kind-18 record).
- *Variable templates.* Template kind 3: the pattern is the dependent
  variable record immediately before the template record, its
  initializer an ordinary initializer root; the importer instantiates
  it as after a replay.
- *Named class specializations.* A specialization reference record
  may name a class specialization of `SpecKind::Undeclared` (only
  named, `using B = X<int>;`), which the importer names the same way.
- *Unresolved names.* A dependent call or reference through an
  unqualified, qualified or template-id name (`f(t)`, `iu::f(t)`,
  `g<T>(t)`, `size<T>`, `a + b` resolved to a namespace-scope
  operator function) is `UnresolvedLookupExpr`, appended to the kind
  table: the name (a `pnames` index, or `N_OP + op` for an
  operator-function-id), the candidate set found at definition time
  ([temp.dep.candidate]; each candidate a function, function template
  or variable template record, which the validator checks), and aux 1
  when argument-dependent lookup applies. Cold annotation 128 carries,
  for any expression, the written qualifier (`expr_nns`) and the
  explicit template argument list (`expr_targs`): the qualifier as an
  `Nns` field, the list as `0` for none or `1 + n` then three words per
  argument, `0 qref quals` a type or `1 expr profile` an expression
  record (with its profile bit); other argument kinds fall back with
  "a template argument the image does not carry". Cold annotation 256
  (WP-1284) is the location of the `template` keyword in a qualified
  dependent name (`T::template get<3>`, the templates' `template_kw`,
  [temp.names]/3); in a dependent member access (`t.template get<1>`)
  the keyword is `CXXDependentScopeMemberExpr` aux bit 2 (bit 1 is
  `->`), and the decoder rejects any other aux bit. Instantiation
  redoes lookup exactly as after replay, so argument-dependent lookup
  at the point of instantiation finds the importer's own functions.
  A name whose candidates include a class member, or one sema keeps
  extra state for (a using-shadow or naming class, `ule_found`; an
  implicit member reference, `member_refs`), falls back with "a
  template expression (...)", the form named in the parentheses
  (WP-1314).

The census separates the "a declaration in a template" reason: a
partial specialization ("an explicit or partial specialization"), a
member of a class the image does not carry ("a member of a class
outside the image", a cascade of that class's own reason), and the
rest. Still falling back after the third slice: concepts, constrained
parameters and templates, requires-clauses and requires-expressions,
template template parameters, deduction guides (member templates,
templates in templates, redeclared templates and inherited defaults are
carried since WP-1292, and explicit and partial specializations since
WP-1283, below), non-dependent references
with written template arguments (`size<int>`, `sq<3>`: "a template
argument list"), `DependentScopeDeclRefExpr` (`T::v`),
`UnresolvedMemberExpr`, the `template` keyword in a dependent name,
and dependent array bounds (pack expansions in base specifiers and
friends of classes are carried since WP-1292, below).
WP-1284 carries concepts, constraints, requires-expressions, dependent
names and types, `DependentScopeDeclRefExpr` and deduction guides (see
"Constraints", "Dependent types and names" and "Deduction guides"
below).

**Class template specializations (WP-1283).** `bmi_image/classtempl.rs`
carries a unit's explicit and partial class template specializations
([temp.expl.spec], [temp.spec.partial]). Each one is an ordinary
`Record` record with the template's name and context, listed in a
section after the scopes (record, template record, an optional
template-head, arguments), ascending by record. Neither is in the
lookup table: lookup finds the template.

- *Partial specializations* carry their template-head (the parameter
  records, which precede the record, and the `template`, `<` and `>`
  locations) and their converted arguments, which are types, values,
  or the head's non-type parameters (alone or expanded). The reader
  builds each with `class_partial_spec`, the builder the parser uses,
  so matching ([temp.spec.partial.match]) and partial ordering see the
  same partial specializations in the same order as after a replay.
  The injected-class-name of a partial specialization is a type record
  (tag `Injected` naming the partial's record).
- *Explicit specializations* carry their arguments as written; the
  reader finds or makes the specialization through `get_class_spec`,
  as a use in source does, and declares it explicit there (what
  `act_on_explicit_class_spec` does after its checks). One declared
  and never defined (`template <> struct B<void>;`) stays incomplete.
- A specialization may be declared in a reopened namespace or by a
  qualified name (`template <class T> struct N::A<T, 1>`): its lexical
  context is a namespace record before it (or the translation unit,
  `LEX_TU`), and its head's parameters are in that context.
- A written argument that is a `bool` or integer literal of the
  parameter's type is carried as its converted value (`integral_constant<bool,
  true>`): the two print and compare identically.

**Explicit instantiations (WP-1307).** `extern template struct
X<int>;` and `template struct X<int>;` of a carried class template's
primary pattern, at namespace scope or by a qualified name, make no
record: the writer skips the specialization and its instantiated
members as it skips an implicit instantiation, and writes an entry in
an image section of their own (after the local externs): the
template record, the arguments as written (`ArgRec`s), the
template-name's location, the lexical scope (a namespace record, or
`u32::MAX` for the translation unit) and a byte of owner bits (bit 0
purview, bit 1 exported) with bit 2 set for a definition. A Spec
reference to an explicitly instantiated class is written as an
implicit one's (its importer-side kind comes from the rerun). The
reader reruns each entry in order through `act_on_explicit_class_inst`
(the parser's action), in its lexical scope and with the recorded
owner, once every record and body is built and before the used
specializations are marked used; the members a definition instantiates
are then the replay's. Running at the end of the unit instead of at the
directive changes nothing a valid unit observes ([temp.spec.partial.general]/1,
[temp.expl.spec]/7, [temp.point]/7). The validator checks each entry
names a class template record, a namespace record or `u32::MAX`, and
valid argument types. Still falling back: attributes on the directive
(the attributed reason), an instantiation of a partial specialization
or member template ("an explicit instantiation of a partial
specialization"), of a constrained template, with an argument the
image does not carry ("an explicit instantiation argument the image
does not carry"), in a class or other scope ("an explicit
instantiation in an unsupported scope"; in a namespace that is itself
not carried, the cascade "an explicit instantiation in a namespace
outside the image"), and explicit instantiations of
function and variable templates ("a template specialization").

**Pattern mem-initializers (WP-1310).** A constructor definition in a
class template (or a constructor template's pattern) keeps its
mem-initializers as parsed (`pattern_mem_inits`), not as
`CtorInit`s: each instantiation substitutes into them and checks the
result as the parser's action would ([temp.inst]/4,
[class.base.init]). A section of their own (after the explicit
instantiations) lists, per such constructor record in ascending order,
the `:` location and each mem-initializer: its mem-initializer-id (an
index into `pnames` for an identifier, or the type the parser
resolved), where it was written, whether it is a pack expansion
(`B(args)...`) and its initializer (`= expression`, a parenthesized
list with its parentheses, or a braced-init-list, nested at most 64
deep). The expressions are records of the bodies section. The
validator requires a constructor record, valid expression slots,
names and types; the reader installs the list on the constructor once
the bodies are built, before anything can instantiate it. Still
falling back: a mem-initializer whose definition-time check failed
("a mem-initializer that failed its definition-time check", already
diagnosed) and a designated braced-init-list ("a designated
initializer in a mem-initializer"). A constructor template's
function-template record is named `N_CTOR`, as its pattern. A pattern
record's used/referenced flags are applied after its template record
is built (they are monotone, and the template copies its pattern's),
so a referenced pattern does not mark the template. Tests:
`pch-pattern-inits-run.cpp`, `image-pattern-inits-run.cpp` and their
`-errors` pairs, `kind2_declaration_image_pattern_inits`.

**Forms carried by WP-1314 (libc++).** The census reason "a template
expression" is split by form: "a template expression (an unresolved
member reference)", "(a substituted parameter pack)", "(pack
indexing)", "(an expansion statement's element)", "(a name found
through a using-declaration)", "(an implicit member
reference)", "(a name that found a class member)" and "(a name that
found something other than a function or a function or variable
template)"; "a local declaration the image does not carry" names a
local class or enumeration, typedef or alias, using-declaration or
structured binding on its own. Carried:

- *Explicit template arguments of a resolved name* (`e_v<double>`,
  `pick<int>(3)`, [temp.arg.explicit]/1): the `DeclRef` side gains a
  third field, the written argument list as a cold-annotation-128
  `TA` field (`0` for none); the importer puts it on the name's
  `DeclRefExtra` as the parser does.
- *Block-scope static assertions* ([dcl.pre]/10; libc++'s
  `static_assert` in `std::declval`, `__constexpr_wmemchr`,
  `__countl_zero`): a local record with the `Assert` payload (no name,
  no previous declaration, its context the function), the condition
  and message expression records patched in as for a member assertion.
  The unit checked a non-dependent one; a pattern's is checked again by
  each instantiation of its function, so a failing one is diagnosed at
  that instantiation exactly as after replay. A failed one is invalid
  and falls back ("an invalid local declaration").
- *Block-scope typedef and alias declarations* ([dcl.typedef]; 350 of
  libc++'s refused declarations, `using _Ip = ...;` in a body): a local
  record with the `Typedef` payload (its name a `pnames` index, no
  previous declaration, its underlying type written before the record);
  the importer builds it in its function as the parser does, and an
  instantiation substitutes it from the pattern's. A local alias
  template falls back ("a local alias template").
- *Unresolved names that found class members* ([temp.dep.candidate],
  [class.member.lookup]; libc++'s `__test<_Tp>(0)` in
  `__is_referenceable_impl`, `__invoke`'s and `pointer_traits`'
  detection idioms): an `UnresolvedLookupExpr` whose lookup found
  member functions or member function or variable templates is carried
  as one that found namespace members. Its naming class, when the
  lookup recorded one (`holder::pick(t)`), is the new cold annotation
  1024 `NAMING`, an `OD(Ctx)` field valid only on that kind; the
  importer records the found set and naming class as the parser does,
  so access is checked against the same class at each instantiation.
  A lookup through a using-declaration (the found set differs from the
  name's declarations) still falls back ("a template expression (a
  name found through a using-declaration)"), as does an implicit
  member reference.
- *Dependent `decltype` bases* (`struct is_ref : decltype(f<T>(0))`):
  a dependent `decltype` type is a valid dependent base of a pattern.
- *A class sema completed from its first declaration across a
  reopened namespace* (`template <> struct traits<char>;` in one
  namespace block, its definition in the next): the definition's
  record sits at the first declaration, so the writer keeps the class
  open across the reopened namespace's record (a `Namespace` with a
  previous declaration, while members are still to come), which the
  validator accepts inside an open class. Any other declaration between
  the first declaration and the members still closes the class ("a
  class member outside its class definition"; libc++'s
  `char_traits<char>`, whose `<__fwd/string.h>` declares the other
  specializations in between).

**Defaulted comparisons (WP-1314).** A comparison operator defaulted on
its first declaration (`friend constexpr bool operator==(partial_ordering,
partial_ordering) noexcept = default;`, a member `operator==`, one in a
class template; [class.compare.default]) is an ordinary function record:
its type, `constexpr`, deletedness and deferred exception specification
are what the unit's class-completion check left. The `defcmps` section
carries what sema keeps beside it: the `default` keyword's location
(`special.cmp.default_loc`, where the synthesized body goes and its
notes point) and the unqualified operator lookups captured at the
`= default` (`special.cmp.lookups`, records; `NONE` when the unit
captured none). The importer puts both back after the records and
before the explicit instantiations, so a use defines the body as after
a replay, an instantiation copies them from its pattern, and a
deferred exception specification resolves the same way. Falls back:
an invalid one, a deduced return type ("a defaulted comparison with a
deduced return type"), a `<=>` that declared an implicit `operator==`,
and a captured lookup through a using-declaration or of a declaration
outside the image. The old reason "a defaulted comparison or an
out-of-line defaulted function" is now "an out-of-line defaulted
function". Tests: `pch-defaulted-cmp-*.cpp`,
`image-defaulted-cmp-run.cpp`.

Tests: `pch-local-static-assert-*.cpp`, `pch-local-typedef-*.cpp`,
`pch-member-names-*.cpp`, `image-member-names-run.cpp`,
`pch-reopened-spec-run.cpp`, `image-reopened-spec-run.cpp`,
`image-local-static-assert-*.cpp`, `image-local-typedef-run.cpp`,
`local_static_asserts_and_explicit_args_take_the_image`.

**Smaller forms carried by WP-1310.** Attributes on block-scope
declarations (libstdc++'s `void __terminate() noexcept
__attribute__((__noreturn__))` redeclared in a body) are carried as a
namespace-scope declaration's are, for the attribute kinds the table
lists; an invalid local declaration falls back with "an invalid local
declaration", a local with another flag with "a local declaration with
flags the image does not carry". A class an elaborated-type-specifier
in a member declaration introduces (`struct S { struct D *p; };`,
[dcl.type.elab]/3; `<__locale_t.h>`) is a record of the enclosing
namespace that falls between the class's members: it closes no open
class, and the validator accepts a class declaration with no lexical
context inside a class definition. Parameters of an adjusted type (`<cstdio>`'s `char[L_tmpnam]`,
`<csetjmp>`'s `struct __jmp_buf_tag __env[1]`) carry the type as
written (see the function payload; a requires-expression's parameter
still falls back, its section has no field for it). A builtin function's record
(declared lazily at its first use) may fall among the members of the
class whose member function first used it; the validator accepts it
anywhere (before, the writer's own check refused such an image as "an
internal image inconsistency", `<typeinfo>`, `<limits>`). Enumerations in class templates
(`template <class T> struct is_integral { enum { value = 0 }; };`) carry
their enumerators as dependent (see the enumerator payload). The type
tags for `__int128_t`, `__uint128_t` and `__builtin_va_list`, `aligned`
attributes and locations in the predefined buffers are described under
"Types", "Attributes" and "File table". Tests: `pch-local-attrs-run.cpp`,
`pch-predef-aligned-*.cpp`, `image-predef-aligned-*.cpp`,
`pch-member-enums-*.cpp`, `image-member-enums-*.cpp`,
`pch-builtin-in-class-run.cpp`, `image-builtin-in-class-run.cpp`,
`local_attributes_and_pattern_inits_are_carried`.

**Forms carried by WP-1320.** A redeclaration of a replaceable global
allocation or deallocation function (`void *operator new(std::size_t)`
in libstdc++'s `<new>`, [new.delete]/2) has the implicit declaration sema
makes as its previous declaration; the writer gives it no previous record,
and the reader passes every non-dependent global `operator new` /
`operator delete` record without one through
`act_on_global_new_delete_decl`, as the parser does, which declares the
implicit functions at the same point and finds the one it redeclares. A
variable of a dependent type directly initialized (`fpos __pos(*this);`
in `<bits/postypes.h>`, [dcl.init.general]/16) keeps its `ParenListExpr`
(expression kind 59, its expressions and end location; a braced argument
still falls back). Tests: `pch-global-new-paren-*.cpp`,
`image-paren-init-run.cpp`, `global_new_delete_and_paren_inits_are_carried`.
A non-type template parameter's default argument is carried as an
expression section (`Nttp` flag bit 2, `sect`; its `default` is then the
section) whenever the section can carry it, else as before; the reader
builds it before the records that need it and sets it with its profile.
That lets an explicit specialization that takes the default (`template<>
struct __promote<double>` in `<ext/type_traits.h>`, whose `bool =
__is_integer<_Tp>::__value`, [temp.arg.general]/1) be carried: the
reader rebuilds the default when it converts the arguments. The writer
still refuses ("an explicit specialization using a non-type default
argument") when the default is inherited from an earlier declaration, is
not a section, or instantiated anything at the specialization or inside
the default's text that is not a class without static data members and
member functions (those would be completed only with the bodies, after
the records). Tests: `pch-nttp-default-spec-*.cpp`,
`image-nttp-default-spec-run.cpp`, `nttp_default_explicit_specs_are_carried`.
A dependent class template specialization written with fewer arguments
than parameters (`__is_signed_helper<_Tp>` in `<type_traits>`) has as
its underlying type the dependent specialization completed with the
defaults (`__is_signed_helper<_Tp, is_arithmetic<_Tp>::value>`,
`dependent_spec_canon`, [temp.arg.general]/9); the writer carries that
`TyRec::Tst` (of the same template, itself without an underlying type)
as `under`, and the reader accepts it there. A dependent member type
(`typename X<T>::type`, a `DepTy::Name` or `DepTy::Tmpl`) is a valid
dependent base class. The remaining refusals of the old reason name
their form ("a specialization type: ..."). Tests:
`pch-dependent-default-tst-*.cpp`, `image-dependent-default-tst-run.cpp`,
`dependent_default_specializations_are_carried`.
Explicit class instantiations are listed after the walk, in source
order, not where the walk meets their declaration: that declaration is
the specialization's, made where it was first named (`typedef
basic_ios<char> ios;` in `<iosfwd>`), usually before the namespace block
the instantiation is written in (`extern template class
basic_ios<char>;` in `<bits/basic_ios.tcc>`), which was then not yet
recorded ("an explicit instantiation in a namespace outside the image").
The reader runs the list in order after the bodies, as before. Tests:
`pch-reopened-extern-template-*.cpp`, `image-reopened-explinst-run.cpp`.
A pseudo-destructor call keeps its `CXXPseudoDestructorExpr` (expression
kind 60: the object, the destroyed type, `->` and qualified in aux;
`noexcept(declval<_Tp&>().~_Tp())` in `<type_traits>`). A new-expression
keeps its `CXXNewExpr` (kind 61, with a side record of the allocated
type, array bound, initializer, placement arguments, allocation and
deallocation functions and type location; the initialization style in
aux, `::`, array form and written array in the bits). One of the
implicitly declared global allocation and deallocation functions
([basic.stc.dynamic.general]/2) that a body names is an `Implicit` record
of kind 7 (`GLOBAL_OP_KIND`), unnamed and at global scope, listed in the
scope section's allocation function list with its operator and its
position among the implicit `operator` functions of that name sema
declares (`declare_global_new_delete`, in declaration order); the
importer declares them as a new-expression would and takes the one at
that position. A declaration reference's side record also carries its
explicit template arguments (`std::declval<void*>()`, an `F::TA` token;
none is a zero count), which were refused as "a template argument list".
Tests: `pch-new-dtor-targs-*.cpp`, `image-new-dtor-targs-run.cpp`,
`global_op_records`.
A block-scope `static_assert`, typedef or alias declaration
(`declval`'s assertion and `__call_is_nt`'s `using _Up = ...` in
`<type_traits>`, [dcl.pre]/10, [dcl.typedef]/2) is a local record like a
variable: its context is the function record, it is named from the
private name table and its `DeclStmt` names it (`Dc::Local`). The
assertion's payload is the namespace-scope one, patched by the roots
walk; the alias records its underlying type before itself (its own type
names it). Validation accepts `Assert` and `Typedef` payloads among the
locals. Tests: `pch-local-decls-*.cpp`, `image-local-decls-run.cpp`,
`local_asserts_and_aliases_are_carried`.
A GNU complex type (`_Complex double`, `__complex__ float` in
`<complex>`) is a type record of tag 23 holding its element type; the
reader interns it with `complex`. Tests: `pch-complex-types-*.cpp`,
`image-complex-types-run.cpp`, `complex_types_are_carried`.
A canonical template type parameter type that no parameter declaration
names (the adjusted type of a dependent array parameter, `const T a[2]`,
[dcl.fct]/5) is a type record of tag 24 holding its depth, index (both
u32) and pack byte; the reader interns it with
`canon_template_type_parm`. Tests: `pch-dependent-array-params-*.cpp`,
`image-dependent-array-params-run.cpp`,
`dependent_array_parameters_are_carried`.
The §6 deduplication keys of a header loaded from the image are WP-1353's
(below).

**Forms carried by WP-1342 (template declarations).** The writer is
`bmi_image/tttp.rs` (template template parameters and template-name
arguments) plus additions to `classtempl.rs`, `constraints.rs`,
`deptypes.rs`, `spec.rs` and `body.rs`. The census reason "a template
parameter the image does not carry" is now "a template parameter outside
the image" (a parameter whose own record was refused for another
reason, counted as a cascade); "a template" no longer names a template
template parameter.

- *Template template parameters* ([temp.param]/1, [temp.arg.template]).
  A `TemplateTemplateParm` is a dependent record of kind 27 (`K_TTTP`),
  after the records of its own parameters (one level deeper, in the
  same context): u16 depth, u16 index, a flag byte (bit 0 pack, bit 1
  `typename` written, bits 2.. the parameter kind: 0 type, 1 variable,
  2 concept template, P2841), u32 first token (or `NONE`), u32
  `template`, `<` and `>` locations, u16 depth of its own head, a
  counted list of its parameter records, and its default as a record
  and a pack word (`NONE`, 0 for none). The validator claims its
  parameters (earlier, same context, depth = its head's depth, index =
  position) and checks the default names an earlier template or
  template template parameter record. The reader builds the head with
  `template_param_list`, the parameter with `template_template_parm` and
  `set_ttp_kind`, and the default with `template_name_for`. A template
  record's, a partial specialization head's and an enclosing
  parameter's lists may name it where they name `Ttp`/`Nttp` records.
  Refused, each under its own reason: a parameter written without
  `class` or `typename`, a constrained one, one whose own head has a
  requires-clause, and a default that is not a template name.
- *Template-name arguments.* An argument naming a template is
  `ArgRec::Template(record, pack)` (argument tag 3: u32 record, u32
  pack word: 0 plain, 1 `TT...`, k + 2 an expansion of k elements); the
  record is a class, alias, variable, function or concept template or
  a template template parameter. A dependent template name
  `nns::template name` (`__xref<_Tp>::template __apply`) is
  `ArgRec::DepTemplate(type, pack)` (tag 4): `type` is a dependent type
  record `nns::template name<>` (`DepTy::Tmpl` without arguments) that
  the writer adds for it, from whose `DependentTemplateSpecialization`
  the reader takes the qualifier and name back as
  `TemplateName::DependentIdentifier`. In the class specialization
  section the same arguments are tags 4 and 5; in a body's template
  argument list (`F::TA`) they are kinds 3 (record, pack) and 4 (type,
  pack), not inside an expression section. `TT<args>` of a template
  template parameter is a dependent `TyRec::Tst` naming the parameter's
  record, its arguments as written, no underlying type.
- *Constrained templates* ([temp.constr.decl]). A class or variable
  template whose parameters are constrained or whose head has a
  requires-clause, and every partial specialization with either, is
  carried: a partial specialization's requires-clause is a constraint
  record of kind `K_HEAD` (1) on the specialization's record, built
  into the specialization's parameter list by the reader. (Explicit
  specializations of a constrained template stay refused, "a
  specialization of a constrained template", WP-1344.)
- *Member partial specializations* ([temp.spec.partial.general]/7). A
  partial specialization of a member template, of a class or of a
  class template pattern at any depth, is listed in the class
  specialization section like a namespace-scope one; its head's depth
  is its template's (`Template` record `depth`), which the validator
  requires of each head parameter and the reader passes to
  `template_param_list`. Its parameters' context is the class.
- *Qualified and parameter class template deduction*
  ([dcl.type.class.deduct]). `DepTy::Deduced` (dependent type tag
  `T_DEDUCED`) gains, after the deduced type, a byte: 0 unqualified, 1
  qualified, 2 qualified with `template`, then (for 1 and 2) the
  qualifier's steps; the reader rebuilds `TemplateName::Qualified` over
  the template's name. Its template may also be a type template
  template parameter's record (unqualified), named with
  `template_name_for` (`decltype(_Cont(std::declval<_Rg>()))`).
- *Names found through using-declarations* ([namespace.udecl]/1). An
  unresolved name whose lookup found using-shadows (`using
  std::pow;` then `pow(x, y)` in a function template) carries what it
  found as the body cold annotation `COLD_FOUND` (bit 2048, after
  `COLD_NAMING`, which it requires): an `F::DL` of `Dc::Lookup` as long
  as the name's declarations, the shadow records (or the declaration
  where lookup found it directly). The reader restores sema's
  `ule_found` from it, so instantiation sees the same set.

Tests: `pch-template-template-parm-run.cpp`,
`pch-constrained-partial-spec-*.cpp`, `pch-member-partial-spec-*.cpp`,
`pch-using-shadow-call-*.cpp` (cpp.include, with `Inputs/` headers);
`template_template_parameters_are_carried`,
`constrained_partial_specializations_are_carried`,
`member_partial_specializations_are_carried`,
`using_shadow_calls_and_qualified_deduction_are_carried` (kiln_driver
`pch_decls`); `template_template_parameters_round_trip`,
`rejects_corrupt_template_template_parameters`, and the qualified
`Deduced` cases of `dependent_types_round_trip` /
`rejects_corrupt_dependent_types` (kiln_sema).

**Forms carried by WP-1343 (function bodies).** Each form below was a
census refusal of a function body or of a type in a declaration.
- *Parameters in types.* A function's parameters get their section
  numbers before its type is written, so a trailing return type or
  `noexcept` operand that names one (`-> decltype(__x + __y)`,
  [basic.scope.param]) is the parameter word `PARAM | k`. A type inside a
  requires-expression may name the requirement's own parameters
  (`requires (T& t) { typename decltype(t.f())::type; }`): those are
  `PARAM | REQP | k`, numbered in the scope section's requirement
  parameter list. A local declaration in a type stays refused ("an
  expression in a type naming a parameter or local declaration").
- *New expression kinds* (appended to `EXPR_KINDS`, 62-68):
  62 `CXXRewrittenBinaryOperator` (`[E, EL]`: the semantic form and the
  written operands; aux the written operator, [over.match.oper]/8); 63
  `CXXDeleteExpr` (`[E, OD(Value)]`: the operand and the deallocation
  function; aux whether it is passed the size and the alignment, at most
  3; `::` and `delete[]` in the flags); 64 `CXXTypeidExpr`
  (`[Qt, Z]` for a type operand, flag bit 4, else `[E, Z]`; aux is the
  potentially-evaluated bit); 65 `CXXDynamicCastExpr` and 66
  `BuiltinBitCastExpr` (casts: `[E, Path]`, aux the cast kind); 67
  `AtomicExpr` (`[EL, Z]`, aux a `__atomic_*` / `__c11_atomic_*` form
  `builtin::atomic::form` accepts); 68 `ArrayTypeTraitExpr` (`[Qt, Z]`,
  aux `__array_rank` or `__array_extent`).
- *New statement kinds* (`STMT_KINDS` 18-19): 18 `CXXTryStmt` (`[S, SL]`:
  the block and the handlers); 19 `CXXCatchStmt` (`[OD(Var), S]`: the
  exception declaration, none for `...`, and the handler). The exception
  variable is a local `Var` record with spec bit 11 (`exception_var`).
- *Cold annotations* (from the end of the bit set, after WP-1342's 2048 `COLD_FOUND`;
  `COLD_ALL` is 32767): 4096 `COLD_MEMBER` on a `MemberExpr` only: its qualifier, found
  declaration and explicit template arguments (`(Nns, OD(Found), TA)`,
  `this->Base::m()`); 8192 `COLD_ARRAY` on an `ArrayTypeTraitExpr` only:
  the dimension expression and the value's low and high words (`(OE, Raw,
  Raw)`); 16384 `COLD_PACK` on an `IntegerLiteral` only: the pack a
  `sizeof...` folded to it came from (`D(Pack)`). Each bit on any other
  kind is invalid.
- *Union initializers.* The `InitList` side record gains a third field,
  the initialized union member (`OD(Field)`, `U u{5}`, [dcl.init.aggr]/16).
- *Block-scope using family.* A using-declaration, its shadows, a
  using-directive, a using-enum-declaration and a namespace alias at block
  scope ([namespace.udecl], [enum.udecl], [namespace.alias]) are local
  records like a block-scope typedef: context the function record, name
  from the private table, `DeclStmt` naming them (`Dc::Local`). A shadow's
  target, a directive's namespace and an alias's namespace are earlier
  non-local records (a shadow's of the same name); a using-enum names an
  earlier defined enumeration. The importer never adds a block-scope
  directive to the function's own using-directive list.
- *Structured bindings.* A block-scope decomposition is its `Var` record
  followed by one `Binding` record per name (decl tag 28, `K_BINDING`, after WP-1342's 27 `K_TTTP`: a
  flag byte and type, then the decomposition variable, the holding
  variable or `NONE`, and the binding expression set from the body's root
  list, `Root::Binding`); bindings are local only ("a structured binding
  outside a block" otherwise) and a binding pack stays refused. The
  importer groups consecutive bindings by decomposition and sets them
  with `set_decomposition` / `set_binding`.
- *Types.* Tag 25 `Placeholder` (a flag byte, bit 0 `decltype(auto)` and
  bit 1 pack, at most 3; the deduced type, `NONE` or the dependent mark
  `{ty: NONE, quals: 0}`; the type-constraint's concept record, `NONE` for
  none; its written arguments, none without a concept): a deduced or
  constrained `auto` (`std::integral auto`). Tag 26 `Atomic` (the value
  type): `_Atomic(T)`.
- *Constants.* A body constant may be tag 4, a pointer (its type; a base
  of byte 0 variable record, 1 function record, 2 lifetime-extended
  temporary record, 3 `type_info` type, 4 earlier aggregate constant; the
  byte offset as a `u64`; an optional designated type; the past-the-end
  byte), or tag 5, an aggregate as the Oven freezes it (its type, its
  bytes, then the pointer-like subobjects as (offset, earlier pointer or
  null constant), the padding ranges and the active union members as
  (offset, `Field` record)). A type's or argument's section carries
  neither when it names a declaration. A variable's evaluated value stays
  recomputed. `strong_ordering::less` and `source_location::current()`
  are such constants.

The forms still refused under this WP's reasons: lambda expressions and
local classes and enumerations (their records are classes whose context
is a function), a designated initializer ("an expression the image does
not carry yet (a designated initializer)"), a default argument used on a
call of a function template specialization ("a local declaration the
image does not carry (a specialization's parameter)"), and a dependent
vector type. Tests: `pch-body-forms-*.cpp`, `image-body-forms-run.cpp`,
`body_expression_forms_are_carried`,
`try_blocks_and_local_usings_are_carried`,
`structured_bindings_and_union_inits_are_carried`,
`constants_atomic_and_placeholder_types_are_carried`,
`specialization_default_argument_is_refused`.

**Forms carried by WP-1344 (specializations).** The writer's additions
are `bmi_image/constrspec.rs`, `specinit.rs`, `selfbase.rs` and
`builtintempl.rs`, plus `spec.rs`, `classtempl.rs`, `explinst.rs` and
`body.rs`. Census reasons that named one cause for several forms are
split by form (`<reason>: <form>`), so what remains says what it is.

- *Packs in a specialization's arguments* ([temp.arg.general]/9).
  A `Spec` record's converted argument list may hold
  `ArgRec::Pack(n)` (argument tag 5, u32 n): the next n arguments are
  the pack's elements, flat, only at the top level of the list
  (`_List<unsigned char, unsigned short, ...>`). The validator checks
  each element against the pack parameter's kind. The reader passes a
  class template's elements flat as written arguments to
  `get_class_spec` and a function template's as one
  `TemplateArg::pack`.
- *Values of enumeration type* ([temp.arg.nontype]). An
  `ArgRec::Value` (in a `Spec` record, an explicit specialization or
  an explicit instantiation) may have an enumeration type record as
  well as an unqualified builtin integral type (`mtx<multi>`,
  `rf<fmt::map, T>`).
- *Explicit specialization arguments as written*
  ([temp.expl.spec]). An explicit specialization's argument that is
  not a converted integer (`get<0>` for a `size_t` parameter,
  `gcd<0, 0>` for `long` ones) is an expression section: class
  specialization argument tag 6 (u32 section, u8 profile 0 or 1), also
  inside a partial specialization's pack. The validator requires its
  section's records and types before the specialization's record; the
  reader builds the section while it builds the record (its types
  first, as a section cannot be built while another is) and
  `get_class_spec` converts it again.
- *Non-type parameter pack expansions in a partial specialization's
  arguments* (`__static_max<_I0, _I1, _In...>`): the same tag 6, a
  section holding the `PackExpansionExpr`.
- *Specializations of constrained templates* ([temp.names]/8). A
  specialization of a template whose constraints are its own (a
  requires-clause on its head, type-constraints on its parameters) is
  carried: the unit made it, so it satisfied them, and the reader's
  `get_class_spec` finds the same specialization before the
  constraints are patched in. A specialization the unit completed
  stays refused when its template has a constrained partial
  specialization or a constrained member (a trailing requires-clause,
  a member template's constrained head), at any depth of the pattern:
  the reader would complete it before those constraints exist
  ("a specialization of a constrained template: completed, with a
  constrained partial specialization" / "... with a constrained
  member").
- *Member typedefs of specializations*. A type naming a member
  typedef of a carried specialization is `TyRec::Typedef` of the
  member's `Member` record (whose pattern payload may be a `Typedef`),
  and a base naming one is checked as the specialization's
  (`__make_unsigned_selector`'s `using __type = ...` through a base).
- *Static data member initializers of class template patterns*
  ([class.static.data]/4). The in-class initializer of a static data
  member of a carried pattern is an expression section when one holds
  it: bit 31 (`INIT_SECT`) of the `Var` record's `spec` word, `init`
  then the section. The reader builds it with the record, so a
  specialization the reader completes while it builds the records
  (`template <class T, bool = (A<T>::n > 2)> struct B;` named in a
  completed class) sees the value. This fixes a pre-existing
  divergence (the default argument was converted without the
  initializer).
- *Self-referential pattern bases* (`__common_reference_impl`,
  `template <class T> struct chain : chain<T *> {};`). A class
  template's record is just after its pattern's, so a pattern base
  naming the template names a later record: the pattern's `RecDef`
  then has bit 63 (`BASES_AFTER`, above the `DefinitionBits`), the
  validator checks its bases as if they were record `i + 1`'s (that
  record must be the template, its pattern `i`), and the reader gives
  the class its bases right after it builds the template, before any
  member.
- *Builtin templates* (`__make_integer_seq`, `__type_pack_element`).
  Every unit declares both, so a dependent specialization names the
  importer's: an `Implicit` record of kind 8 or 9 (unnamed, at global
  scope, no flags or owner), named by a `TyRec::Tst` with no
  underlying type and its arguments as written. A non-dependent one
  stays refused ("a builtin template's specialization that is not
  dependent").
- *Exception specifications naming specializations*. The writer's
  `prescan` also writes a function's whole type, so a specialization
  its `noexcept` expression names is recorded before the function's
  record (`noexcept(noexcept(std::declval<const _Swap&>()(...)))`).
- *Order*: the specialization reference records a body's walk names
  are written in source order (they were last-first), as replay
  instantiates them.

Tests: `pack_arguments_are_carried`,
`member_typedefs_of_partial_specializations_are_carried`,
`specializations_of_constrained_templates_are_carried`,
`explicit_specialization_value_arguments_are_carried`,
`self_referential_pattern_bases_are_carried`,
`static_member_initializers_read_during_completion`,
`partial_specialization_pack_arguments_are_carried`,
`exception_specifications_naming_specializations_are_carried`,
`builtin_template_specializations_are_carried` (kiln_driver
`pch_decls`: census without refusals, image equals replay under
`check`, the program runs); `kind2_declaration_image_fallback`
(modules_bmi: a completed specialization with a constrained member is
refused); `self_bases_and_builtin_templates_round_trip`,
`rejects_corrupt_self_bases_and_builtin_templates` (kiln_sema).

**Forms carried by WP-1345 (declarations and class members).** Each
form below was a census refusal of a declaration or class member; new
tags come from the end of each enumeration.
- *Dependent using-declarations.* `using B<T>::f;` and `using typename
  B<T>::type;` in a class template ([namespace.udecl], [temp.dep.type])
  are decl kind 29 `K_DEP_USING`: a flag byte (bit 0 `typename`) and the
  qualifier's steps; the name is the header's (an identifier, an
  operator, or `N_CTOR` for an inheriting declarator `using B<T>::B;`,
  [class.inhctor.init], which the importer names again from the rebuilt
  qualifier's canonical type and does not enter in lookup). The typename
  form's type is dependent type tag 6 `T_USING` (the record). A class
  template pattern whose inherited constructors all come from such
  declarators keeps `HAS_INHERITED_CTOR` in its definition bits. A pack
  and a conversion-function name stay refused.
- *Attributes.* `preferred_name(T)` (attribute op `Type`: the attrs
  section gains a trailing vector of type references), `diagnose_if(cond,
  msg, kind)` and `enable_if(cond, msg)` (conditions in the body root
  list, like the other expression attributes).
- *`explicit(expr)`.* Constraint-patch kind 4 `K_EXPLICIT` on a function
  record: the operand, patched into `cold.explicit_expr` with the
  constraints ([dcl.fct.spec]/3).
- *Names.* A literal operator (`operator""_x`, [over.literal]) is name
  word `N_LIT | k` (`N_LIT` = `0x8000_0000`, `k` the suffix's identifier);
  a conversion function template is named by `N_CONV` like its pattern.
- *Invented template parameters.* An abbreviated function template's
  invented type parameters ([dcl.fct]/22) carry `IMPLICIT` in their
  record flags.
- *Defaulted members of patterns.* A member of a class template pattern
  defaulted on its first declaration is a function record like any
  other; its definition is made when an instantiation uses it.
- *Implicit members of specializations.* Each implicitly declared
  special member of a complete class template specialization reference
  is an `Implicit` record whose context is that reference, in member
  order (a body naming one writes the members sema declared before it
  first, so overload candidates are listed as in replay); the importer,
  which instantiated the class at that reference, declares each again as
  sema does lazily and defines it when it was used. A specialization
  whose members have `explicit(expr)` is carried like any other: the
  importer resolves the specifier when overload resolution asks, after
  the constraint patches.
- *Explicit specializations named before their declaration.* When a
  declaration names an explicit specialization of a class template
  before `template <> struct X<...>` declares it (sema makes it at the
  first name and specializes it in place), the writer visits the
  specialization just before its first member, where replay declares
  it; the declarations in between name it through an incomplete
  specialization reference.
- *Anonymous unions and structs* ([class.union.anon]). The unnamed
  member class keeps `ANONYMOUS` (definition bit `ANON_BIT`) on a record
  with name `NONE`; its unnamed object member is decl kind 30
  `K_ANON_OBJ` (the class record), and each name it injects is decl kind
  31 `K_INDIRECT` (the object record; the importer finds the indirect
  field the class definition made, by name). A constructor's member
  initializer may name a member of an anonymous member (`Field`, init
  kind 2) or an indirect field (init kind 3).
- *Using-enum in a class* ([enum.udecl]). A non-dependent class's
  `using enum E;` is its `UsingEnum` record and the enumerator shadows
  it introduces are `Shadow` records in the class, each with its access.
  Other class-scope using-declarations stay refused.
- *Conversion names in dependent member access.* The name word of a
  `CXXDependentScopeMemberExpr` naming `operator T` (`__d.operator
  basic_string_view<C, T>()` in a requires-expression) is `0x8000_0000
  | ty << 3 | quals` (the type index below `2^27`); the importer names
  the conversion to the rebuilt type's canonical type.
- *Out-of-line members of nested classes.* A member of a class nested
  in a class template or partial specialization defined outside it
  (`template <class C> basic_ostream<C>::sentry::sentry(...)`) is
  preceded by every enclosing template-head's parameters, as WP-1292's
  out-of-line member templates are. An out-of-line partial
  specialization of a member class template (`template <class R> struct
  fb::res<R&> { ... };`) is a class record whose context is the closed
  class, with lexical context the namespace and no previous
  declaration.
- *Explicit object member functions* ([dcl.fct]/6). Function specifier
  bit 20 (`explicit_object`); the first parameter's flags carry
  `ParamData::EXPLICIT_OBJECT`, which no other parameter may.
- *Block-scope function declarations in templates.* `extern T* f();`
  in a member of a class template (`polymorphic_allocator()`'s
  `get_default_resource`) is a local function record written in
  declaration order with its previous declaration; sema keeps it out of
  `local_externs`, and so does the image.
- *Attributes on explicit instantiations.* Plain-word attributes written
  on `template class` / `extern template class` (`visibility`, libc++'s
  `_LIBCPP_EXTERN_TEMPLATE_TYPE_VIS`) are a second trailing vector of the
  attrs section, keyed by the instantiation's index in the explicit
  instantiation section; the importer appends them to the
  specialization after running the instantiation. Attributes the
  specialization took from its template's declarations are made again
  by the instantiation.
- *File-scope `asm`* ([dcl.asm]). Decl kind 32 `K_ASM`: the string
  literal, a body-section expression patched in with the bodies (the
  importer declares a placeholder and sets the string with
  `set_file_scope_asm_string`).

Still refused in this bucket: a non-word attribute with arguments on an
explicit instantiation (`deprecated("...")`; the argument-less
`__attribute__((__deprecated__))` is WP-1353's), a dependent using pack
or conversion-function name. (A non-dependent using-declaration of a
base member in a class, an explicit specialization of a member class
template defined out of class and a non-template out-of-line defaulted
special member are WP-1353's.) Tests: `crates/kiln_driver/tests/pch_decls.rs` and the corpus
pairs `cpp.include/pch-decl-forms-run.cpp`,
`module.interface/image-decl-forms-run.cpp`.

**Forms carried by WP-1352.** The writer's additions are
`bmi_image/latedef.rs`, `varspec.rs` and `fnspec.rs`, plus `spec.rs`,
`classtempl.rs` and `body.rs`. Four side sections follow `defcmps`, in
this order: `lheads`, `fexpl`, `fsuses` (the `cspecs` section gains
function entries).

- *Specializations declared, then defined* ([temp.expl.spec]/7,
  [temp.spec.partial]/1). Sema has one declaration for
  `template <> struct ch<char>;` and its later definition, so the walk
  meets it where it was declared, before the records its definition
  names. The listed record (`cspecs`) is then a declaration only; the
  definition is a second `Record` record with a definition and `prev`
  the listed record, written when the walk reaches the definition's
  first member. The validator requires the same name, the listed record
  without a definition, and a definition. A redeclared partial
  specialization (`tuple_element<I, tuple<T...>>`, `hash<bitset<N>>`)
  is the same, its definition's own template-head in `lheads`: `u32 n`,
  then per entry `u32 rec, tloc, langle, rangle`, `u32 k`, k parameter
  records (written just before `rec`, claimed by it). An explicit
  specialization's definition has no entry. The reader defines the
  specialization it made for the listed record, as the parser does.
  A specialization a declaration named before it was declared (what
  sema made in between comes first in the source) is WP-1345's
  instead: visited just before its first member, no late record.
- *Variable template specializations* ([temp.inst]/7). An implicit
  specialization of a carried variable template (`e_v<double>`) is a
  `Spec` record of template kind 3, never complete, its converted
  arguments as a class template's (packs flat). The reader declares it
  with `get_var_spec`; the initializer comes with the bodies and is
  instantiated in `image_mark_specs`, a constant one at once. A body's
  `DeclRefExpr` naming it names that record (`dc_ok` accepts template
  kind 3). Still refused ("a variable template specialization"): a
  placeholder type. (A template with partial specializations or
  constraints was refused until WP-1358: the selection is kept in
  `psels`.)
- *Declaration order*. Implicit variable and function template
  specializations at namespace scope are written where the walk meets
  them (`var_spec_at`), so the importer makes them in replay's order
  (a namespace's member list, a template's specialization list, losing
  overload candidates included). One the image cannot name is left to
  its users, as before.
- *Template arguments in type expressions*. A type expression section
  may hold template arguments of kinds 3 and 4 (a carried template, a
  dependent template name; `is_valid_expansion<_Templ, ...>`,
  `tuple:2933`), each naming its record.
- *Explicit instantiation declarations of function templates*
  ([temp.explicit]/11, `extern template ostream& __ostream_insert(...)`).
  The specialization is a `Spec` record of template kind 0 at its place
  in the walk, listed in `fexpl`: `u32 n`, n ascending record numbers.
  The reader sets its kind and point of instantiation (the record's
  location) as `act_on_explicit_function_inst` does. An explicit
  instantiation definition stays refused ("a template specialization:
  an explicit instantiation definition").
- *Explicit instantiation declarations of single members*
  ([temp.explicit]/3, `extern template basic_string<char>::basic_string(
  const basic_string &)`). There is no declaration for the directive:
  the walk writes the `Member` record of every member function so
  instantiated (used or not), its `loc` the directive's, listed in
  `fexpl`. The reader updates the member's `MemberSpecInfo` as
  `act_on_explicit_member_inst` does. A definition stays refused.
- *Explicit specializations of function templates*
  ([temp.expl.spec], `template <> double __do_strtod<double>(...)`).
  The writer makes no record for the specialization sema made by
  deduction; the written declaration is a `Function` record with no
  `prev`, listed in `cspecs` as a template kind 0 entry (no head,
  converted arguments, one per parameter, no pack). Bodies naming the
  specialization name that record. `fsuses` keeps what the unit's uses
  set on the specialization: `u32 n`, then per entry `u32 rec, u8
  bits` (bit 0 `USED`, bit 1 `REFERENCED`, nonzero), ascending, rec a
  listed function entry. The reader finds or makes the specialization
  with `get_function_spec`, marks it explicit, builds the record as its
  redeclaration (`function_spec_redecl`) and, after the bodies, applies
  `fsuses`. Still refused: one declared more than once, one with a pack
  argument, one not at namespace scope ("a template specialization:
  ...").
- *Substituted parameters of uncarried templates*. A
  `SubstTemplateTypeParm` whose template has no record (a variable or
  function template specialization's, an uncarried alias's) with an
  unqualified replacement is written as the replacement, which is how
  it prints (`optional:508`).

Tests: `declared_then_defined_explicit_specializations_are_carried`,
`redeclared_partial_specializations_are_carried`,
`variable_template_specializations_are_carried`,
`template_arguments_in_type_expressions_are_carried`,
`explicit_function_instantiation_declarations_are_carried`,
`explicit_function_specializations_are_carried`,
`explicit_member_instantiation_declarations_are_carried` (kiln_driver
`pch_decls`: census without refusals, image equals replay under
`check`, the program runs, diagnostics after the header as after a
replay); `kind2_declaration_image_wp1352_forms` (modules_bmi);
`late_definitions_round_trip`, `rejects_corrupt_late_definitions`,
`function_specializations_round_trip`,
`rejects_corrupt_function_specializations` (kiln_sema); corpus
`cpp.include/pch-declared-specializations-{run,errors}.cpp` and
`module.interface/image-declared-specializations-run.cpp`.

**Forms carried by WP-1353.** The writer's additions are in
`bmi_image/spec.rs`, `deptypes.rs`, `scope.rs`, `attrs.rs` and the new
`dedup.rs`, `classusing.rs`, `outerspec.rs`, `oodefault.rs` and
`diagif.rs`. One side section, `dedup`, follows `fsuses`.

- *A template named through a qualifier* ([temp.names]/3,
  `typename _OrImpl<...>::template _Result<...>` in libc++'s
  `<__iterator/iterator_traits.h>`, `typename holder::template box<T>`).
  A specialization whose template name is `TemplateName::Qualified` over
  a carried template is a dependent type record (type tag 21 `Dep`) of
  dependent tag 7 `QualTst`: `u8 kw` (0/1, the `template` keyword), the
  qualifier's steps as for a dependent name (`NnsRec`s), and a
  qualified reference to a plain `Tst` record of the same arguments
  (written just before it, quals 0). The validator requires a non-empty
  valid qualifier and that the reference is a `Tst`. The reader builds
  the plain specialization, wraps its template name in a `Qualified`
  name with the importer's qualifier and calls `template_specialization`
  again, which is what the parser makes. A base class written so
  (`: _OrImpl<...>::template _Result<...>`, also under `typename`) is a
  dependent base through it. The census now names the qualifier, the
  dependent qualifier, the template template parameter and the
  using-declaration forms of "a template named through a qualifier or
  parameter" apart; the last three stay refused.
- *A redeclaration of the implicit `std::align_val_t`* ([new.delete]/1,
  [cstddef.syn], libstdc++'s `<new>`). Sema declares `std::align_val_t`
  implicitly with the replaceable allocation functions, in the program's
  own `std` once one is declared (else an implicit one), at the first
  new-expression or `operator new` declaration. A header that uses `new`
  before its own `enum class align_val_t` redeclares that enumeration.
  The writer gives the enumeration no previous record, and a type naming
  the implicit enumeration is an `Implicit` record of kind 10
  (`ALIGN_VAL_T_KIND`; no owner, context, name or previous record, at
  most one per image). The reader finds the entity redeclared the way
  the parser does: it calls `declare_global_new_delete` (a no-op once
  the importer declared them) and takes `global_ops.align_val_t`; the
  header's enumeration record is an ordinary redeclaration of it
  (`image_enum_prev` looks in the importer's global `std`). Before, the
  enumeration's `prev` was the implicit declaration without a record
  ("a redeclaration of an entity outside the image", `new:94`).
- *An argument-less word attribute on an explicit instantiation*
  ([dcl.attr.deprecated], `extern template class
  __attribute__((__deprecated__)) codecvt_byname<...>` in libc++'s
  `<__locale>`). An `attrs.insts` entry may be an `Exprs` attribute with
  no expressions (start and end both 0), as a `Words` one; it carries
  the attribute as written.
- *§6 deduplication keys of a precompiled header* (the reason the
  importer re-declared a header's entities when a module's global module
  fragment included the header again; `module.global.frag/
  gmf-floatn-limits.cpp` under `--pch-diff check`). The producer's
  parser notes each namespace-scope declaration it parses outside a
  replay (not an empty declaration): its first and last tokens, its
  token count and the declarations made. The `dedup` section is `u32 n`,
  then per entry `u32 start, end, ntoks, lo, hi`: the two tokens as
  image locations, the count (nonzero), and the half-open range of the
  records made for the declaration (`lo <= hi <=` the record count;
  `lo == hi`: none), in parse order. An entry whose tokens have no image
  location is dropped. The reader maps each entry to the importer's
  locations and the declarations made for its records (stubs of the
  importer's own declarations, `Implicit` kinds 6 and up, are not part
  of it); the parser records them as replay's parse would have when its
  module state activates, so a later global module fragment's copy is
  skipped by position and the earlier declarations are made visible.
- *Using-declarations of base members in a class* ([namespace.udecl]/3,
  `using _RangeAdaptor<_Drop>::operator();` in libstdc++'s `<ranges>`;
  `bmi_image/classusing.rs`). In a class that is not dependent, a
  `Using` member record (kind as at namespace scope, its qualifier's
  steps, non-empty) and one `Shadow` member record per member it
  introduces, each with its access. A shadow's target is the base
  member's record: an ordinary member (a non-constructor member
  function, a field, variable, typedef, class, enumeration, enumerator,
  template or shadow of the same name) or a member reference record
  (`Member`) of a class template specialization, which the writer
  writes before the shadow (a member template of a specialization may
  now be a `Member` record's pattern). The reader declares them with
  `using_decl` and `using_shadow`. An inheriting declarator (`using
  B::B;`) stays refused ("a using-declaration in a class").
- *An explicit specialization of a member class template defined
  outside its class* ([temp.expl.spec]/2, `template <> struct
  __future_base::_Result<void>` in `<future>`; `outerspec.rs`). Written
  as WP-1345's out-of-class partial specialization: a `Record` record
  whose context is the (closed) class, its lexical context the
  namespace, no previous record; the validator accepts it at namespace
  scope when `cspecs` lists it as an explicit specialization.
- *A special member defaulted outside its class* ([dcl.fct.def.default]/5,
  `inline S::S() = default;`; `oodefault.rs`). A `Function`
  redeclaration of a non-dependent class's special member with the
  defaulted bit and no body. The importer runs `act_on_defaulted` on
  each such declaration once the bodies (and so the in-class
  initializers its definition uses) are built, which defines it as the
  parser's call after the declarator did. A class template's member
  defaulted out of class stays refused ("an out-of-line defaulted
  function").
- *`diagnose_if` rechecks* ([dcl.attr] extension, clang
  `diagnoseArgDependentDiagnoseIfAttrs`; `diagif.rs`). Nothing new is
  written: the importer fills `diagnose_if_recheck` from the bodies it
  built (every call naming a function with argument-dependent
  `diagnose_if` attributes, with no value-dependent argument), so an
  instantiation in the unit diagnoses a shared call as after a replay.

Tests: `qualified_template_names_are_carried`,
`implicit_align_val_t_redeclarations_are_carried`,
`deprecated_explicit_instantiations_are_carried`,
`class_using_declarations_are_carried`,
`out_of_line_defaulted_members_are_carried`,
`out_of_class_member_explicit_specializations_are_carried`,
`diagnose_if_rechecks_follow_the_image`,
`gmf_reinclusion_merges_with_image_declarations` (kiln_driver
`pch_decls`); `kind2_declaration_image_wp1353_forms` (modules_bmi);
`dependent_types_round_trip`, `rejects_corrupt_dependent_types`,
`align_val_t_records`, `dedup_round_trip`, `rejects_corrupt_dedup`
(kiln_sema); corpus `cpp.include/pch-decl-followups-{run,errors}.cpp`
and `module.interface/image-decl-followups-run.cpp`.

**Forms carried by WP-1350 (lambdas, local classes, template forms).**
New tags and kinds come from the end of each enumeration.
- *Lazy tags.* A closure type ([expr.prim.lambda.closure]) and a class
  or enumeration whose context is a function ([class.local], [dcl.enum])
  are *lazy tags*: like a local variable, a lazy tag gets records only
  when something carried names it (its type, a member, its
  lambda-expression, its declaration statement). Then the tag and
  everything declared in it are written at once, in declaration order,
  as a namespace-scope class is: its members and nested classes, a
  generic call operator's template and the invented template parameters
  of its `auto` parameters (sema's context for them is the enclosing
  function; the image's is the closure record), and the implicit members
  sema declared (`Pass.implicit`). A lazy tag's record is a class or enum
  record whose context is the function record. Its overriders, key
  functions and member initializers are patched as a namespace-scope
  class's are, and its declaration locations are merged into the
  location table. While one tag is being written, a tag named by it falls back:
  a tag is never written inside another.
- *Closure records.* A closure type is a class definition with
  `DefinitionBits::LAMBDA` (and `LAMBDA_CAPTURES` when it captures). The
  call operator, the conversion function to pointer to function
  ([expr.prim.lambda.closure]) and the static invoker are ordinary
  member records; the conversion and invoker keep `IMPLICIT` in their
  record flags, which is how the importer tells them apart from written
  members (`closure_thunk` / `closure_invoker`). A capture's member is
  an unnamed field record, exempt from the anonymous-member rules.
- *The lambdas section.* After WP-1353's `dedup` section (itself after
  `fsuses`): a count, then
  per lambda the words `closure`, `call_op` (a generic lambda's: its
  template's pattern; `NONE` for none), `default_loc`, `lbracket`,
  `rbracket`, `number`, `context`; the bytes `default` (0 none, 1 `=`,
  2 `&`) and `flags` (bit 0 a parameter clause was written, bit 1 a
  trailing return type, bit 2 `mutable`, bit 3 the capture initializers
  are set); the captures (count, then per capture the bytes `kind`
  (0 `this`, 1 `*this`, 2 by copy, 3 by reference) and `flags` (bit 0
  implicit, bit 1 an init-capture) and the words `var` (a declaration
  word, `NONE` for `this`), `field`, `loc`, `ellipsis`); the unexpanded
  packs the body names (declaration words); the capture initializers
  (body expressions).
- *Lambda-expressions.* `LambdaExpr` is expression kind 69. Its slot is
  `F::LB`, a lambda record's index, and its operands are the capture
  initializers. The importer builds every closure and lambda record
  before the bodies (`image_lambdas`), sets the initializers with the
  expression, and sets the call operator's body after the function
  bodies (`image_lambda_bodies`). `F::LB` is refused in a type's
  expression section.
- *How closures are numbered.* A closure type's identity is the
  replay's: the image carries the `LambdaMangling` number and context
  sema gave it, verbatim, so the `Ul…E[n]_` mangled names agree on both
  sides ([basic.def.odr]). A context that is a variable recorded
  after the closure (a namespace-scope initializer's lambda, written
  while the variable's type is prescanned) is patched once that
  variable's record exists. A closure without a number (`$_N`) gets its
  name when it is first mangled, as in replay. The importer does not
  restore `ManglingState.numbers`: every carried closure has its number,
  and a unit's own lambdas number in their own contexts.
- *Designated initializers* ([dcl.init.aggr]). `DesignatedInitExpr`
  (template patterns only) is expression kind 70, slot the initializer;
  its designators are cold bit 32768 `COLD_DESIG` (`F::Desig`: a
  count, then each `.name`'s name index and location). A pattern's
  semantic `InitListExpr` carries its written form (`syntactic_list`,
  which instantiation rebuilds from) in the `COLD_REWRITE` slot. An array
  or range designator (an extension) is refused.
- *Vectors.* A vector type of a value-dependent element count
  (`ext_vector_type(N)`, `vector_size(N * sizeof(T))`) is expression
  type tag 3 `DepVector` (element, expression section, `VectorKind`
  byte, canonical flag), like `DepArray`. `__builtin_convertvector` is
  expression kind 71 (slot the operand; its `)` is the end location).
- *Resugared typedefs.* A typedef type naming typedef record `d` with
  an underlying type other than `d`'s own (a member typedef named in a
  member alias template, its underlying type substituted again:
  libc++'s `unique_ptr::pointer` in `_EnableIfMoveConvertible`) is type
  tag 27 `TypedefAs` (the record, the underlying type).
- *A specialization's parameter.* A default argument use on a call of a
  function template specialization names the specialization's parameter,
  which has no record. The declaration word `PARAM | SPECP | spec << 8 |
  i` (`SPECP` = `0x2000_0000`, `spec` a function `Spec` record below
  `2^21`, `i` below 256) names its `i`-th parameter (`Dc::Param` only).
  The importer declares the specializations before their patterns'
  default arguments exist, so once the defaults are set it defers each
  specialization's defaults again as substitution does (`lazy_defaults`,
  [temp.inst]/12). It then instantiates the ones a body uses, as the
  replay's calls did (`image_spec_defaults`).

Still refused in this bucket:
- *A closure or local class named before its context is recorded.* A
  lambda in a requires-expression (its context is the requires body,
  which has no record) and a local class named by its function's deduced
  return type (named while the function's type is prescanned, before
  the function's record exists).
- *An expression in a type naming a parameter or local declaration.* A
  requires-expression in a type (`std_function.h`'s `__function_guide_t`,
  `requires (_Fn& __f) { ... }` as a template argument). A type's
  expression sections are built while the records are imported, before
  the requires-expression bodies, which declare its parameters.
- A lambda-expression or local class in a type's expression section
  (`decltype([]{})`), and a local class used before its definition.
- An array or range designator, and a member function of a local class
  in an imported module's inline function (the replay has the same
  gap: neither side emits it; xfail
  `module.interface/local-class-members-link-run.cpp`).

Two gaps outside the bucket, first reached once libstdc++'s `<compare>`
was carried, are closed with it: a dependent base named through a
qualifier (`: std::X<T, U>`, an `ElabQ` record over a dependent `Tst`)
passes the reader's base check (the writer already wrote it, and the
reader rejected its own writer's output as "an internal image
inconsistency"). The comparison category check (`cmp_category_type`)
folds a category value from its initializer when the image left it
unevaluated (an aggregate value is not carried). Before that fix, every
built-in `<=>` in a consumer of the image failed with "the type does not
have the expected form".

Tests: `crates/kiln_driver/tests/pch_decls.rs`, `modules_bmi.rs`
(`kind2_declaration_image_lambdas`,
`kind2_declaration_image_designators_and_spec_defaults`) and the corpus
pairs `cpp.include/pch-lambda-forms-{run,errors}.cpp`,
`cpp.include/pch-wp1350-forms-{run,errors}.cpp`,
`module.interface/image-lambda-forms-run.cpp`,
`module.interface/image-wp1350-forms-run.cpp`.
**Forms carried by WP-1360.** The writer's additions are in the new
`bmi_image/outside.rs` and in `selfbase.rs`, `body.rs`, `requires.rs`
and `exprs.rs`. No record tag, type tag or cold-annotation number is
new. The class definition bits gain bit 62.

- *The outside-image reasons, by kind.* "a body referring to a
  declaration outside the image" and "a type declared outside the
  image" were one reason each. The census now names the kind of
  declaration in parentheses. Each kind below says how a reader would
  find the entity, which is the shape a later wave carries it in:
  - *(a compiler builtin)*: a function sema declares lazily on first
    use (`__builtin_*`). The reader would find it by name, as the
    parser's lookup declares it. This would be an `Implicit` record
    whose kind names the builtin (the `GLOBAL_OP_KIND` shape, with the
    builtin id in place of the operator).
  - *(an implicit member)*: an implicitly declared special member of a
    class. It is carried below when the class is closed. Otherwise it
    stays refused.
  - *(an implicitly declared function)*, *(a predefined entity)* and
    the type kind *(a predefined type)*: entities every unit declares
    (the implicit allocation functions not yet seen, `__func__`, the
    target's predefined typedefs such as the SVE `__SVBool_t` types in
    `<bits/math-vector.h>`). The reader would find each in its own
    unit, as for `std::align_val_t` (kind 10): an `Implicit` record
    whose kind names the entity, which the importer looks up or
    declares.
  - *(a declaration of another unit)*: a declaration owned by an
    imported module or header unit. The reader would find it through
    that unit's own image, which is the import section's job (S6).
  - *(a refused declaration)*: a cascade. The census also prefixes it
    with "cascade:". It goes away with the first refusal.
  - *(a later member of its class)* and *(a declaration not
    recorded)*: a declaration of this unit that the walk records after
    the body that names it, or never. These are the order-dependent
    forms. The carried ones are below.
  - *(a record of another kind)*, *(a base of a class outside the
    image)*, *(a local declaration of another function)*, *(a parameter
    of another requires-expression)* and *(an explicit specialization)*:
    the uses that checked a declaration's record and found none of the
    kind they need. Before, these used the same words.
  - Type kinds: *(a closure type)* and *(a local class or enumeration)*
    are WP-1350's forms. *(a class named before its record)* is a class
    of this unit named in a type before the walk reaches its record,
    such as a member class of a specialization (`stor<8, 8>::type`).
- *A `noexcept` operand naming a later member* ([class.mem.general]/7:
  the operand is a complete-class context; `noexcept(noexcept(*cur))`
  before `T *cur;`). Writing the type's operand gives "an expression in
  a type naming a later class member". The function then goes through
  the `late` entry, as one whose operand names its parameters does,
  with the same layout. The importer marks each `late` entry's function
  as the parser marks a declarator it saved for the complete-class
  context: its type carries `ExcSpec::Deferred(f)`, listed in
  `unparsed_exc`. A specialization the importer instantiates before
  the bodies are built (a header's `counted<long>`) then defers its
  own specification ([temp.inst]/14) instead of copying a type with
  none. `image_late_nx` sets the specification with
  `set_member_exception_spec`.
- *A class named in its own base clause* ([class.pre]/2: the class is
  declared at its *class-head-name*; `struct derived : base<derived>`,
  the CRTP). A class that is not a template, at its first declaration,
  whose base-specifiers name it, has bit 62 (`BASES_SELF`) in its
  `RecDef` and no bases in the record. The writer pushes the record,
  then writes the bases. Their specialization records (`Spec`, and
  `Member` records under them) follow the class record. It then patches
  the base list into the record. A base type names the class through a
  pointer, reference or array type, or a template argument, to depth
  16. The validator checks the bases as if they were record `j - 1`'s,
  where `j` is the first record after `i` that is neither `Spec` nor
  `Member`. Bits 62 and 63 never appear together. The reader gives the
  class its bases right after it builds record `j - 1`, through the
  same path as `BASES_AFTER`.
- *An implicit special member named in a later type*
  ([class.default.ctor]/1, [class.copy.ctor]/6; `noexcept(cmp()(a,
  b))`, `-> decltype(ord(cmp()(a, b)))`). The walk writes implicit
  members after every declaration. A type section that names one whose
  class record is closed and not dependent writes its `Implicit` record
  (kind 0 to 5, the special kind, context the class) where it is first
  named. The end-of-walk pass skips it. The validator accepts an
  `Implicit` record whose context is a closed class, as well as a
  complete specialization at the top level.
- *A function template with a deduced return type declared before its
  definition and used in between* ([dcl.spec.auto.general]/13,
  `template <class T> auto f(T); ... template <class T> auto f(T t) {
  ... }`). `set_deduced_return` gives every redeclaration the
  definition's type, so the first declaration's type names the
  definition's template parameters. While it writes such a
  declaration's type (one listed in `declared_returns`), the writer maps
  a template parameter of another redeclaration of the same template to
  its own parameter of the same depth, index, pack-ness and kind
  ([temp.over.link]/6).
- *A data member in an unevaluated operand of a member's type*
  ([expr.prim.id.general]/4; `struct holder { void *p; raw<sizeof(p)>
  buf; }`). A declaration reference whose target is a `Field` record
  and whose context is a class is accepted where a value is named.
- *A requires-expression of a concept whose satisfaction was checked*
  (a `static_assert(testable<int>)` in the header). A substituted
  requires-expression keeps its pattern's body with new parameters, so
  the writer's map from body to requires record keeps the first
  record, the pattern's. Before, the substituted record's parameters
  were named from the pattern's body: "a parameter of another
  requires-expression".

- *A member function template of a specialization in a dependent
  call* ([temp.res.general]/1, [temp.dep.candidate];
  `std::_Destroy_aux<false>::__destroy(__first, __last)` in
  `bits/stl_construct.h`). The callee's lookup result is a `Member`
  record of the specialization whose pattern is the member template, a
  `Template` record of kind 0. The validator now accepts it where a
  lookup finds a declaration (`Dc::Lookup`, `Dc::Found`). Before, it
  was "(a record of another kind)".

Tests: `noexcept_naming_a_later_member_is_carried`,
`member_templates_of_specializations_in_dependent_calls_are_carried`,
`self_naming_bases_are_carried`,
`implicit_member_named_in_a_type_is_carried`,
`checked_concepts_sizeof_members_and_auto_redecls_are_carried`
(kiln_driver `pch_decls`); `kind2_declaration_image_wp1360_forms`
(modules_bmi); corpus `cpp.include/pch-outside-refs-{run,errors}.cpp`
and `module.interface/image-outside-refs-{run,errors}.cpp`.

**Forms carried by WP-1358 (specializations).** The writer's additions
are `bmi_image/psel.rs`, `mtspec.rs` and `named.rs`, plus
`constrspec.rs`, `spec.rs`, `classtempl.rs`, `latedef.rs` and
`body.rs`. One side section follows `lambdas` (WP-1350): `psels`.

- *Partial specialization selections* ([temp.spec.partial.match]/2).
  The importer completes a specialization while it reads the records,
  before the constraints are patched in, so it cannot repeat matching
  against a constrained partial specialization. `psels` keeps the
  unit's result: `u32 n`, then per entry `u32 spec, u32 partial` (the
  partial specialization's record, or `NONE` for the primary
  template), ascending by `spec`. An entry names a complete `Spec` of a
  class template (template kind 1) whose template has a constrained
  partial specialization, or any `Spec` of a variable template (kind
  3) with partial specializations. Its partial specialization is a
  partial specialization of that template recorded before it. The
  reader notes the entry before it completes or declares the
  specialization, and `match_partial_args` then deduces against that
  partial specialization only (the arguments matched it, so deduction
  succeeds) or takes the primary template. A class whose completion
  completes others (bases, data member classes at any depth) that need
  a selection has their references written first (`pre_record_sels`),
  so they are completed first, each with its selection. Variable
  templates with partial specializations or constraints are carried
  this way (`numbers:139`'s `e_v<double>`). A constrained variable
  template's constraints held for the unit's specialization
  ([temp.names]/7).
- *Constrained members of completed specializations* ([temp.mem.func]/1,
  [over.over]). The completion copies a member function's trailing
  requires-clause (`member_constraints`), which is not in yet at record
  time. After `image_constraints`, the reader gives every function
  member of a complete class specialization that lacks one the
  trailing requires-clause of the member it was instantiated from,
  with the specialization's arguments. A member template gets what the
  completion copies from its pattern's head (`Substituter::param_list`):
  its parameters' type-constraints and its requires-clause,
  unsubstituted, and its `member_constraints` entry. Before WP-1358 a
  member template of a carried specialization had none, so a
  consumer's specializations of it mangled without them (`Tk`/`Q`).
  Still refused ("a specialization of a constrained template:
  completed, with a constrained member"): of a constrained class
  template, a member with a trailing requires-clause in a nested
  class, or one whose name another member shares (overloads differing
  only in their constraints, [basic.scope.scope]/4), a special
  member's (its constraints decide which is eligible, [special]/6),
  and any constrained member template (WP-1344). Of any class
  template, a constrained member function template with the name and
  function type of another member function template of its class
  ([temp.over.link]/7: the completion, without the constraints, would
  declare it as that one; such a specialization was wrongly carried
  before). A specialization of a constrained member template that the
  header itself makes is refused as "a specialization of a constrained
  member template of a class specialization": it is made at record
  time, before the constraints, and would mangle without them.
- *Specializations of partial specializations*. A partial
  specialization declared before it is defined (`template <class T>
  struct traits<T *>;`, `<bits/iterator_concepts.h>`), or defined just
  before the declaration naming its specialization: `spec_rec` takes
  the definition's record as `from`. `one` closes the open classes a
  declaration is no member of before its specialization references
  are written (`close_before`), so the importer completes the
  specialization after the partial specialization's definition ends.
- *Specializations named while their user is recorded*. What `prescan`
  does not reach (`ptag<1>()` in a trailing return type's `decltype`,
  whose implicit default constructor the record adds) is dropped with
  the record's numbering, written first in the order the record named
  it, and the record written again (`record_after_named`). A record
  that still names a new reference, or names anything but a `Spec`,
  `Member` or `Implicit` record, is refused ("a specialization named
  while its user is recorded"). `late_split` ignores implicit special
  members, which sema declares when they are first needed.
- *Member function template specializations* ([temp.mem]/1,
  `chrono::hours(__h)`). The template of a specialization of a member
  template of a class specialization is that class's `Member` record
  (S4b) whose pattern is the member template's `Template` record (kind
  0). The specialization is a `Spec` with that template. The reader
  finds the member template in its class specialization and makes the
  specialization with the same arguments. `mt_spec_at` writes these
  where the walk meets them (losing overload candidates included, as
  `var_spec_at`), and writes the class specialization first when only a
  body named it before (`dur<long>(h)`). A substituted type parameter
  whose template is such a member template names that `Member` record.
  A body's `DeclRefExpr` may name such a `Member` record.
- *Dependent expressions as partial specialization arguments*
  (`segmented_iterator.h:72`, `has<T, sizeof(T) * 0>`). A non-type
  argument of a partial specialization that is a dependent expression
  other than a parameter reference is a type expression argument
  (`ArgRec::Expr`). One without a profile is refused ("a partial
  specialization argument the image does not carry").
- *Value and pack template arguments in bodies* (`chrono`'s literal
  operator templates). A body's explicit template argument token is 3
  words per argument. Kind 5 is a value `[5, type, constant index]`.
  Kind 6 is a pack `[6, n, 0]`, followed by its n elements (no nested
  pack).

Tests: `constrained_partial_specialization_selections_are_carried`,
`specializations_named_while_recorded_are_carried`,
`member_template_specializations_are_carried`,
`member_template_specializations_of_body_named_classes_are_carried`,
`specializations_of_partial_specializations_are_carried`,
`variable_template_partial_selections_are_carried`,
`partial_specialization_argument_expressions_are_carried`,
`value_and_pack_template_arguments_in_bodies_are_carried`,
`constrained_members_of_completed_specializations` (kiln_driver
`pch_decls`). `record_time_completion_concept_id_matches_replay` and
`record_time_completion_variable_template_matches_replay` are ignored
repros of pre-existing bugs (a record-time completion evaluates a
concept-id or a variable template before its constraint or initializer
is in). `kind2_declaration_image_fallback` (modules_bmi);
`partial_selections_round_trip` (kiln_sema). Corpus:
`cpp.include/pch-spec-forms-{run,errors}.cpp` and
`module.interface/image-spec-forms-{run,errors}.cpp`.

**Static assertions in class templates (WP-1307).** A
`static_assert` in a class template, a member class of one or a
partial specialization (dependent: `DEPENDENT_CTX`) is a member record
(kind 26) in its class's member order; its condition and message are
body expressions walked like a default member initializer. A
non-dependent one makes no record (it was checked when the unit was
compiled), and one in a function template's body is a local
declaration ("a static assertion in a template" until local
declarations of templates are carried). The validator requires the
record in a class definition, dependent, in a dependent class, unnamed.
The reader builds the declaration with a placeholder `true` condition
and sets the expressions after the bodies: a specialization the unit
completed, and so already checked, may be instantiated while the
records are built. Tests: `image-static-assert-run.cpp`,
`kind2_declaration_image_static_assert_in_template` (the importer's
failing instantiations report what a replay reports).

**Specializations named before the definition (WP-1307).** A Spec
reference the unit completed is completed by the reader at its record
only when the template's pattern is defined by then; one named before
the definition (`template <class T> struct X; using XI = X<int>;`) is
completed at the first member reference to it, or after the bodies, as
a replay completes it at its first use after the definition. A member
reference's pattern may be a member of any declaration of the
template's chain. Test: `module.unit/alias-forward-template.cpp`.

Still falling back: a partial specialization declared before it is defined
("a redeclared partial specialization"), partial specializations in a
class or of a member template ("a member template or a template in a
template"), constrained ones ("a constrained template", WP-1284),
arguments that are dependent expressions ("a partial specialization
argument the image does not carry"), and explicit specializations
whose template has a non-type parameter with a default argument ("an
explicit specialization using a non-type default argument"). Member
templates, templates in templates and redeclared templates are carried
since WP-1292 (below).

**Variable template specializations (WP-1307).** A partial or
explicit specialization of a carried variable template is a `Var`
record listed in the same specialization section, with a template
record of kind 3: a partial one with its head and converted
arguments, an explicit one with its printed arguments (sema keeps no
written ones for a variable; the converted less the trailing defaults
convert and print the same). The validator accepts a `Var` record for
a kind-3 template where it accepts a `Record` for kind 1. The reader
builds a partial one with `var_partial_spec` and an explicit one with
`find_or_create_var_spec`, marked explicit and declared in its lexical
scope, as `act_on_var_spec_declarator` does; its initializer follows
as any variable's. Uses of a variable template specialization in a
non-dependent body (`kind<char **>`) still fall back ("a template
argument list"). Tests: `image-var-spec-run.cpp`,
`image-var-spec-errors.cpp`.

*Static assertions in templates.* A `static_assert` in a class template
(or a partial specialization) is checked again by each instantiation
([temp.inst]/3). Before WP-1283 the image silently left it out, and an
importer accepted ill-formed instantiations; WP-1283 refused it ("a
static assertion in a template"), and since WP-1307 it is carried
("Static assertions in class templates" above). A non-dependent
`static_assert` was checked when the module was compiled and is still
not carried.

*Declarator locations.* Sema keeps, beside each declaration of
dependent type, where its first type-specifier, its outermost
`&`/`&&` and its innermost `*`/`[` were written (`type_spec_locs`,
`ref_locs`, `inner_chunk_locs`); an instantiation that forms an invalid
type is diagnosed there (`T arr[2]` with `T = void`, at the `[`). The
`declocs` section (`bmi_image/declocs.rs`, after the specializations)
lists them as (record, kind, location) triples, ascending, for every
carried record, so an imported pattern's instantiation reports what
replay reports. Before WP-1283 they were missing and such errors
pointed at the declaration's type-specifier. A function parameter's
entries are not carried yet (parameters are not records).

**Member templates (WP-1292).** `bmi_image/membertempl.rs` carries
member function and class templates ([temp.mem]) of classes and class
templates, at any depth, and their out-of-line definitions.

- *Depth.* A template-head declared in a class has the depth of the
  number of class templates and partial specializations enclosing it;
  its parameter records carry that depth. The validator computes the
  depth from the context chain and checks it.
- *Out-of-line definitions.* `template <class T> template <class U> T
  S<T>::mul(U)` and `template <class T> template <class V> struct
  S<T>::Out { ... }` are preceded by one parameter record per parameter
  of each enclosing template-head, outermost first, then by the
  definition's own head's parameters, all in its lexical context. The
  definition claims the enclosing heads' parameters, its template
  record claims its own. The pattern's context is the class, its
  lexical context the namespace, and its `prev` the in-class pattern.
- *Claims.* Every dependent record belongs to exactly one template: a
  member of a class template's pattern is claimed by the class, except
  a member template's pattern and parameters (its template record's)
  and an out-of-line definition's enclosing heads (the definition's).
- *Redeclared templates.* Template declarations have no redeclaration
  link of their own. The writer's `prev` of a template record is the
  template described by its pattern's previous declaration
  (`described_template(prev(pattern))`), the declaration sema replaced
  in lookup, and the reader passes the same `prev` to `template_decl`.
- *Inherited defaults* ([temp.param]/12). A redeclaration's parameters
  carry only their own defaults. After the bodies are built, the reader
  runs, for each redeclared template, what `finish_template_pattern`
  runs: `merge_template_defaults` and `record_inherited_defaults`
  against the earlier declaration's head, so `D<>` and `scaled(4)`
  resolve as after a replay.

Tests: `image-member-template-run.cpp`, `image-member-template-errors.cpp`
and `kind2_declaration_image_member_templates` (an error inside an
imported member template's pattern is reported as after a replay).

**Polymorphic class templates and pack bases (WP-1292).** A class
template's pattern with virtual functions, overriders, `final`, pure
virtual functions, virtual destructors and virtual bases is carried as a non-template
class's is ("Classes (S3)"): the pattern's `FnSpec` bits and definition
bits are the pattern's, and an instantiation computes its overridden
functions, key function and vtable as after a replay (the pattern's own
overridden sets are empty, as sema leaves them). A base-specifier's
flag byte has bit 0 `virtual` and bit 1 pack expansion
([temp.variadic]/5.2): a pack base `Ts...` carries its pattern `Ts`
as its type, only in a dependent class, where the validator checks it
as any dependent base. Test: `image-poly-template-run.cpp`. A dependent
base is a type parameter or a dependent specialization, also named
through a dependent alias template specialization the validator looks
through (`bc<!is_ptr<T>::value>`, WP-1300; test
`cpp.include/pch-expr-alias-base-run.cpp`).

**Friends (WP-1292).** `bmi_image/friend.rs` carries the friend
declarations ([class.friend]) of classes that are not templates:

- the befriended function, or the class an elaborated `friend struct
  H;` introduces, is an ordinary record before its `Friend` record,
  with context its namespace, lexical context the class, and its
  `FriendKind` in bits 2 and 3 of the owner byte (1 declared, 2
  undeclared). The reader sets `misc.friend()` from them, so its
  builder enters a hidden friend (`Undeclared`) with `ADL_ONLY` as
  sema's does ([namespace.memdef]/3, [basic.lookup.argdep]/4); a friend
  redeclaring an earlier function is `Declared` and its `prev` is that
  function;
- the `Friend` record (payload code 25) is a member of the class with
  no name: either `target`, an earlier befriended function record
  lexically in the class, or a type reference and `owned`, the
  befriended undefined class record or none (`friend class Other;`
  naming an existing class);
- the class definition's friend list, written after its bases, is the
  record indices of the first declarations sema listed at `}`
  (spec/sema-class.md §2.4); the reader sets it before
  `complete_record_definition`. A list entry outside the image is "a
  friend outside the image".

The validator checks that a `Friend` record is in a defined class that
is not dependent and names an earlier befriended record of that class,
that a record with friend bits is a function or undefined class in a
namespace whose lexical context is the open class, and that friend
list entries are earlier function or class records without adjacent
duplicates.

**Friend templates and friends of class templates (WP-1307).** A
friend function template or class template (`template <class U>
friend struct It;`, the `__wrap_iter` shape, where the friend is the
template's first declaration) is a Template record with friend bits
whose head and pattern carry the class as lexical context; its
`Friend` record is dependent (`DEP`), as sema marks it, even in a
non-template class. A friend in a class template (a hidden friend
defined in the class, or a declaration of a namespace function or
class) is carried when its befriended record's lexical class is a
carried dependent class: the befriended record is dependent, owned by
that class for the validator's dependent-record claims (its head
depth is the class's level), and a friend function's body is a pattern
body. The validator accepts a `Friend` record in any class
definition, dependent exactly when its class is or it names a
template, and Template records of kind 0 or 1 in friend lists. The
reader skips inherited-default recording for a dependent friend class
template as `finish_template_pattern` does. Still falling back:
befriended member functions ("a friend member function"), friend
specializations and friends that are members of class templates ("a
friend specialization or member of a class template"), friends
declared by a qualified name in a class template ("a friend declared
by a qualified name"), and other friends of class templates ("a friend
declaration in a class template"). Tests: `image-friend-run.cpp`,
`image-friend-errors.cpp`, `image-friend-template-run.cpp`,
`image-friend-template-errors.cpp`,
`kind2_declaration_image_polymorphic_templates_friends_and_implicit_members`,
`kind2_declaration_image_friend_templates_errors`; an xfail repro of a
pre-existing attachment check, `module.unit/friend-template-instantiated-attachment.cpp`.

**Context reasons (WP-1292).** A record whose semantic context is not
in the image is "a member of a class outside the image" when the
context is a class (a cascade of the class's own reason) and "a
declaration in an unsupported context" otherwise.

**Imports, partitions and using-declarations (S6, WP-1068).** S6
lets three new shapes produce an image: a unit with imports, a unit
that re-exports (`export import`), and a module partition. It also
carries the using family in the purview.

*Imports and re-exports: a replayed preamble.* An image does not
encode what a unit imports. The importer replays the unit's
*preamble* instead: the main file from byte 0 up to the end of the
unit's last top-level module or import directive. That covers the
module declaration and every `import`, `export import` and header-unit
import, with the replay's own preprocessor and parser. Each import
therefore runs through the importer's ordinary path: the module file
is found from the capsule's recorded imports (§11.2), and an image or
a replay of that module is used as for any other import. The import
declarations are made by sema as in a replay. As a result, the
following are those of the replay by construction:

- visibility ([module.import]/2, an `export import` making the
  imported unit's exports visible to importers of this one, §5);
- reachability ([module.reach], the interface dependency set);
- entity identity and cross-unit merging (§6, WP-460);
- the `import` tracking that `-fmodule-file-deps` and codegen read.

When the replay reaches the preamble's end, the preprocessor stops
lexing the main file. The parser then builds the records after it from
the image, with the unit (and its module directive) already entered.

The image header carries a `preamble` word after `name_loc`: the
main-file offset just past the newline ending the last module or
import directive. It is `u32::MAX` when the unit imports nothing. For
that case the S1–S5 path is unchanged: nothing is replayed, and the
reader applies the module directive itself. The writer takes the
offset from the preprocessor, which records where each module and
import directive of the main unit's own main file ends (include
depth 0, `ModuleDirective::end`). A directive in an included file or
in a macro expansion leaves no usable offset, and such a unit falls
back ("an import outside the module's main file, or code after its
`;`"). So does a directive followed on its line by anything but blanks
and a `//` comment (`import M; int x;`, which since WP-1198 declares
`x`): the preamble must end at a line start. Every record of an
image with a preamble must lie after it ("a declaration before the
module's last import").

The writer skips every declaration owned by another unit: imported
ones, and those of the units the preamble's imports replayed. A record
that refers to one of them falls back, with the existing reasons
(each pinned by `kind2_declaration_image_imports`):

- a body naming an imported declaration ("a body referring to a
  declaration outside the image");
- a qualifier naming an imported namespace ("a qualifier naming a
  namespace outside the image"), a type naming an imported class ("a
  type declared outside the image"), an alias or directive naming an
  imported namespace ("a using-directive or alias naming a namespace
  outside the image");
- a namespace reopened from an import, and an implicit member or
  instantiation of an imported class or template ("a declaration in an
  unsupported context").

S6 thus carries the declarations a unit adds on top of its imports, as
long as they do not refer to them. Referring to imported entities
needs an external-reference record (a module key and a source
position, like the AST capsule's position-keyed resolver,
`bmi_ast::value`), and that is a follow-up. A unit made only of
imports and re-exports (an umbrella `export module M; export import
A; export import :P;`) has an empty record list and is fully carried.

The name gate (Part 2a, "ODR merging") is checked twice. The first
check runs before the preamble is replayed. The second runs at its
end, because the preamble's imports may have made a gated name seen.
If the second check fails, or the stop lies inside an open `#if`, the
preprocessor does not stop. It goes on lexing the rest of the main
file and the unit replays, with the reason logged ("its image declares
'X', already seen", "its preamble ends in a conditional"). A preamble
past the end of the recorded main file, or off a character boundary,
replays from the start ("its image has a bad preamble").

Mechanics. `try_image_unit` enters the unit with the ordinary
`replay_capsule_unit` and then gives the unit's main-file frame a
lexer over bytes `0..preamble` and an `ImageStop`. At that lexer's end
of file, `image_stop_reached` decides: on success the frame pops as at
any end of file, and `Marker::UnitEnd` (and the parser's
`Event::UnitEnd`) carries the image, which the parser hands to
`Sema::load_module_image` (`after_preamble = true`) before
`act_on_unit_end`; otherwise the frame's lexer is replaced by one over
`preamble..len` and lexing goes on. The reader checks that
`after_preamble` matches the image (a preamble word iff a preamble
mode load) and that the unit sema entered has the image's key; either
mismatch is a corrupt file ("a preamble that does not match the
import", "a preamble that does not enter the image's unit"). The
writer's offset is `ModuleDirective::end`, which the parser passes to
`Sema::module_note_directive_end` after each module or import
directive of the main unit; a `None` there (an include or a macro
expansion) makes the unit fall back.

*Partitions.* An interface partition (`export module M:P;`) and an
implementation partition (`module M:P;`) produce images like a
primary interface. The image's `module` string is then the unit's key
(`M:P`). The partition's own module directive is part of its preamble,
so a partition always takes the preamble path, even without imports.
After the stop, the reader checks that the unit sema entered has the
image's key, and reports a mismatch as a corrupt file. The kind and
the partition name come from the replayed directive, so a primary that
imports its partitions (`export import :P;`, `import :Q;`) gets
module linkage across the partitions as the replay does. Every record
is owned by the partition's unit with its purview and export bits
(§5). A module implementation unit (`module M;`) and a private module
fragment still fall back ("a unit that is not a module interface or
partition", "a private module fragment").

*Using-declarations, using-directives, using-enum and namespace
aliases.* These are carried at namespace scope, including inside
`export` and `export namespace`. Each is a record whose owner byte
holds its purview and export bits. An importer's lookup finds an
exported shadow exactly as in the replay, and a non-exported one stays
invisible ([module.interface]/2, [basic.lookup]/4). There are five
kinds:

- kind 20, *using-declaration*: a named record whose payload is the
  qualifier, as steps (`::`, a namespace record of the image, a class
  type). It is not in the lookup table: the declaration itself is
  found through its shadows.
- kind 21, *using-shadow*: the payload is the target, an earlier
  record of a declaration (a function, variable, class, enumeration,
  enumerator, typedef, alias, template or shadow). Its name is the
  target's. Its `prev` is the earlier shadow of the same target in the
  same scope that it replaces in lookup (a repeated using-declarator).
  The reader rebuilds each one with `AstBuilder::using_shadow`, so the
  shadow set of a using-declarator is the records that follow it.
- kind 22, *using-directive*: an unnamed record whose payload is the
  nominated namespace's first record. The reader adds the directive to
  the lookup tables of its context's namespace (or the translation
  unit) as `using_directive` does. As in the replay, an imported
  directive is then seen by later lookups of that namespace.
- kind 23, *namespace alias*: a named record whose payload is the
  target, a namespace or alias record. Its `prev` is the earlier alias
  of the same namespace that it redeclares.
- kind 24, *using-enum*: a named (the enumeration's name) unlisted
  record whose payload is the complete enumeration's definition record.
  Its enumerators' shadows follow it.

Qualifier step 3 (`3 alias:u32`) names a namespace alias record. It
is used in a type's or body's nested-name-specifier and in a
using-declaration. The importer rebuilds it as `Nns::NamespaceAlias`
over the alias's namespace.

The importer rebuilds the exported `using std::X;` shape of `std.cppm`
from these records once the target is in the image or reachable
through an external reference. Until S5 and the external-reference
follow-up land, a using-declaration whose target lies outside the
image falls back ("a using-declaration naming a declaration outside
the image"), and so does a directive or alias naming a namespace
outside it ("a using-directive or alias naming a namespace outside the
image").

The following still fall back:

- using-declarations in a class: member shadows, access, hiding, and
  inheriting constructors ("a using-declaration in a class");
- dependent and pack using-declarations (`UnresolvedUsing*`,
  `UsingPack`);
- block-scope using-declarations and directives in bodies.

*S6, measured* (WP-1068; harness profile, aarch64, `/usr/bin/time -v`,
median of 5 interleaved). An ad hoc project: two interface partitions
and an implementation partition of 400 classes, 400 `constexpr`
functions and 400 aliases each (about 2400 records), a primary of 1600
records that re-exports the first two and imports the third, a module
that re-exports the primary, and a consumer that uses all of them in a
`static_assert` and `main`.

- Import, `-fsyntax-only`: 0.11 s and 20.3 MiB replayed, 0.04 s and
  15.0 MiB from the images (clang 19: 0.01 s, 85.5 MiB). `-c`: 0.12 s
  to 0.05 s, peak RSS unchanged at 60.0 MiB (LLVM's).
- Precompiling the primary and the re-exporting module, whose imports
  now come from images: 0.12 s to 0.07 s and 0.10 s to 0.04 s, peak
  RSS 19.8 to 15.4 MiB and 20.3 to 14.3 MiB. A partition's own
  precompile, which imports nothing, grows from 0.04 s and 13.9 MiB to
  0.05 to 0.07 s and 16.6 MiB (writing about 2400 records), and its
  module file from 78 KiB to 461 KiB (about 160 B per record).
- On/off equivalence: identical LLVM IR and identical visibility errors
  (`kind2_declaration_image_imports`, `image-part-run.cpp`,
  `image-part-hidden.cpp`).

**Locations (S7, WP-1070).** Before S7 every carried location had to be
in the unit's main file, so a purview that #included a header, or any
declaration a macro produced, replayed. S7 carries the locations of the
module's source manager (spec/source.md) that the image uses, in an
image location space of its own:

- **File table.** One `file` per buffer the image's locations are in:
  its spelled name (as diagnostics print it), its absolute path when the
  module file was built (empty for an in-memory buffer), the decoded
  text's length, its content hash (FNV-1a 64, `kiln_source::hash`, the
  hash module files share) and whether it was entered as a system
  header. An in-memory buffer (`<scratch space>`, which token pasting
  and `_Pragma` write) carries its text inline. Since WP-1310 so do
  the predefined buffers (`<built-in>`, `<command line>`: a predefined
  macro such as `__CHAR_BIT__` expanded in a template), with their line
  markers and `#line` directives as `(offset, presumed line, presumed
  name or none)` triples (file flag 4), which the reader adds to the
  buffer again so a note in it prints the same presumed position; a
  predefined buffer is entered at its own range, never from a
  precompiled header's root. A file on disk with `#line` directives
  still falls back. Entry 0 is the unit's main file.
- **Location records.** One `sloc` per inclusion range or macro
  expansion entry of the module's source manager that a carried
  location is in, or that such a range or entry names: an inclusion
  (`file`, the image location of the `#include` that entered it or
  `u32::MAX` for the buffer's own range, and its file-kind marks, so
  `# 1 "x" 3` and `#pragma GCC system_header` survive), the expansion of
  a macro's body (spelling, length, the invocation's range, the macro
  name's range) or a macro argument's (spelling, length, the
  parameter's range). Record `k` covers image locations `[base_k,
  base_k + len_k]` with `base_0 = 0` and `base_{k+1} = base_k + len_k +
  1`, where `len_k` is the file's length or the expansion's; a record
  names only earlier records (the source manager allocates every
  dependency before what names it), and expansion records are made once
  per entry, so every declaration in one expansion shares them. Record
  0 is the main file's own range: a main-file location is its offset,
  as before S7.
- **Writing.** A location is carried by carrying its range or entry,
  dependencies first (an explicit stack, no recursion). It falls back
  with the reasons listed under "Bodies" (a predefined buffer, `#line`,
  invalid UTF-8) or when the image's space (below the reserved words)
  is used up.
- **Reading.** The reader checks the records against each other and
  the file table (every record names earlier records, a spelled range
  stays in one record, record 0 is the main file's range, an in-memory
  text matches its hash), then recreates them in order with
  `add_inclusion` and `add_expansion`, so an imported declaration's
  location is an ordinary location of the importer: its caret, its
  include stack ("In file included from m.cppm:3:") and its macro notes
  ("expanded from macro") are the replay's, byte for byte
  (`kind2_image_header_locations`). The main file is the module file's
  recorded copy, as for replay. Every other file is added with
  `add_lazy_file`: it is stat'ed and identified but not read, so a file
  the importer later `#include`s is the same buffer; its text is read
  only when a diagnostic needs a line or a column.
- **Changed files.** A header whose size differs from its record, or
  that is missing, makes the unit replay ("'./h.h' changed since the
  module file was built", "'./h.h' is missing"); replay uses the module
  file's recorded copies, so the carets are the replay's. A file
  already loaded with other text replays the same way. A file of the
  recorded size whose text then fails the hash uses the module file's
  recorded copy (`-v`: "note: './h.h' changed since its module file was
  built; its recorded text is shown"), and failing that prints no line
  or column at all ("its text is not available"). Either way no
  diagnostic points at a wrong position (`kind2_image_edited_header`).

**Attributes (WP-1278).** Every attribute node of a carried declaration
is one record of the `attrs` section (`bmi_image/attrs.rs`): the node's
kind, syntax, flags and location, its two argument words, and the
written scope and name sema keeps for diagnostics. One table, keyed by
kind, says how a kind's words travel, and it is the whole format:

- plain words with a bound per word (flag-only kinds hold `0, 0`;
  `visibility` and `type_visibility` 0 to 2; `format` and
  `no_sanitize` any);
- a `Symbol` (`abi_tag`), written as text;
- expressions (`deprecated`, `nodiscard`, `unavailable`, `alloc_size`,
  `alloc_align`): expression records of the bodies section, built after
  the bodies and attached then;
- a builtin id (`BuiltinAttr`), written with its spelling, which must
  name the same id in the importer's table;
- `nonnull`'s parameter indices: a run of `size_t` integer literals
  (value and location), empty for every parameter;
- an asm label, written as its text. A written label (flag 2 clear)
  also names the first declaration's symbol, as the parser
  records it; an inherited one (flag 2) must match that symbol. The
  label may be written on a later declaration than the first (glibc's
  `__REDIRECT` redeclares a prototype with one, WP-1310): the first
  declaration's symbol is carried by that declaration's record, which
  is in the image because a redeclaration of an entity outside it is
  refused (`pch-redirect-label-run.cpp`).

A function's builtin identity (the `builtin_id`, `lib_builtin` and
`gnu_lib_twin` side tables) travels as (record, map, id, spelling), the
spelling checked like a builtin attribute's. The importer attaches the
nodes right after it builds the declaration, in the unit's order, so its
consumers (codegen visibility and linkage, mangling, the use-site
warnings with their notes in the header, sanitizer checks) see the
replayed declaration's attributes. A kind the table does not list, or
a node whose words fall outside its row, falls back with "an attribute
the image does not carry yet", and the census lists the kind by name:
a wrong attribute on an imported declaration is a miscompile, so the
image never guesses. `aligned` and `alignas` with an expression or no
argument are carried since WP-1310 (row `ExprA`: `a` the expression or
none, the target's biggest alignment; the layout reads the attribute).
Not carried yet: `preferred_name` (a type argument), `diagnose_if` and
`enable_if` (conditions evaluated at the call), `alignas(type-id)` (a
type in `b`), `gsl::Owner` and `gsl::Pointer` with a type argument.

```
attrs   := strs:u32 str*
           recs:u32 arec*            ascending by record
           builtins:u32 (decl:u32 map:u8 id:u32 name:u32)*
           ints:u32 (value:u32 loc:u32)*
           params:u32 (pos:u32 arec)*  WP-1314: parameter attributes,
                                     `decl` the function, ascending
arec    := decl:u32 kind:u16 syntax:u8 flags:u8 loc:u32 a:u32 b:u32
           s:u32 scope:u32 name:u32  s, scope, name: strs indices
```

`syntax` is 0 C++11, 1 GNU, 2 keyword, 3 `__declspec`, 4 pragma. For
an expression kind `a` and `b` are expression records or none, for
`nonnull` the start and length of its run in `ints`, for a `Symbol`,
builtin or label kind `s` is the text and `a` is 0.

A function parameter's attributes (WP-1314: `[[clang::lifetimebound]]`
on libc++'s `std::move`, `std::forward`, `std::min`; `noescape`,
`maybe_unused`) are `params` records: the function's record, the
parameter's position and the node, attached to the parameter as the
importer builds it. Only plain-word kinds travel there; a parameter
with another kind falls back with "an attribute the image does not
carry yet" (`pch-param-attrs-*.cpp`, `image-param-attrs-*.cpp`).

**Scopes and reserved names (WP-1278).** `bmi_image/scope.rs` carries
the scopes and names beyond named namespaces and export blocks:

- a linkage specification is an `Export` record (unnamed and
  transparent) listed with its language and whether it is braceless;
  the importer builds the `LinkageSpec`. A C one in a namespace falls
  back in a module: [dcl.link]/6 makes its functions and variables the
  same entities as the global ones of their names, which a module's
  reader does not record. A precompiled header carries it (WP-1310):
  the header's own links are redeclaration chains, and the reader
  records the header's C-linkage entities, namespaced ones included, in
  the consumer's extern C context (libc++ `<cxxabi.h>`;
  `pch-extern-c-namespace-run.cpp`, `-errors.cpp`);
- an inline namespace is a `Namespace` record listed as inline;
- an unnamed namespace is a `Namespace` record with no name. The
  importer extends the unnamed namespace its scope already has
  ([namespace.unnamed]/1, as the parser does) or declares it with its
  implicit using-directive, which the writer leaves out;
- the namespace `std` is carried. The importer extends the hidden one
  sema declared (around `std::align_val_t`, for a new-expression) if it
  has one, as the parser's first `namespace std {` does;
- reserved names are carried. Sema declares two kinds of global-scope
  names by itself, which the gate cannot see: builtins and the hidden
  namespace `std`. A declaration at global scope named `main`, with a
  builtin's name, or with a name sema predeclares there falls back;
- a builtin function sema declared while compiling the unit (an
  implicit `Function` in an implicit `extern "C"` linkage
  specification at global scope) is an `Implicit` record of kind 6,
  unnamed and at global scope, listed with its builtin id and spelling;
  its linkage specification has no record. The importer uses the
  builtin it has declared already or declares it as its own lookup
  would, so bodies call the importer's declaration. An image naming a
  builtin the importer does not declare (another table or target) is
  corrupt; one whose type names `FILE` falls back;
- an unnamed class or enumeration that a typedef names for linkage
  purposes ([dcl.typedef]/9) is listed with that typedef; the importer
  links them when it builds the typedef, so linkage and mangling use the
  typedef name. One that no typedef names (`enum { FE_INVALID = 1 };`,
  [dcl.enum]/2) is carried unnamed (WP-1284; the reader used to reject
  it, so the writer's self-check made such a header "an internal image
  inconsistency").

```
scopes  := inline:u32 u32*           ascending records
           links:u32 (rec:u32 lang:u8 braceless:u8)*
           builtins:u32 (rec:u32 id:u32 spelling:str)*
           anon:u32 (tag:u32 typedef:u32)*   ascending by typedef
```

**Constraints (WP-1284).** `bmi_image/constraints.rs` and
`bmi_image/requires.rs` carry constrained templates, concepts and
requires-expressions as patterns ([temp.constr], [expr.prim.req]):

- a concept is a `Template` record of kind 4 with no pattern. Every
  constraint is a body expression listed in the `cons` section with
  the record it belongs to and its kind: 0 a template parameter's
  immediately-declared constraint ([temp.param]/4), 1 a template head's
  requires-clause, 2 a function's trailing requires-clause, 3 a
  concept's constraint-expression. Each record has at most one of each
  kind, a kind-1 one is on a template, a kind-2 one on a function, and
  every concept has its kind 3. The importer patches them in after the
  bodies, as the parser attaches them;
- a specialization reference to a constrained template (or to a class
  template with a constrained member) falls back with "a specialization
  of a constrained template": the importer would check its constraints
  while it builds the records, before they are in. So does a concept
  merged with another unit's, and a type-constraint naming a concept
  template parameter;
- satisfaction is never carried. A concept-id the unit checked falls
  back; a dependent one is carried unchecked and checked by the
  importer's instantiations exactly as after a replay, with the same
  notes in the module;
- a dependent requires-expression is a body expression whose body is a
  record of the `reqs` section (its context, location, flags,
  parameters and the invented `expr-type` parameters of its
  return-type requirements, [expr.prim.req.compound]/1). The importer
  declares them before it builds the bodies. A non-dependent one (the
  unit evaluated it) and one whose access checks wait for an
  instantiation fall back.

**Dependent types and names (WP-1284).** `bmi_image/deptypes.rs`
carries the dependent types a pattern names as type tag 21: a
dependent name (`typename T::type`, `T::inner::type`), a dependent
template specialization (`T::template rebind<U>`), a type-trait
transform (`__remove_cv(T)`), a member pointer whose class or pointee
is dependent, a vector of dependent element type, and a deduced class
template placeholder ([dcl.type.class.deduct]: the type of a variable
written `Wrap w(x)`, with its deduced type once known). A qualifier may
have a dependent-identifier step (`T::inner::`, step 4). The importer
builds each with the type context's constructors, so canonical types
are the replay's. A placeholder whose template is named through a
qualifier or a parameter, and a vector of value-dependent size, fall
back; the other expression-bearing types are "Expressions in types"
below.

*Written locations.* A dependent type or qualifier component has no
location of its own; the parser records where each was written
(`templ.written_locs`, spec/sema-templates.md §5.8) and substitution
reports a failure there. The `written` section carries the entries
whose key the image carries (a type of the type table, a qualifier all
of whose components are carried), so an error substituting an
imported pattern has the replay's position and caret.

**Deduction guides (WP-1284).** The implicit guides
([over.match.class.deduct]/1) are not carried: the importer forms them
on first use from the class template's constructors, as after a
replay, so a deduction's candidates and notes are the same. An explicit
guide is a `Function` record of kind 5 named by the special name word
`0xFFFF_FF03`, at namespace scope, alone or as a function template's
pattern; the importer builds it with `deduction_guide`, naming the
class template its return type specializes. A guide whose return type
is not written as `C<args>` (an alias or a qualifier), a member guide
("a member deduction guide": since WP-1292 carries member class
templates, the writer refuses it by name rather than leaving it to the
validator), and one with an `explicit(expr)` specifier fall back.

```
cons    := n:u32 (kind:u8 decl:u32 expr:u32)*
reqs    := n:u32 (ctx:u32 loc:u32 flags:u8
                  np:u32 (name:u32 loc:u32 ty:qref pflags:u16 flags:u8)*
                  ni:u32 (depth:u16 loc:u32)*)*
written := n:u32 (key wn:u32 loc:u32*)*   by first location
key     := 0 qref | 1 n:u32 step*
type    += 21 dep                    WP-1284
dep     := 0 keyword:u8 n:u32 step* name:u32          dependent name
         | 1 keyword:u8 n:u32 step* name:u32 n:u32 targ*
         | 2 kind:u8 base:qref result:qref            transform
         | 3 pointee:qref class:qref                  member pointer
         | 4 elem:qref count:u32 kind:u8              vector
         | 5 template:u32 deduced:qref                deduced placeholder
step    += 4 name:u32                dependent identifier
```

**Expressions in types (WP-1300).** `bmi_image/exprs.rs` carries the
types and template arguments that hold an expression. Each expression
is an *expression section* of `texprs`: a body arena of its own
(constants, strings, expressions, no statements) whose last
expression is the root. The forms:

- `decltype(e)` ([dcl.type.decltype]), type tag 22 kind 0: the
  section, the computed type (the `Dependent` builtin for a
  type-dependent operand) and flags (bit 0 instantiation-dependent,
  bit 1 canonical). `decltype(nullptr)`, `std::nullptr_t`, is the
  first of these (`<bits/c++config.h>`, `<stddef.h>`);
- a value-dependent array bound `T[e]` ([temp.dep.constexpr]), kind 1;
- a pack expansion `P...` of a known expansion count
  ([temp.variadic]), kind 2;
- `noexcept(e)` on a function type whose operand is not a `bool`
  literal ([except.spec]/2): exception bytes 7 (`noexcept(e)` false), 8
  (true) and 9 (value-dependent), followed by the section;
- a template argument written as an expression ([temp.arg.nontype]),
  in a written specialization (an alias template's arguments, a class
  template's when they are not a literal of the converted value, a
  dependent template specialization's): `targ` 2 with the section and
  whether the argument has a profile.

The canonical forms (`DependentDecltype`, a dependent array of a
profile bound) are written through the profile's first expression
with the canonical bit, and the importer takes the canonical type of
what it builds, so both share replay's profile and canonical type. The
importer builds a section with the first type naming it, before any
declaration record uses the type and before the image's bodies. It
recomputes each profile as replay does: `dependent_profile` for a
type's operand and `profile_expr` for an argument's. The validator
requires each type's sections to name only records its record bound
allows, and each section to be named by at least one type.

A section may not name a function or requires-expression parameter
or a local declaration, because the importer builds types first. The
writer refuses such a type with "an expression in a type naming a
parameter or local declaration". There is one exception: a function's
own `noexcept(e)` whose operand names its parameters, such as
`noexcept(noexcept(x.swap(y)))`. That function's type is carried with
no exception specification. A `late` entry gives the function record,
the exception byte and the operand's expression in the image's
bodies, and the importer sets the specification once the bodies are
built, as the declarator did. A trailing `decltype` naming
parameters still falls back. `SubstNonTypeTemplateParmExpr` (the
parameter record and its replacement), which an alias
specialization's substituted underlying type holds, is appended to
the kind table.

```
texprs  := n:u32 (consts strings exprs stmts)*   stmts empty
           late:u32 (rec:u32 exc:u8 expr:u32)*
type    += 22 ex                     WP-1300
ex      := 0 sect:u32 under:qref flags:u8        decltype(e)
         | 1 elem:qref sect:u32 canon:u8         dependent array
         | 2 pattern:qref n:u32                  counted pack
targ    += 2 sect:u32 profile:u8                 an expression
```

**Fallback census (WP-1070).** `-fkiln-bmi-image-census` (hidden, with
`--precompile` or `-fmodule-output`) builds the image a second time in
census mode and prints on stderr, after the compile's diagnostics, how
much of the unit it carries: "image census for '<module>': N top-level
declarations, C carried, R not", then one line per fallback reason with
its count and the first example in source order (`file:line:col`, or
the declaration's kind and name when it has no location), most frequent
first, then the attributes of the declarations refused for their
attributes, by name (a kind with no name as `(kind N)`). A top-level
declaration is one whose lexical parent is the translation unit, a
namespace, an export or a linkage specification (a member, a parameter or a template's pattern counts as
its outermost such declaration; carried scopes are not counted
themselves). The census records the *first* reason of each, rolls the
declaration back and goes on; a failing namespace is kept as a
placeholder so its members are counted on their own. A reason that is
the consequence of an earlier refusal ("... outside the image") is
prefixed `cascade: `. The module file written is the one without the
flag, byte for byte, and the image's own `-v` fallback reason is
unchanged (`kind2_image_census`). A unit with no image for a
unit-level reason prints "image census for '<module>': no image:
<reason>", and a non-module TU "image census: not a module unit".
`-fkiln-bmi-image-census=all` (WP-1320) also prints, last, one line per
refused top-level declaration in source order, "refused: <example>:
<reason>", for per-header tables.

**Versioning and the fingerprint.** The section starts with its own
format word (`IMAGE_VERSION = 33` since WP-1358, 32 for WP-1360, 31 for WP-1350, 30 for WP-1353, 29 for WP-1352, 28 for WP-1345, 27 for WP-1344, 26 for WP-1343, 25 for WP-1342, 24 for WP-1314, 23 for WP-1320, 22 for WP-1310, 21 for WP-1307, 20 for WP-1300, 19 for WP-1292, 18 for WP-1293, 16 for WP-1284, 15 for WP-1283, 14 for WP-1278, 13 for WP-1279, 12 for WP-1070 (S7), 11 for WP-1068 (S6), 10 for WP-1132, 9 for WP-1120, 8 for the first WP-1132 slice, 7 for WP-1162, 6 for WP-1166, 5 for WP-1139, 4 for the first S4
slice, 3 for S3, 2 for S2; rejected if different). The file
header's kiln version and the flags fingerprint (§11.4) already reject a
file made by another kiln or under other flags. `-fkiln-bmi-image` is
not part of the fingerprint: a file with an image can be used by an
importer with the flag off (it replays) and vice versa.

**Encoding** (little-endian `u32`s; every count is checked against the
bytes left before anything is allocated):

```
image   := version:u32 main_path:str module:str kw:u32 name_loc:u32
           preamble:u32              S6: end of the replayed preamble
                                     (u32::MAX: none)
           names:u32 str*            declared names
           gated:u32 u32*            ascending indices into names: the
                                     global-scope names (the pp's gate)
           files:u32 file*           S7: the file table (0: main file)
           slocs:u32 sloc*           S7: the location records
           pnames:u32 str*           parameter and local names (not gated)
           types:u32 type*
           decls:u32 offset:u32*decls blob_len:u32 decl*
           lookup:u32 (ctx:u32 name:u32 decl:u32)*   sorted
           consts:u32 const*         S2 bodies (empty arenas if none)
           strings:u32 (enc:u8 len:u32 byte*)*
           exprs:u32 offset:u32*exprs blob_len:u32 expr*
           stmts:u32 offset:u32*stmts blob_len:u32 stmt*
           attrs                     WP-1278 ("Attributes")
           scopes                    WP-1278 ("Scopes and reserved names")
           cspecs:u32 cspec*         WP-1283 ("Class template specializations")
           declocs:u32 (rec:u32 kind:u8 loc:u32)*   WP-1283, ascending
                                     (kind 0 type-spec, 1 `&`, 2 `*`/`[`)
           cons reqs                 WP-1284 ("Constraints")
           written                   WP-1284 ("Dependent types and names")
           lexterns:u32 u32*         WP-1293: block-scope declaration
                                     records, in `local_externs` order
           texprs                    WP-1300 ("Expressions in types")
           defcmps:u32 (rec:u32 kw:u32 (NONE | n:u32 u32*n))*
                                     WP-1314 ("Defaulted comparisons"),
                                     ascending by rec
cspec   := rec:u32 template:u32 (0 | 1 tloc:u32 langle:u32 rangle:u32
           params:u32 u32*) args:u32 carg*      ascending by rec
carg    := 0 qref | 1 qref value:i128   a type, a value
         | 2 parm:u32                   a head parameter record
         | 3 n:u32 carg*                a pack (top level only)
const   := 0 qref signed:u8 width:u32 u64*   integer (ceil(width/64) words)
         | 1 qref value:u8                   bool
         | 2 qref kind:u8 bits:u128          floating
         | 3 qref kind:u8                    null pointer / member
expr    := kind:u8 bits:u8 aux:u8 cold:u16 deps:u8 ty:qref loc:u32
           n:u32 word*n          deps: ExprDeps bits (S4; 1 type,
                                     2 value, 4 instantiation)
stmt    := kind:u8 aux:u16 loc:u32 n:u32 word*n
str     := len:u32 utf8*
qref    := type:u32 quals:u8         0 = none, 1..=90 builtin, else 91+i
type    := tag:u8 payload            1 pointer(qref), 2 lref(qref),
                                     3 rref(qref), 4 carray(qref, size:u64),
                                     5 iarray(qref), 6 paren(qref),
                                     7 elab(keyword:u8, qref),
                                     8 function(ret:qref, n:u32, qref*,
                                       info:u16, exc:u8 [sect:u32]),
                                     9 enum(decl:u32),
                                     10 typedef(decl:u32),
                                     11 qarray(cv:u8, qref),
                                     12 sugar(k:u8),
                                     13 record(decl:u32),
                                     14 elabq(keyword:u8, n:u32,
                                       step*n, qref),
                                     15 ttp(decl:u32)   S4
                                     16 tst(template:u32, n:u32,
                                       targ*n, under:qref)   S4b
                                     17 injected(template:u32)   S4b
                                     18 subst(repl:qref, template:u32,
                                       index:u32, pack:u32)   S4b
targ    := 0 qref | 1 qref value:i128   a type, an integral value
step    := 0 | 1 decl:u32 | 2 qref template:u8   `::`, namespace, type
         | 3 decl:u32                namespace alias (S6)
decl    := kind:u8 flags:u8 owner:u8 access:u8 name:u32 ctx:u32
           lexical:u32 loc:u32 prev:u32 payload
file    := name:str path:str len:u32 hash:u64 flags:u8 [text:str]
                                     flags: 1 system, 2 text follows
sloc    := 0 file:u32 from:u32 marks:u32 (off:u32 kind:u8)*   inclusion
         | 1 spelling:u32 len:u32 exp_lo:u32 exp_hi:u32
             name_lo:u32 name_hi:u32                          macro body
         | 2 spelling:u32 len:u32 exp_lo:u32 exp_hi:u32       macro arg
```

Every `loc` word above (and in the body records) is an image location
(S7, "Locations" below); `u32::MAX` is none.

In a record, `u32::MAX` means none (for `ctx`, the translation unit),
and `lexical` is `u32::MAX` when it equals `ctx` and `u32::MAX - 1` for
the translation unit. Tag 11 is a cv-qualified array spelled through a
typedef, parentheses or an elaborated name (`const Quad&` where `Quad`
names an array type): the reader rebuilds it with the type context's
`qualify`, which pushes the qualifiers onto the element type as
[dcl.array]/6 and [basic.type.qualifier]/3 require. Tag 7 is an
unqualified elaborated type; tag 14 (S3) one with a nested-name-specifier,
whose steps are `::` (first only), a namespace record of the image, or a
type (a class of the image, through any carried type) with its
`template` keyword bit; a namespace step counts as a dependency in the
type table's order. The exception byte is 0 (none), 1
(`noexcept`), 2 (`throw()`), 3 (canonical non-throwing), and since
WP-1278 4 (`__attribute__((nothrow))`), 5 (`noexcept(true)`) and 6
(`noexcept(false)`), both written as a `bool`
literal, as the C library's `__THROW` expands; the importer builds the
constant expression again). Since WP-1300, 7 to 9 are `noexcept(e)`
of any other operand, followed by its expression section ("Expressions
in types").

The owner byte holds bit 0 = in the purview and bit 1 = exported (the
`ModuleTable` owner point of the record). The access byte (S3) is a
class member's `Access` (1 public, 2 protected, 3 private) and 0
elsewhere. A name word at or above `0xFFFF_FE00` is a special name
(S3): `0xFFFF_FF00` constructor, `0xFFFF_FF01` destructor,
`0xFFFF_FF02` conversion function, `0xFFFF_FE00 + op` `operator@`; the
importer recomputes the name from the class or type. The kind byte is
1 namespace, 2 export, 3 function, 4 variable, 5 enumeration, 6
enumerator, 7 typedef, 8 alias, 9 label, 10 lifetime-extended
temporary, 11 class, 12 field, 13 access specifier, 14 implicit
special member, 15 template type parameter, 16 non-type template
parameter, 17 template (S4), 18 specialization reference and 19 member
reference (S4b, WP-1139), and since S6 (WP-1068) 20 using-declaration,
21 using-shadow, 22 using-directive, 23 namespace alias and 24
using-enum, 25 friend (WP-1292) and 26 static assertion (WP-1307). The flags byte may hold `DEPENDENT_CTX`
(64) on parameters and patterns of templates and on the members of a
class template's pattern (S4). The payloads are:

- function: the declaration kind (0 function, 1 member function, 2
  constructor, 3 destructor, 4 conversion function; 0 exactly when the
  context is not a class), type, `FnSpec` bits (u32), a byte that is 1
  when the exception specification is deferred to the function (an
  implicitly `noexcept` destructor, or an explicitly defaulted special
  member whose specification the unit left unevaluated; the type is
  written without one; a referenced defaulted member whose byte is 1 is
  not evaluated by the importer, since a reference from a template
  definition left it unevaluated in the unit too, WP-1300),
  the parameters (n, then per
  parameter: name index into `pnames` or none, location, type, the
  parameter flags u16, the storage flags u8, the default argument, an
  expression index or `u32::MAX`, and since WP-1310 a byte, 1 when the
  type as written follows (an array or function type the declaration
  adjusted, [dcl.fct]/5: `char[20]`, `int (*)(int)` written `int(int)`;
  instantiating a function template's pattern substitutes into it)),
  then the body: 0 (none), 1 and the
  offsets of its braces (a dropped non-inline body), or 2 and a
  statement index (a carried body, S2), then the ctor-initializers (S3;
  n, then per initializer: `CtorInitKind` u8, target u32, written
  index u8, the initializer's expression index, location). A `Base`
  initializer's target is the base's position in the class's base
  list; a `VirtualBase` one's (WP-1120) is the virtual base's type
  (an unqualified type index, a class the constructor's class has as a
  direct or indirect base); a `Field` initializer's is a field record
  of the same class (which may follow the constructor); a `Delegating`
  one's is 0. Only a constructor with a carried body has initializers;
  then the functions it overrides (WP-1120; n, then each a record: a
  first declaration of a member function, destructor or conversion
  function, or an implicit destructor or assignment operator record,
  never itself). Only a first declaration of a member function,
  destructor or conversion function lists any; a redeclaration takes
  its first declaration's list;
- variable: type, `VarSpec` bits (u32), the initializer (an expression
  index or `u32::MAX`) and the evaluation state (`u32::MAX` not
  evaluated, `u32::MAX - 1` not a constant, else a constant index);
- label (S2, local only): nothing;
- lifetime-extended temporary (S2, local only): type, the extending
  variable (an earlier local variable record of the same function) and
  the temporary's index;
- enumeration: scoped, the fixed underlying and promotion types (or 0),
  and when complete the computed ones;
- enumerator: its type, a byte (1: a value follows, 16 bytes,
  little-endian `i128`; 0: none) and the initializer (`u32`, an
  expression or `u32::MAX`); the type may be sugared (an enumeration
  whose fixed underlying type is a typedef) and the value a `bool` (as
  0 or 1), since WP-1307; a qualified type falls back. Since WP-1310 an
  enumerator of a dependent enumeration (a member of a class template
  or of its member class) has no value and keeps its initializer as
  written, which each instantiation of the class substitutes; a value
  with an initializer, or no value outside a dependent context, is
  malformed;
- typedef and alias: the underlying type;
- namespace and export: nothing;
- class (S3): the `TagKind` byte, then 0 (a declaration) or 1 and the
  definition: its `DefinitionBits` (u64, without `COMPLETE` and
  `BEING_DEFINED`), the end (the first record after its members), the
  key function (WP-1120; `u32::MAX`, or a member function, destructor
  or conversion function record declared inside the class) and the
  base-specifiers (n, then per base: type, location, access, and a
  byte that is 1 for a virtual base, 0 otherwise);
- field (S3): type, the carried `FieldData` flags (`mutable`, the
  initializer style, has a default member initializer), the bit-field
  width and the default member initializer (expression indices or
  `u32::MAX`);
- access specifier (S3): nothing (its access is the header's);
- static assertion (WP-1307): the condition and the message (expression
  indices; the message `u32::MAX` when there is none);
- implicit special member (S3): the `SpecialKind` (0 default
  constructor, 1 copy and 2 move constructor, 3 copy and 4 move
  assignment, 5 destructor); its context is a complete class that
  precedes it, and the record lies after that class's end;
- template type parameter (S4): depth u16, index u16, a byte that is 1
  for `typename` (0 for `class`), the location of that keyword
  (`u32::MAX` when none) and the default argument, a `qref` (type 0
  for none, S4b); its name is a `pnames` index or none;
- non-type template parameter (S4): type, depth u16, index u16;
- template (S4): the template kind (0 a function template, 1 a class
  template, S4b), the pattern record, the `template`, `<` and `>`
  locations, the depth u16 and the parameter records (n, then each);
- specialization reference (S4b): the template record, the completed
  byte and the arguments (n, then `targ`s);
- member reference (S4b): the specialization reference record and the
  pattern member record.
- using-declaration (S6): the qualifier (n, then `step`s);
- using-shadow, namespace alias and using-enum (S6): the target record;
- using-directive (S6): the nominated namespace's first record.

**Bodies (S2, WP-1064).** The four body sections follow the lookup
table. Every expression and statement record is the AST node's header
plus its two payload slots, written as the fields a per-kind schema
names (`expr_shape`, `stmt_shape` in `kiln_sema::bmi_image::body`). The
writer, the decoder's validator and the reader walk the same schema, so
there is no second encoding. A field is one of: a raw word, a bool, an
expression or statement index (or an optional one, `u32::MAX` for
none), a declaration reference of a given class, a constant or string
index, a location, a type (`qref`, two words), a counted list of
expressions, statements, declarations or types, a cast's base path (0
none, else 1 + the number of steps, then each step's class record and
base position, S3), a nested-name-specifier (a count, then components:
0 `::`, 1 namespace and record, 2 type and `qref` and bool), a
statement's attribute list, and a `_Generic`'s operand list (WP-1132:
the association count n, the control, then n associations of four
words: the type as a `qref`, `u32::MAX` and 0 for `default`, the
type's or `default`'s location and the expression; the control is an
expression index and a location, or with bits 16, the type-name form,
a `qref` and its location). A declaration reference is a record index,
`0x8000_0000 | i` for the image's i-th parameter (counting the
parameters of the function records in order), or `u32::MAX - 1` for the
translation unit. The expression record's `cold` mask (a byte until
WP-1284 widened it to `u16`) says which cold annotations follow the
slots: 1 end location, 2 written type, 4
operator location, 8 begin location, 16 member-access operator
location (S4b), 32 the copy of a default argument or default member
initializer rebuilt for a lifetime-extending use (an expression index,
written before the `CXXDefaultArgExpr` or `CXXDefaultInitExpr` it belongs
to and rejected on any other kind; spec/sema.md §7.6a, WP-1166, WP-1177:
without it an importer gave the copy's temporaries full-expression
lifetime). A `CXXDefaultInitExpr` copy uses the same field, so the format
word did not change for it. Bit 512 (WP-1310) is a `CXXDefaultArgExpr` or
`CXXDefaultInitExpr` use whose initializer holds a source-location
builtin: the location and context its values are for and a `bool`, the
`__impl` object too (rejected on any other kind).

The carried kinds (`EXPR_KINDS`, `STMT_KINDS`; append-only tables, the
record's kind byte is the index):

- expressions: integer, floating, character, string, `bool`,
  `nullptr` and `__null` literals; declaration references; predefined
  names (`__func__`); unary, binary, compound-assignment and conditional
  operators; `__builtin_choose_expr` (the condition, both arms, aux 1
  or 2 for the chosen arm, 0 while the condition is dependent);
  `_Generic` (WP-1132; clang's `GenericSelectionExpr`: the operand list
  above and the selected association's expression, `u32::MAX` while
  the selection is result-dependent; the decoder rejects a selection
  that is not one of the associations' expressions and a second
  `default`; the value category, type and dependence are the header's,
  copied from the selected expression when the selection was made);
  subscripts; parentheses; calls and
  operator calls; implicit, C-style, functional, `static_cast`,
  `const_cast` and `reinterpret_cast` casts; initializer lists, implicit
  value initialization and `T()`; `sizeof`/`alignof`, type traits and
  `noexcept`; constant expressions; materialized temporaries,
  expressions with cleanups and opaque values; GNU statement
  expressions; `throw`; default arguments; `&&label`; array init loops
  and their index; since S3 member access, `this`, member calls,
  constructions (`CXXConstructExpr`, `T(args)`), default member
  initializers, bound temporaries (the destructor to run) and
  derived-to-base casts with their path; since WP-1310 the
  source-location builtins (`__builtin_LINE()` and the rest, aux the
  `SourceLocKind`, slot a the context the call was written in). Their
  values are not carried: once the image's declarations and bodies are
  built the importer computes them again, at the call's own location
  and context, and at each default argument or member initializer use
  at the bit-512 location and context, as sema computed them at the
  producer (`__builtin_source_location()` finds and checks
  `std::source_location::__impl` first). A consteval
  `std::source_location::current()` called in a header body still
  falls back, its result being a pointer constant ("a constant the
  image does not carry"). Tests: `pch-source-loc-run.cpp`,
  `image-source-loc-run.cpp`;
- statements: compound, declaration, expression, null, `if`, `switch`,
  `case`, `default`, `break`, `continue`, `while`, `do`, `for`,
  range-`for`, `return`, label, `goto`, and attributed statements whose
  attributes take no arguments (`[[fallthrough]]`, `[[likely]]`,
  `[[unlikely]]`, in C++11 or GNU syntax);
- local declarations: variables (including `static` locals and
  range-`for` variables), labels and lifetime-extended temporaries
  (`const int& r = 4;`). A local is a declaration record whose context
  is a function record; it is not in the lookup table. In a PCH, a
  block-scope `extern` variable or function declaration (WP-1293) is a
  local `Var` or `Function` record (kind 0, no body) written in
  declaration order, not on first use, and its `prev` may name an
  earlier record at namespace or block scope, so a later namespace-scope
  declaration of the header links to it ([basic.link]/6). `lexterns`
  lists them in Sema's `local_externs` order; the reader pushes each
  with its enclosing namespace, so the consumer's declarations find
  them as replay's do.

Records are in post order, so every operand and sub-statement has a
lower index. The one edge from an expression to a statement, a GNU
statement expression, is checked to point below the expression, so a
decoded image is acyclic. Decoding validates each record against its
schema (kind in range, value-category bits, `aux`, cold mask, every
index and reference of the right class) before anything is built. Each
type that a builtin spells through sugar (`__size_t`,
`__signed_size_t`, `__ptrdiff_t`, the types of `sizeof` and pointer
differences) is type tag 12, which the reader maps to its own sugar
node; since WP-1310 so are the implicit typedefs `__int128_t` (3),
`__uint128_t` (4) and `__builtin_va_list` (5), which the importer
declares too (a specialization for `__int128_t`, `<cstdarg>`'s
`__gnuc_va_list`).

The reader builds the records through `AstBuilder` in order, after the
declarations they refer to: parameters and locals first, then the
expressions and statements, then it attaches each function's body, each
parameter's default argument and each variable's initializer. A carried
constant value is restored as the variable's evaluated state (scalar
values only; an aggregate is recomputed by the importer on first use).
The importer's constant evaluator and codegen then see the same tree as
the replay's, so `static_assert`, constant evaluation and inlining
behave identically.

Falls back (each with its `-v` reason):

- `new`/`delete` ("a new or delete expression": the global allocation
  functions are implicit declarations the image does not carry),
  `typeid` and `dynamic_cast` ("a typeid or dynamic_cast expression"),
  `std::initializer_list` construction, inherited constructors,
  pseudo-destructor calls: follow-ups of S3 (WP-1065 in the backlog);
- lambdas ("a lambda expression"; a closure is a local class): S3
  follow-up;
- template expressions, requires-expressions and concepts: S4 (in a
  pattern, the dependent kinds other than those the slices carry are
  "a template expression (...)", see "Templates as patterns");
- `try` blocks and function-try-blocks, coroutines, expansion
  statements, `asm`, contract assertions, computed `goto`, and
  attributes with arguments (`[[assume(...)]]`): follow-ups under
  WP-1064 in the backlog;
- a lifetime-extended temporary with a constant value (bound to a
  `static` or `constexpr` reference) or at namespace scope;
- a location in a predefined buffer that is not in memory ("a
  location in a predefined buffer (<built-in>, <command line>)"; the
  in-memory ones are carried since WP-1310), in a file on disk with
  `#line` directives or line markers, whose presumed file is not one of
  the file table's ("a file with #line directives or line markers"), or
  in a file that was not valid UTF-8 (S7, see "Locations").

**Classes (S3, WP-1065).** A class (`struct`, `class` or `union`) is
a class record; its members are the records between it and its end:
fields, access specifiers, member functions, constructors, the
destructor and conversion functions, static data members, nested
classes, member enumerations and member typedefs, each with its access
byte. A member function defined outside its class is a later record
whose context is the class and whose lexical context is the enclosing
namespace, after the class's end, with the in-class declaration as
`prev`. What is carried and what is recomputed:

- carried: the `DefinitionBits` of the definition (triviality, both
  bits per special-member kind: `TRIVIAL_*` and WP-1109's
  `NONTRIVIAL_*`, bits 57-61, [special]/6; the implicit members'
  constexpr-ness and parameter shapes, empty/standard layout, ...) and
  each member function's `FnSpec`, including `not_selected` (an
  ineligible special member or a prospective destructor that was not
  selected, [special]/6, [class.dtor]/4; WP-1119), frozen as sema computed them at the closing brace, so
  the importer does not redo [class.prop] or [class.default.ctor]
  analysis; the base-specifiers; every field with its default member
  initializer and bit-field width; every member function's body as for
  S2 (an inline member body is carried, a non-inline one is braces only,
  [class.mfct]/1, because the module's object file defines it); a
  constructor's ctor-initializers;
- carried for polymorphic classes (WP-1120): the `POLYMORPHIC`,
  `ABSTRACT` and `HAS_VIRTUAL_BASES` definition bits; the `FnSpec`
  bits `virtual` (semantic and as written), `pure`, `override` and
  `final` ([class.virtual], [class.abstract]; a `final` class is its
  `final` attribute); each first declaration's overridden functions,
  the set sema computed at the declaration ([class.virtual]/2; the
  importer installs it, a redeclaration shares it), so final-overrider
  and vtable layout inputs are the replay's; the class's key function
  (the cold `key_function` map, Itanium ABI §5.2.3), so the importer
  emits a vtable, VTT, RTTI and thunks with the same symbols and
  linkage as a replay or a kind-1 import (the vtable of a
  module-attached class belongs to the unit); a base-specifier's
  `virtual` and a constructor's `VirtualBase` initializers ([class.mi]/4,
  [class.base.init]/7). A carried destructor body re-runs the
  destructor's `operator delete` lookup ([class.dtor]/16) so a virtual
  destructor's deleting variant is the replay's;
- recomputed: the implicit special members. The record says only which
  ones sema had declared (sema declares them lazily, [class.copy.ctor]/6
  and peers, plus whatever the module's own bodies used); the importer
  calls the same `declare_implicit` on the completed class, so their
  signatures, `noexcept` and `constexpr` come from the carried bits and
  match the replay's, and any other one is declared on demand as
  usual. The ones sema declared eagerly at `}` because they need
  overload resolution over a member's or base's special members (a
  non-trivial, deleted, inaccessible or non-`const`-parameter one,
  `finish_special_members`), or because the class is dynamic, are among
  the carried records, so the importer declares the same set as the
  replay, each computed (deleted or not) from the completed class as
  there (WP-1292; before it such a class fell
  back, "a class whose implicit special members need overload
  resolution"). Declaring one may declare another first, as a derived class's
  default constructor declares its base's: that record then takes the
  member already declared. An implicit destructor or assignment
  operator recomputes the functions it overrides and its `virtual`
  bit. Their definitions are synthesized on first ODR-use as for a
  replayed class. Layout, the record's size and the field offsets are
  recomputed by the importer's codegen and constant evaluator from the
  members; special names are recomputed from the class.

The importer builds each class with `begin_record_definition`, its
members in order, and closes it with `complete_record_definition` and
the carried bits; it then builds the bodies and attaches the fields'
initializers and widths, the ctor-initializers (a base by its position
in the built class's base list, a field by its record) and the bodies.
The decoder checks, before anything is built, that members lie inside
their class, that an out-of-line member and an implicit member name a
complete earlier class, that a class is defined once and its end lies
inside its parent's, that bases are earlier complete classes (not
itself), that access bytes are 1 to 3 exactly on class members, that
function kinds match their context, that only constructors have
ctor-initializers, and every initializer's target; since WP-1120 also
that a key function is a member function, destructor or conversion
function record inside its class ("bad key function"), that overridden
functions are first declarations of member functions (or implicit
destructor or assignment operator records) other than the function
itself and only a first declaration lists any ("bad overridden
functions"), and that a base's flag byte is 0 to 3 (bit 0 `virtual`,
bit 1 a pack expansion, WP-1292).

A class falls back (with its `-v` reason) when it declares a destroying `operator delete` ("a destroying
operator delete"), redeclares a virtual function with other overridden
functions than its first declaration's, overrides a function outside
the image ("an override of a function outside the image"; S6), has a
key function outside the image, inherits constructors or assignment operators, is an
anonymous union or struct (or a member of one, or names one through a
typedef), is a template or a closure type, is incomplete,
befriends a member function or a specialization (friends of other
classes are carried since WP-1292, friend templates and friends in class
templates since WP-1307), a
class-specific allocation function, an out-of-line defaulted function
(a defaulted comparison is carried since WP-1314, "Defaulted
comparisons"), a `[[no_unique_address]]` field, any
other side data sema keeps in its member maps ("a declaration with
side data the image does not carry", for example a member named
through a qualified name), or any `DefinitionBits` outside
`DEF_BITS`. A function declared through a typedef of a function type,
a conversion function named through a typedef, and an implicit
declaration that is not a carried special member (a builtin such as
`__builtin_printf` that a body names declares a library function, "a
builtin or library function") fall back too. A qualified type whose
qualifier names a namespace that is not in the image ("a qualifier
naming a namespace outside the image"), a namespace alias, a dependent
name or a splice falls back.

**The reader.** When the module's `UnitBegin` event carries an image, the
parser calls `act_on_unit_begin` and then the image load. The load
decodes the whole section first: decoding is total, and every
cross-reference (an earlier type or record, of the right kind) is checked
there. Only then does it apply the module directive and build each
record, setting the owner point first. A decoding error is the existing
corrupt-module-file fatal error, and no declaration is built.

**What the preprocessor does.** It reads the image only when its own flag
is on and the gate passes. It then moves the section out of the capsule
(the file is never held twice), maps the main file, and queues
`UnitBegin` (carrying the image) and `UnitEnd` back to back. No capsule
file is lexed. The unit's exported macros are none (a named module), so
nothing else is lost.

**Precompiled headers (WP-1036).** A PCH's declaration image
(`bmi_image/pch.rs`; the driver side is spec/driver.md "The declaration
image") is the same format with one difference: the module name is
empty and the module keyword, name and preamble words are `NONE`. A
module name is never empty, so neither reader accepts the other's
image. The producer writes the header's declarations as if the header
were the purview of a unit: every top-level declaration of the
translation unit in a real file, owned by no module. Every function
body is carried, not just the inline ones, because a consumer emits
the header's definitions as textual inclusion does. The writer runs in
`pch` mode. The first declaration it refuses means no image at all,
never a partial one. `-fkiln-bmi-image-census` on the producer counts
the header's reasons. The consumer (`Sema::load_pch_image`) decodes and
checks the image when the driver chooses the path, before
preprocessing. It loads the image's file table into the source manager
and builds the records before the unit's first token, in the global
module (owner byte 0, the unit's own), so the unit may redeclare,
define, specialize and instantiate them. `install_pragma_state` sets
the header's Sema pragma stacks from the record (§11.13). The section
layout is the module image's; `lexterns` (WP-1293) is empty in a module
image. The consumer also records the header's namespace-scope functions
and variables with C language linkage in its "extern C context"
(`Sema::extern_c`, [dcl.link]/6-7) as replay does, so its own C-linkage
declarations in other namespaces redeclare or conflict with them
(WP-1293).

#### Part 2a coverage and follow-ups

2a's image covers modules made only of the node kinds above. Every other
module replays, and that includes `std` (it has a global module fragment,
classes and templates). The follow-ups, in priority order:

1. S2: expression and statement records, and inline, `constexpr` and
   `consteval` functions. **Done (WP-1064)**, see "Bodies"; bodies are
   loaded eagerly, but every record stays addressable by index for
   S-lazy (WP-1069);
2. S3: classes (fields, methods, bases, special members). **Done for
   non-template classes (WP-1065), polymorphic ones with virtual
   functions, key functions and virtual bases since WP-1120**, see
   "Classes (S3)". Its follow-ups: `dynamic_cast`/`typeid` in bodies
   (polymorphic class templates and friends of classes are carried
   since WP-1292, and so are classes whose implicit members need
   overload resolution; friend templates and friends in class templates
   since WP-1307), anonymous unions and structs,
   inherited constructors, out-of-line defaulted functions (defaulted
   comparisons since WP-1314), class-specific allocation functions,
   `[[no_unique_address]]`, the member side maps (qualified member
   names), and in bodies `new`/`delete` (the implicit global allocation
   functions), `typeid`/`dynamic_cast`, `std::initializer_list` and
   lambdas;
3. S4: templates as patterns. **First slice landed: namespace-scope
   function templates (WP-1066)**; **second slice landed:
   specialization references and class templates (WP-1139)**;
   **third slice landed: packs, non-type defaults, alias and variable
   templates, unresolved names (WP-1279)**, see "Templates as
   patterns" and its follow-ups;
4. S5: global module fragments with ODR merging (replacing the name
   gate);
5. S6: imports, re-exports and partitions. **Landed (WP-1068)**, see
   "Imports, partitions and using-declarations (S6)"; its follow-ups:
   external references to imported entities (and with them
   using-declarations of imported or global-module-fragment entities),
   namespaces reopened across units, class-scope using-declarations;
6. S-lazy: the lazy lookup hook (see above);
7. S7: locations in other files and in macro expansions. **Done
   (WP-1070)**, see "Locations (S7)"; its follow-ups: `#line` and line
   markers in files on disk (a presumed-file table; the predefines
   buffer's are carried since WP-1310), and reading the
   file table at `import` time only for the files a diagnostic needs
   (today every non-main file is stat'ed when the image is read).

**What `import std;` still needs after S3.** libc++'s `std.cppm` falls
back on its first node, the global module fragment (S5): every header is
included there, and the purview is only `export namespace std { using
std::...; }`. With S3 in, the constructs between `std` and the image
are, by stage:

- S3 follow-ups: classes are carried since WP-1065 and polymorphic
  ones with virtual bases since WP-1120 (`exception`, `ios_base`,
  `locale::facet`, `memory_resource`, `error_category`; the class
  templates among them, `basic_istream` and `basic_ostream` over
  `basic_ios`, wait for S4), friends (granted across
  libc++, `__wrap_iter`'s for one; those of non-template classes are
  carried since WP-1292), anonymous unions (`optional`,
  `variant`, `string`'s representation), `[[no_unique_address]]` (allocators,
  `unique_ptr`'s deleter through `_LIBCPP_COMPRESSED_PAIR`), and in bodies
  `new`/`delete` and lambdas; every one of these is inside a template
  in libc++ and so also waits for S4;
- S4: templates, concepts and requires-expressions, which are most of
  the headers;
- S5: the global module fragment itself, and entity merging with other
  imports;
- S6: the purview's using-declarations are carried since WP-1068, but
  `std`'s name entities of its global module fragment ("a
  using-declaration naming a declaration outside the image"), so they
  wait for S5 and the external-reference follow-up;
- S7: locations in the headers (other files) and in macro expansions
  (`_LIBCPP_*`) are carried since WP-1070; the fallback census of a
  purview that #includes every libc++ header (docs/status/pch-image-census.md)
  puts its remaining location fallbacks at two declarations (a
  `<built-in>` location) out of about 25 thousand, behind attributes,
  reserved names and templates;
- follow-ups under WP-1064: attributes with arguments
  (`__attribute__((__visibility__(...), __abi_tag__(...)))` through
  `_LIBCPP_HIDE_FROM_ABI`, `[[__assume__]]`), `try`/`catch` (in
  `<ios>`, `<locale>`, `<future>`), and `asm`.

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

**Stamp.** Beside `<name>.pcm`, `<name>.stamp` holds a magic line
(`kiln-module-cache-stamp 2`), then `<size> <checksum> <path>` for the
module file, where `<checksum>` is its trailing §11.3 checksum in hex,
then one `<size> <mtime> <path>` line for every file the build read.
These are the dependency list of an `-MD` with system headers, plus the
source. A hit needs the stamp valid, the module file's size and checksum
the stamp's, every other file unchanged, and the module file's first
bytes kiln's magic. So a libc++ header edited in place, or a damaged
module file, is a miss. This is clang's module-cache validation (size
and mtime, not content) for the inputs; the module file itself is named
by its content, because racing compiles each rename their own copy of
it into place (see "Concurrency").

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

**Concurrency** (WP-1074). Parallel builds (`make -j`, ninja) start
many compiles that miss the same entry at once. The rules:

- **A hit takes no lock.** It reads the stamp, then the module file.
  Each is replaced only by a rename of a complete temporary file, so
  neither is ever torn, and the stamp names the module file by its
  content. A module file renamed into place after the stamp was read
  either has the same bytes (the writer is deterministic: every compile
  of one key writes the same file) or fails the stamp and is a miss.
- **A miss takes the entry lock.** `<name>.lock` in the entry, an
  advisory exclusive lock (`flock`, `File::try_lock`, polled with a
  back-off of 1 to 50 ms). The kernel releases it when its holder exits,
  a SIGKILL included. Holding it, the compile checks the entry again: a
  compile that waited finds what the holder published and uses it. So
  one compile per entry builds and the others wait, instead of every one
  of them building.
- **The holder clears and publishes.** Only a holder writes the
  temporary files of `<name>` (`<name>.pcm.tmp.*`, written by
  `--precompile`, and `<name>.stamp.tmp.*`), so any it finds were left by
  a killed holder, and it removes them. It then builds into
  `<name>.pcm` (renamed into place) and writes the stamp (renamed into
  place) last. A failed build removes the entry directory when nothing
  else is in it.
- **The lock file does not outlive its holder.** The holder removes
  `<name>.lock` while it still holds the lock, then releases it. A
  waiter that then gets the lock checks that the file it locked is still
  the one at the path (device and inode); if not, it opens the path
  again. A lock file left by a killed holder is removed by the next
  compile that takes it, or by the next hit when no one holds it. A
  settled entry holds exactly `<name>.pcm` and `<name>.stamp`.
- **Waiting is bounded.** A waiter gives up after 120 seconds
  (`KILN_MODULE_CACHE_WAIT_MS` replaces the limit, in milliseconds; for
  kiln's tests) and builds the module from its source (part A), leaving
  the entry alone. So a stuck compile delays the others, never fails
  them. An error taking the lock (a file system without locks, an
  unwritable entry) also falls back to part A.
- **Where locks do not exclude.** Some file systems accept `flock` but
  do not enforce it between processes (a macOS directory mounted into a
  Linux container). There every racer builds, renames and writes its
  stamp, as before WP-1074. The content-named stamp keeps that correct:
  whichever module file and stamp are renamed last, they match. A
  temporary file removed by another racer's clean-up only fails that
  racer's build, which falls back to part A.
- `std.compat` is built under its own lock and imports `std` through
  the cache, taking `std`'s lock inside it. Locks are always taken in
  that order and held only for one lookup, so two compiles cannot
  deadlock.

Before WP-1074 there was no lock and the stamp named the module file by
its modification time. The module file and the stamp renamed into place
last could then come from different racers, and the entry left behind
was a miss, rebuilt by the next compile (1 in 100 races of 4 compiles,
about 1 in 12 of 16).

**Pruning** (WP-1050). The key covers kiln's identity, so every rebuilt
kiln, like every other configuration, starts new entries of about 9 MiB
each. The cache is pruned as clang prunes its implicit module cache
(`maybePruneImpl` in clang/lib/Serialization/ModuleCache.cpp, clang 22;
`pruneModuleCache` in clang/lib/Frontend/CompilerInstance.cpp, clang
19; the rules are the same in both):

- **When.** After a compile's first lookup in the cache, hit or miss, it
  looks at `<root>/modules.timestamp` (clang's name). Missing: it is
  created and nothing more happens, so the first pruning comes one
  interval after the cache's first use. Modified no longer ago than
  `-fmodules-prune-interval=` (default 604800 s, 7 days): nothing.
  Otherwise its modification time is set to now and the cache is pruned.
  An interval or a prune-after of 0 turns pruning off. The values are
  clang's `unsigned` seconds: decimal, `0x` hex or `0` octal, up to
  4294967295, else clang's "invalid integral value" error.
- **One pruner per interval.** clang rewrites the timestamp and lets two
  compiles that noticed it at once both prune ("a benign race"). kiln
  takes an advisory lock on the timestamp file, never waiting, checks
  its age again under the lock, and only then updates it. A timestamp
  that cannot be updated (a cache another user owns) means no pruning,
  not pruning on every compile.
- **What.** Only directories of the root named as keys (32 lowercase hex
  digits), found with `lstat`: a link is never followed, and anything
  else in the root (clang's own cache, if the root is shared) is left
  alone. The compile's own entry is skipped. clang walks every
  subdirectory and removes `.pcm`, `.timestamp` and `modules.idx` files
  one by one; kiln removes whole entries.
- **Last use.** An entry is unused when the newest modification time of
  its files (lock files aside) is older than `-fmodules-prune-after=`
  (default 2678400 s, 31 days). clang reads each module file's access
  time instead. kiln does not, because `noatime` mounts never update it,
  `relatime` ones only once a day, and anything that reads the files (a
  backup, an indexer) would keep them forever. Instead a hit sets its
  stamp's modification time to now when it is older than a day (or than
  half the prune-after, when that is shorter), so a used entry writes at
  most once a day. The module file is never touched: importers'
  dependency files list it, and a newer time would rebuild them.
- **Safety.** The pruner takes the entry locks (`std.lock`,
  `std.compat.lock`, and the lock of any other module the entry has
  files for) without waiting. A lock another compile holds, because it
  builds or waits to, keeps the entry, as does a file of a module it did
  not lock. Holding the locks, it renames each `<name>.stamp` to
  `<name>.stamp.pruning` and checks the last use again. A hit notes its
  use through a handle it opened on the stamp and then checks that the
  file is still at the stamp's path. So a hit either touched the stamp
  before that second check, which keeps the entry (the stamps are
  renamed back), or finds its stamp gone and is a miss, which waits for
  the locks and rebuilds. Then the pruner removes every file of the
  entry (links as links; it never descends into a directory), releases
  the locks, which removes their files, and removes the directory. A
  miss that creates its lock file there once the locks are released
  keeps the directory; one whose directory was removed between its
  `mkdir` and its lock makes the directory again. A holder of an entry lock removes a
  `<name>.stamp.pruning` a killed pruner left.
- **Failure is silent.** A removal that fails is said under `-v` (`cannot
  remove '<path>': <reason>`) and otherwise ignored; so is a cache that
  cannot be read. `-v` also says `pruning '<root>'` and `removed unused
  '<entry>'`.

**Dependency output** (WP-1051). The hook returns the module file and
the files its build read, as the stamp lists them (on a miss, the list
the new stamp is written from). `kiln_pp` reports the read through
`PpCallbacks::module_file_read` and brackets every unit read from a
module file with `PpCallbacks::module_unit`, so the dependency list
skips what a capsule replay enters (an image reads no file at all). An
importer's `-MD`/`-MMD` rule therefore lists the entry's `<name>.pcm`
(absolute) and, under `-MD`, the stamped inputs as system files: the
same bytes on a hit and on a miss, and for both kinds of module file
(spec/driver.md "Dependency output"). The nested build passes
`-fno-module-file-deps`, so `std.compat`'s stamp lists `std`'s inputs
but not `std.pcm`, whose modification time changes whenever a racing
compile renames the same bytes into place.

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

- Pruning (WP-1050, above) runs only when some compile uses the cache,
  as in clang: a cache no compile touches again is never pruned, and
  its entries (about 9 MiB each) stay until removed by hand.

### 11.13 The preprocessor-state record (WP-1280)

The preprocessor state a header leaves behind, as a module-file section
(`SECTION_PP` = 3, `kiln_pp::state`). It is the first piece of a real
PCH (spec/driver.md "Precompiled headers", `-fkiln-pch-image`), and
header units' exported macros go through it. Loading it replaces
reading the header for everything the preprocessor keeps; Sema's state
(the declarations, and the pragma state below, which is carried but
installed with the declaration image of a PCH, §11.11 "Precompiled
headers") is WP-1036's.

**What it holds.** The difference between the state where the header
begins (the *baseline*: the predefines and the command line read, no
`-include` yet) and the state at its end (spec/pp.md
"Preprocessor-state record" defines the points):

- **Macros.** Every definition made or replaced, by name, with its
  locations (the name's range, where the definition starts and its
  length), its parameters, its flags (function-like, variadic, named
  variadic, used, allowed to be redefined) and its body as compact
  tokens. A builtin macro's definition (`__LINE__`, `__COUNTER__`, ...,
  as a `push_macro` stack holds it) is its builtin index. **Undefs**:
  the baseline macros no longer defined (`#undef __GNUC_MINOR__`).
- **Include guards and `#pragma once`.** Each guarded file with its
  guard macro, and each once file, as file-table entries; at the end of
  the header, the header's own guard counts, as it would once included.
  In-memory buffers are left out (they cannot be included again).
- **`push_macro` stacks** that changed (an entry is a definition or
  "undefined") and **poisoned identifiers** with the poisoning
  pragma's range.
- **`__COUNTER__`**, the number of `#pragma clang diagnostic push`es not
  popped, and the attribute macros the header defined (WP-1065; their
  definition and end locations and token text).
- **The `#pragma clang|GCC diagnostic` state**: every mapping state the
  header made (diagnostic name, severity, the user/no-`-Werror`/no-fatal
  bits), the state in effect and the push stack (`kiln_diag`
  `PragmaSnapshot`); installed as the state at the point the record is
  loaded, so a diagnostic after it sees the header's mappings and
  `pop` returns to the state before.
- **Sema pragma state** (the inventory; written by the producer from
  `kiln_sema::PragmaState`, carried, and installed by a PCH's image
  consumer, WP-1036; not compared by `check`):
  the `#pragma pack` value and push stack (label, value), `#pragma
  ms_struct`, the `#pragma GCC visibility` stack (default, hidden,
  protected, or an attribute push, with its location) and the FP pragma
  override and `float_control(push)` stack (`kiln_sema::fp_bits`).
  The rest of the inventory:
  - `once`, `push_macro`/`pop_macro`, `GCC poison` and `clang|GCC
    diagnostic` are the preprocessor state above;
  - `message`, `warning`, `error`, `GCC system_header`, `GCC
    dependency`, `region`/`endregion` and `mark` act where they appear
    and leave no state;
  - `clang deprecated`, `final` and `restrict_expansion` keep no state
    yet (spec/pp.md deviation 3);
  - every other pragma reaches Sema as tokens. Sema state other than
    the stacks above, such as a pending `#pragma weak`, is carried with
    the declarations by WP-1036 only where the image carries it.

**Compact tokens; no lexing on load.** A body token is its kind (a tag
and a payload: the identifier's name index, the literal's encoding, the
punctuator's index, the character of an `other` token), its lexer and
preprocessor flags and its offsets in the definition's range. Nothing is
lexed when the record is read: a token's spelling is read from its file
only when something prints it (`-E`, `#`, `##`, a diagnostic), through a
lazy buffer (spec/source.md `add_lazy_file`).

**Layout** (little-endian `u32`s; a string is its length then UTF-8):

```
u32 VERSION (1)
file table and location records      §11.11 "File table" / "Location records"
u32 n, n names                       every identifier the record uses
u32 n, n definitions                 0 user | 1 builtin, see above
u32 n, n definition indexes          macros
u32 n, n name indexes                undefs
u32 n, n (name, u32 k, k definition indexes or u32::MAX)   push stacks
u32 n, n (name, lo, hi)              poisoned
u32 n, n (file index, name)          guards
u32 n, n file indexes                once
u32 counter, u32 diagnostic pushes
u32 n, n (name, def, end, tokens)    attribute macros
u32 n, n mapping states, u32 current, u32 n, n state indexes   diagnostics
u32 pack, u32 n, n (u8 has-label, [label], u32 value), u8 ms_struct
u32 n, n (u8 present, [u32 code, location])               visibility
u64 fp, u32 n, n u64                 FP override and stack
```

The file table's entry 0 is the header (the main file of the producer).
Unlike a declaration image's, the table also carries the predefined
buffers (`<built-in>`, `<command line>`) inline and files with `#line`
(a macro defined by `-D` or by a producer's `-include` keeps its
definition location). The reader is total: every truncation and byte
change decodes to an error or to a well-formed record
(`record_is_total`). Materializing ties the record to an importer's
source manager: a file it already has with the recorded length and hash
is reused, any other is added lazily; a missing file, or one whose size
changed, is an error, and a same-size edit is found when its text is
first read (the driver then reports the record unusable).

**In a precompiled header.** The section block of a kind-2 file
(§11.3: the count, each section's tag and bytes) followed by its
FNV-1a 64 checksum (`bmi::write_sections`), after the `KILNPCH` text
(spec/driver.md "Format").

**Header units.** A header unit's exported macros are written to a
record with the unit's file as entry 0, read back and materialized,
and the importer sees the materialized definitions (`end_unit`,
`unit_round_trip`); a unit the record cannot carry keeps its in-process
definitions. `ModuleOptions::no_unit_record` turns the round trip off.
The header-unit tests (`tests/conformance/module.import/`,
`header-unit*.cpp`) pass with it on.

**Tests.** `crates/kiln_pp/tests/pp_state.rs`: macros and undefs,
redefinition diagnostics pointing into the header, `push_macro` and
poison, `__COUNTER__`, the diagnostic stack, totality, and the pp
torture corpus (`tests/differential/pp/torture/*.cpp` each as the
header, every `Inputs/` header included again after it), each comparing
`-E`, `-dM -E` and diagnostics of replay with the installed record.
`crates/kiln_driver/tests/pch_image.rs` covers the driver.

**Measured** (harness build, aarch64, WP-1280; median of 5;
`-std=c++26`, the PCH built with `-fkiln-pch-image`, then `-E -P` of a
small TU with `-include-pch`):

- `<bits/stdc++.h>` (libstdc++ 14): a 465 KB record (549 KB PCH)
  for 4964 macros. Replay takes 0.23 s and 31 MB; loading the record
  takes 0.01 s and 8.9 MB (0.02 s with `-fno-validate-pch`).
- Every libc++ public header (106 `#include`s): a 542 KB record
  (679 KB PCH) for 4569 macros. Replay takes 0.24 s and 45 MB; the
  record, 0.02 s and 9.2 MB.
- For reference, clang++-19 builds its PCH of `<bits/stdc++.h>` in
  1.45 s and 282 MB. A `-fsyntax-only` with that PCH takes 0.32 s and
  162 MB, and `-E` of the header takes 0.13 s and 94 MB. clang's PCH
  carries the AST as well, so the numbers do not compare one-to-one.

The -E and -dM -E outputs after the header are identical, as are the
diagnostics. The `-fkiln-pch-image=check` mode is clean on both, in
`-E` and in `-fsyntax-only`.

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
  sources are in `Inputs/`, named by `module:` (and `unit:`) lines
  (spec/harness.md "Directives"). The `run` tests are two-step under
  kiln and clang alike (§11.7); the compile tests give kiln the sources
  (part A) and clang module files. All run under clang 19 and 22 as the
  compiler (WP-986; `visibility.cpp` and `module.reach/missing-import.cpp`
  are `requires: clang>=22`: clang 19 reports a module-linkage name as
  "must be imported" where clang 22 and kiln say "undeclared").
- `module.unit/single-invocation-run.cpp`: part A's single `kiln++`
  invocation over the module sources (kiln only, WP-986).
- Coverage note (WP-986): no module test is `skip(clang)` for a harness
  limit any more. The remaining `skip(clang)` module tests are
  deliberate deviations or kiln-only forms: `basic.lookup.argdep/
  module-enum*.cpp` (deviation 10), `basic.stc.thread/module-tls-run.cpp`
  (deviation 9), `module.global.frag/gmf-dup-run.cpp` (deviation 7),
  `module.import/import-std-vector.cpp` (kiln's implicit `std` lookup,
  §11.12), `single-invocation-run.cpp`, and
  `module.import/import-identifier-type.cpp` (clang treats a line-initial
  `import` identifier as a directive). `cpp.module/p3034-probe.cpp` is
  `xfail(clang)` (clang accepts macro module names).
- `module.global.frag/gmf-merge-importer.cpp` (+ `Inputs/gmi-*.cppm`):
  entities declared in both an importer's and an imported unit's GMF
  merge (§6, "Merging by entity"; WP-460), compared with clang.
- `module.global.frag/gmf-spec-merge.cpp` (+ `Inputs/gmf-spec-merge.cppm`,
  WP-1156): a class and a class template's explicit and partial
  specializations defined by an importer before its `import` and after
  it merge with the imported unit's GMF definitions.
  `gmf-floatn-limits.cpp` and `gmf-floatn-limits-run.cpp` (+
  `Inputs/gmf-floatn-*.cppm`): libstdc++'s `numeric_limits<_Float32>`/
  `<_Float64>` specializations across the module boundary, with `<math.h>`
  before and after `<limits>` on each side.
- `module.import/header-unit*.cpp`: header units (macros visible): kiln
  builds them in-process, clang through the harness's `header-unit:`
  lines (`-fmodule-header`, WP-986).
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
  - four racing misses, and (WP-1074) sixteen that build once, racers
    with one killed (SIGKILL) at three points, a miss waiting on a held
    entry lock and clearing a killed holder's temporary files, a stuck
    holder's waiter falling back to part A, and a hit removing a stale
    lock file (the lock tests skip where `flock` does not exclude, after
    trying the system's temporary directory);
  - an unwritable root;
  - (WP-1050) pruning: the timestamp created on first use, no scan
    within the interval, an unused entry removed, a young one, a locked
    one, a non-entry directory, a link's target and the compile's own
    entry kept, a hit refreshing its stamp and not its module file, and
    the flags (0 turns pruning off, short values, an invalid value,
    `--help`);
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
  importers), `module.unit/partitions-run.cpp` (interface and
  implementation partitions, an implementation unit),
  `module.unit/two-step-impl-unit-run.cpp`,
  `basic.start.dynamic/module-init-run.cpp` (observable
  initialization order, once only), `module.global.frag/gmf-dup-run.cpp`
  (kiln only: clang emits the GMF class's vtable in every importer,
  deviation 7), and `module.interface/two-step-visibility.cpp` (the
  visibility diagnostics through a module file). WP-986 folded the
  duplicate `two-step-partitions-run`, `two-step-module-init-run` and
  `two-step-gmf-dup-run` into the converted originals.
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
- Compilation databases and the full format (WP-1148, §11.9):
  `crates/kiln_driver/tests/scan_deps.rs` `compilation_database_p1689`
  (per-entry `directory`, `arguments` over `command`, `output`, rule
  order, an import naming another entry's source; the empty database),
  `compilation_database_make` (database order, a failing entry beside
  good ones, clang 19's header under `clang-scan-deps-19`),
  `compilation_database_errors` (clang's messages for an unreadable,
  malformed or ill-shaped database and for both `--` and a database),
  `experimental_full_shapes` (the 22 and 19 shapes) and
  `differential_against_clang_scan_deps`: one module graph across
  directories through kiln and the real clang-scan-deps 19 and 22
  (where installed), P1689, make and full compared after the documented
  deviations; `kiln_driver::scan_deps_cdb` unit tests cover the database
  parser and shell splitting.
- CMake (WP-982): `scripts/oss/selftest.sh` `cmake_modules` builds
  `scripts/oss/fixtures/modules` (§11.9). WP-1148: the OSS harness rows
  `cmake_modules` (that fixture, `import std` under libc++) and
  `fmt_module` ({fmt} 12.1.0 with `FMT_MODULE=ON` and an importer) build
  with Ninja through kiln-scan-deps against a clang 19 control
  (docs/status/oss/{cmake_modules,fmt_module}.md);
  `module.global.frag/gmf-floatn-limits.cpp` reduces the `fmt_module`
  libstdc++ failure, fixed by WP-1156 (§6).
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
  checksum give a diagnostic or a fallback evaluation, never a panic),
  `kind2_carries_pointer_and_reflection_values` (WP-1047: the importer
  of `module.interface/Inputs/cv.cppm` and `cr.cppm` re-evaluates none of
  their constant evaluations under kind 2 but the failed trial
  evaluations, and its IR is the same under both kinds; WP-1160 adds
  `cl.cppm` and `cx.cppm`, the `-v` reason beside a kind-2 count, and
  16 damaged sections of each behind a valid checksum);
  `crates/kiln_sema/src/bmi_ast.rs` `wire_*` and `fuzz_wire_values`
  (every value of tags 5-10, the version-4 pointer bases and reflection
  forms, and a key holding pointer and reflection arguments round-trip;
  their truncations, flips, out-of-range paths, flags and overlong
  varints are rejected without a panic); the `-run` tests
  `module.interface/carried-pointers-run.cpp` and
  `carried-reflections-run.cpp` (address and specialization identity
  across the two units, `&X<&g>::x`, `&Wrap<^^S>::x`, template parameter
  objects), `carried-literals-run.cpp` (string literals, temporaries and
  labels, WP-1160) and `carried-reflections-more-run.cpp` (alias template
  specializations, data member specifications and annotations, WP-1160);
  and every `module:` test under `cargo xtask test --bmi-kind 1` and
  `--bmi-kind 2` (spec/harness.md "Module-file kinds"), both in `ci`.
