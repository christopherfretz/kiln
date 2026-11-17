# Changelog

All notable changes to kiln are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and kiln adheres to [Semantic Versioning](https://semver.org/). What a
version number promises is in [docs/STABILITY.md](docs/STABILITY.md).

## [Unreleased]

Planned as 1.3.0.

### Added

- `-fmason-metadata=all` names every vague definition row: its `name`
  is the symbol, so a linker can name a definition the optimizer deleted
  (WP-1586). The other levels are unchanged byte for byte.
  `scripts/mason/odr_corpus.py` builds the must-equal and must-differ
  corpus of definition hashes (`tests/mason/corpus/`, 51 cases) and its
  `manifest.tsv` for a linker's test lane
  ([spec/harness.md](spec/harness.md) "The O-13 corpus").

- `-fmason-metadata-system-unit` (WP-1619): with `-fmason-metadata`,
  marks every definition and type row of the unit's `.mason.meta` section
  as coming from a system header, for building a standard library such as
  libc++ and libc++abi from its sources, so the mortar linker treats those
  rows as it treats the library's own. Hashes, keys, the other flags and
  diagnostics are unchanged; a PCH or module file serves units with and
  without it.

- `cargo xtask gxx-dg` (WP-1443): GCC 16.1's C++ testsuite (`g++.dg`
  with `g++.dg/torture`, `c-c++-common`, `g++.old-deja`) through kiln++,
  with GCC and clang as oracles. It reads the `dg-` directives (errors and
  warnings by line, target and xfail selectors, `c++NN` standards as
  `g++-dg.exp` picks them) and sorts failures into crash, wrong-code,
  rejects-valid, accepts-invalid and wrong-line, each split by what clang
  does. The first triaged run is `docs/status/gxx-dg.md`: kiln passes
  47950 of 56766 cases. `scripts/fetch-gcc-tests.sh` fetches the C++ suites.
- `-fstack-clash-protection` and `-mstack-probe-size=` take effect
  (WP-1411): the function attributes and the `probe-stack` and
  `stack-probe-size` module flags are written as clang 19 writes them,
  and `-fno-stack-clash-protection` undoes it. The flag was accepted
  with no effect before.
- `-grecord-gcc-switches` and `-grecord-command-line`, with their
  `-gno-` forms, write the command line into the compile unit's debug
  info (WP-1411); the arguments are recorded as given.
- `-mtls-dialect=` follows clang (WP-1411): x86 accepts `gnu` and
  `gnu2`, where `gnu2` enables TLS descriptors, and AArch64 rejects the
  option.
- `-moutline` and `-mno-outline` turn LLVM's machine outliner on or off
  at `-O1` and above, as clang 19 (WP-1422); on x86-64 `-moutline` is
  clang's `-Woption-ignored` warning.
- `-ftls-model=global-dynamic|local-dynamic|initial-exec|local-exec`
  sets the TLS model of thread-local variables without a `tls_model`
  attribute (WP-1422); an invalid value is clang's error.
- `-ggnu-pubnames`, `-gno-gnu-pubnames`, `-gpubnames` and
  `-gno-pubnames` set the compile unit's name-table kind as clang 19
  does, and `-gsplit-dwarf` now gives GNU pubnames as clang's (WP-1422).
  `-gpubnames` and `-ggnu-pubnames` were rejected before.
- `-mlarge-data-threshold=` is accepted on x86-64 (WP-1419): with
  `-mcmodel=medium` or `large` it sets the `Large Data Threshold` module
  flag, with another model it is clang's
  `-Wunused-command-line-argument` warning, and on AArch64 it is an
  unsupported option, as in clang.
- `-fmerge-all-constants` and `-fno-merge-all-constants` are accepted
  (WP-1417). They have no effect: a constant `const` local array or
  class is still copied to the stack, where clang makes it a static.
- Every AArch64 NEON and fp16 builtin that kiln's `arm_neon.h` and
  `arm_fp16.h` reach now compiles, lowered as clang 19 lowers it
  (WP-1421). These are the multi-register and lane loads and stores,
  the compares, the scalar and saturating forms, the conversions, the
  SHA3/SHA512/SM crypto builtins and `vldap1`/`vstl1`. Before, 628
  builtins stopped at "not supported yet". In a census of one call per
  intrinsic, the count that compile went from 3094 to 4426 of 4884
  (docs/status/neon-builtins.md). `__has_builtin` now answers 1 for
  them.
- The GNU unwinder builtins, in C and C++ (WP-1428):
  `__builtin_dwarf_cfa`, `__builtin_unwind_init`, `__builtin_eh_return`,
  `__builtin_eh_return_data_regno` (a constant expression),
  `__builtin_dwarf_sp_column`, `__builtin_extend_pointer` and, on
  AArch64, `__builtin_sponentry`, lowered as clang lowers them. They were
  undeclared identifiers before.
- The AArch64 SME keyword attributes (WP-1426): `__arm_streaming`,
  `__arm_streaming_compatible`, `__arm_in`, `__arm_out`, `__arm_inout`,
  `__arm_preserves`, `__arm_locally_streaming`, `__arm_new` and, at
  `-fclang-compat=20` and later, `__arm_agnostic`. The type keywords are
  part of the function type (conversions, redeclarations, overriders,
  clang 20 mangling), every keyword gets clang 19's IR attributes and
  diagnostics, and SVE types may be used in a streaming function with
  `sme`. The SME intrinsics of `<arm_sme.h>` are not provided yet.
- clang's `-Wgcc-compat` warning for a GNU attribute GCC knows written
  after the declarator of a function definition (WP-1426).
- More debug info under `-g`, matching clang 19 (WP-1445): structured
  bindings are debugger variables (clang's `DIExpression` on the
  decomposed object), a variable-length array gets its variable and a
  `DISubrange` counted by `__vla_exprN`, labels get `DILabel` and
  `#dbg_label`, and the generated initializer functions
  (`__cxx_global_var_init*`, `_GLOBAL__sub_I_*`,
  `__cxx_global_array_dtor`, `__tls_init`) get artificial subprograms,
  so a breakpoint in a dynamic initializer works.
- The link-mode options `-nodefaultlibs`, `-nostartfiles`, `-nolibc`,
  `-nostdlib++`, `-static-pie`, `-static-libgcc`, `-static-libstdc++`,
  `-shared-libgcc`, `-r`, `-z <arg>`, `-u <sym>` and `-T <script>` are
  accepted by `kiln` and `kiln++` and reach the link step as in clang, in
  command-line order (WP-1453); they were unknown arguments before (found
  building ICU4C, which links a library with `-nodefaultlibs -nostdlib
  -shared`). A compile-only run warns about them as clang does.
- `-fcontracts` and `-fno-contracts`, as in GCC 16.1 (WP-1463):
  contracts (`pre`, `post`, `__cpp_contracts`) before C++26, or none at
  C++26. With libstdc++, either flag also links `libstdc++exp` (the
  default `handle_contract_violation` of libstdc++ 16) when the GCC
  installation has it, as `g++` does. GCC's
  `-fcontracts-client-check=none` and `-fcontracts-definition-check=on`
  are accepted; its other contract options are diagnosed as unsupported.
- `-fshort-enums` and `-fno-short-enums` (WP-1461): every enumeration
  without a fixed underlying type takes the smallest type that holds its
  values, as with `__attribute__((packed))`, in C and C++, as GCC and
  clang (`-fshort-enums` was rejected before).
- `-fno-kiln-debug-source-checksum` (WP-1488): under `-g`, no source
  file's MD5 checksum goes into the DWARF 5 line table, so an edit that
  moves no line (a comment in a header) leaves the objects byte-identical
  for incremental links with mortar; a debugger can then no longer detect
  a stale source. The default, `-fkiln-debug-source-checksum`, is unchanged.
- GCC's `__builtin_is_corresponding_member` and
  `__builtin_is_pointer_interconvertible_with_class` (WP-1472), so
  libstdc++'s `std::is_corresponding_member`,
  `std::is_pointer_interconvertible_with_class` and their feature-test
  macros are available, in constant expressions and at run time. They
  share one common-initial-sequence rule with `__is_layout_compatible`,
  which now also accepts members that are both `[[no_unique_address]]`
  (CWG2759, as GCC; clang answers false).
- GCC 16.1's contract flags (WP-1485): `-fcontracts-client-check=none|pre|all`
  makes the caller check the preconditions (and with `all` the
  postconditions) of each function it calls directly, and
  `-fcontracts-definition-check=off` drops a function's own checks of its
  preconditions and postconditions; `contract_assert` is unaffected
  ([spec/contracts.md](spec/contracts.md) §6.1). No ABI change.
- `-fcheck-new`, `-faligned-new[=N]`, `-fno-aligned-new` and
  `-fnew-alignment=N` (WP-1485), as clang and GCC: a null check after
  each allocation, and the threshold above which `new` calls the
  aligned allocation functions. The default output is unchanged.
- GCC's `__builtin_exit`, `__builtin__Exit` and `__builtin__exit` (WP-1485),
  calls of the library functions; never constant expressions.
- The [meta.syn] metafunctions `std::meta::is_applicable_type`,
  `is_nothrow_applicable_type` and `apply_result` (WP-1505, P1317), for a
  callable type and a tuple-like type
  ([spec/reflection-traits.md](spec/reflection-traits.md) §1.4).
- `-fmason-metadata[=min|default|detail|all]` and `-fno-mason-metadata`
  (WP-1557; kiln only, off by default): each object gets a `.mason.meta`
  section of metadata for the mortar linker's link-time checks, today a
  summary of the translation unit's ABI-affecting options. Other linkers
  drop the section; output without the flag is unchanged
  ([spec/codegen.md](spec/codegen.md) §5.5).
- `-fmason-metadata` (from level `default` up) also records the type of
  every external function and variable an object defines or uses, so the
  mortar linker can check at link time that the objects agree on it
  (WP-1564); a C-linkage entity's type is written in one canonical form
  that a C object and a C++ object share (WP-1565, after mortar's rulings:
  `int` and `unsigned int` alike, no record for C++ functions renamed by
  an asm label, C++ function types as hashes only at `default`), and
  `abi.stdlib-abi` now lists libc++'s layout-changing ABI switches
  ([spec/codegen.md](spec/codegen.md) §5.5).
- `-fmason-metadata` (from level `default` up, C++) also records a hash
  of the layout of every class an object uses (size, alignment, bases,
  members, virtual functions), so the mortar linker can report two
  objects that disagree on a class's layout (WP-1566), and of every
  enumeration an object refers to (underlying type, enumerators and
  their values; WP-1576); classes and enumerations that are local to one
  translation unit get no record. At `detail` and `all` it also records
  each class's and enumeration's members, bases, vtable slots and
  enumerators, so the linker can name the first difference (WP-1576).
  A C-linkage type no longer keeps the outer bound of an array behind a
  pointer, so `int (*)[]` and `int (*)[4]`, which C treats as
  compatible, are not reported as a mismatch (WP-1576)
  ([spec/codegen.md](spec/codegen.md) §5.5).
- `-fmason-metadata` (every level, C++) also records a row for each
  definition that may appear in several objects and that the object
  defines (inline functions and variables, template instantiations,
  explicit specializations), with a hash of its type for now, so the
  mortar linker can report an explicit specialization declared after an
  implicit instantiation elsewhere; `scripts/mason/odr_oracle.py` checks
  such objects without the linker. The unit summary names the layout
  hash scheme in `odr.layout-scheme`; `odr.scheme` now names the
  definition hash. A C-linkage function whose type has a variable array
  bound no longer gets a declaration record, and at `detail` a member of
  unnamed class type records how many entries describe it (WP-1581)
  ([spec/codegen.md](spec/codegen.md) §5.5). The hash now covers the
  whole definition: its declaration, body, constructor initializers,
  initializers of variables (as written, never their value), default
  arguments and default member initializers where they are used, and
  `new`, `delete`, `throw` and traits (WP-1582, WP-1583), lambdas
  (numbered inside the enclosing definition, with their captures, call
  operator and body), local classes and enumerations, coroutines,
  statement expressions, `asm`, label addresses, `_Generic`,
  `__builtin_choose_expr`, `offsetof` and the GNU vector and atomic
  forms (WP-1584; scheme `kiln-odr-1.d3`). A closure's call operator row
  is the closure's hash inside its enclosing definition; a function with
  a lambda in its signature, and the member functions of a local class,
  get no row. A function template specialization the definition names
  is hashed by its template, signature and template arguments rather
  than by its symbol, so two units that spell an equivalent signature
  differently agree (fmt's tests); the definition's default arguments,
  `aligned`, `noreturn`, `target`, `target_version`, `indeterminate` and
  `[[assume]]` are hashed; abi tags, visibility and
  `exclude_from_explicit_instantiation` of the entities a definition
  names are not, so a library built with its hiding macros empty
  agrees with its users; a row that uses an internal-linkage entity or
  a type local to its unit is marked `USES_INTERNAL` (WP-1585; scheme
  `kiln-odr-1.d4`). The `noexcept` of a member template of a class
  template specialization not instantiated yet, requires-expressions,
  parenthesized initializer lists in templates and the arguments of
  `target_clones` are hashed, and a function template specialization is
  hashed by its short form only beside an overload whose template
  parameters and arity could also match (WP-1626; scheme
  `kiln-odr-1.d5`). A row is marked partial only when the definition
  holds a construct not hashed yet (some dependent types, reflection):
  under 0.1 % of the rows of fmt and googletest
  ([spec/abi.md](spec/abi.md) "Definition hash (`odr_def`)"). The unit
  summary also records `abi.wchar-sign` (`signed` or `unsigned`) beside
  `abi.wchar`, and an enumeration's underlying-type record says whether
  the enumeration is scoped and whether its underlying type is fixed
  (WP-1608). With `KILN_STATS_LOG`, each job's line counts the
  definition rows, those of user code, and those of user code that name
  a standard library entity (`mason_defs`, WP-1627;
  [spec/driver.md](spec/driver.md)). With `-fmason-metadata-system-unit` the
  unit summary also says `unit.system` = `yes` (WP-1639); the summary's
  key space is open: readers skip keys they do not know unless marked
  `ENFORCE`.

### Changed

- The `.mason.meta` definition hash's scheme is `kiln-odr-1` (WP-1586):
  `kiln-odr-1.d5` frozen and renamed, no digest changed. Objects written
  under a `kiln-odr-1.dN` scheme are incomparable with `kiln-odr-1` ones;
  any later change to the hash is `kiln-odr-2`. The known misses and
  exclusions are one table, each row with the test that pins it
  ([spec/abi.md](spec/abi.md) "Misses and exclusions").

- The PCH snapshot format changed several times since 1.2.0 (new
  diagnostics, options and AST data changed the schema; the current
  version is in [spec/pch.md](spec/pch.md)). Rebuild a PCH built by
  kiln 1.2.0; until then it is not misread, as the spec describes.
- `__is_trivially_copyable`, `__is_trivial`, `__is_pod` and
  `__builtin_is_implicit_lifetime` (and with them
  `std::is_trivially_copyable`, `std::is_trivial`, `__builtin_bit_cast`,
  the atomic builtins and `std::meta::is_trivially_copyable_type`) follow
  [class.prop]/1-2 (WP-1452): a deleted or ineligible special member does
  not count, so a class whose copy and move operations are all deleted,
  or whose destructor is deleted, is not trivially copyable, and one with
  no eligible default constructor is not trivial. clang 19/22 and GCC 16.1
  each differ from the standard on some of these classes
  (docs/status/deviations.md). Argument passing, debug info and the
  `__has_trivial_*` traits are unchanged.

- Default change: at C++26 and later (`-std=c++26`, `gnu++26`, `c++2c`,
  `gnu++2c`) kiln++ links `libstdc++exp` when it links against
  libstdc++ and the GCC installation has `libstdc++exp.a` (WP-1485),
  without `-fcontracts`. The archive holds the parts of libstdc++ whose
  ABI is not frozen (the default contract-violation handler,
  `<stacktrace>`, `std::text_encoding::environment`, `<debugging>`, the
  Filesystem TS); only referenced members are linked, so other programs
  link to the same binary. g++ 16.1 does not add it at a plain
  `-std=c++26`. `-nostdlib++`, or an installation without the archive,
  leaves the link line as before; below C++26 it is still added only
  with `-f[no-]contracts`.

- `-fno-elide-constructors` is accepted before C++17 (WP-1503): the
  copies C++98 to C++14 allow to be elided are now built, and the flag
  makes them, as clang does. By default they are still elided.
- The g++.dg runner (`cargo xtask gxx-dg`) reads the severity a
  `dg-message` or `dg-regexp` pattern names, so expected errors written
  that way are no longer reported as rejects-valid, and a line-0
  `dg-error` accepts an error on any line; a case whose errors only a
  `dg-message ""`, `dg-prune-output` or `dg-excess-errors` could account
  for is counted apart as `expect-unknown` (WP-1554). The standard
  citations in docs/status/gxx-dg.md were checked against the working draft
  (WP-1556). Every CI job now has a `timeout-minutes`, so a hung runner no
  longer holds the merge queue for six hours (WP-1600).

### Fixed

- `.mason.meta`: three false ODR differences reported by mortar's
  maintainers. A definition without listed attributes no longer hashes
  differently in a unit that has such an attribute anywhere (an
  `alignas` type, a `[[noreturn]]` declaration, `<stddef.h>`), so an
  inline function's row agrees across units (WP-1636); `[[noreturn]]`,
  `[[gnu::noreturn]]` and `__attribute__((noreturn))` hash alike
  (WP-1637); a lambda of an inline function named under a function type
  or template argument (libc++'s `std::visit` tables) no longer marks the
  row `USES_INTERNAL` (WP-1638). Scheme `kiln-odr-1.d5`
  ([spec/abi.md](spec/abi.md) "Definition hash (`odr_def`)").

- Codegen, found building libc++abi and libc++ 22.1.8 with kiln (WP-1620):
  the vtable of a `__cxxabiv1` `type_info` class that RTTI referred to
  before the class's own vtable was emitted is one global again (it was
  defined as `_ZTV...E.N` and left undefined under its own name); the key
  function of `__cxxabiv1::__fundamental_type_info` defines the
  fundamental `type_info` objects (`_ZTIv`, `_ZTIPKc`, ...) and their
  names, as clang and GCC do; a function first declared as a friend of a
  class under `extern template` and defined outside the class is emitted
  (libc++'s `std::__get_ostream_file`).
- Templates and concepts, found running GCC's C++ testsuite (WP-1570,
  WP-1578, WP-1598, WP-1599): a member call through `this` of the current
  instantiation naming one non-template member function with a
  non-dependent type, in a class with no dependent base, is no longer
  type-dependent ([temp.dep.expr]/5), so `auto b = this->f(1);
  b.g<int>()` needs no `template`. Before C++17 the temporaries of a
  dependent template argument no longer leak into the next
  full-expression. A concept-id with a non-type parameter whose argument
  fails is judged atom by atom ([temp.constr.atomic]/3), and a
  requires-expression under a pack expansion defers its later
  requirements ([expr.prim.req.general]/5).

- Expansion statements and related checks, found running GCC's C++
  testsuite (WP-1483): an iterating `template for` follows the working
  draft's [stmt.expand]/5.2. Its range and iterator are `constexpr`
  exactly when the loop variable is, so a non-`constexpr` loop over a
  run-time object sees the object itself. An iterator without `-` is
  rejected, and the first error in the range stops the statement. An
  array placement new into an inactive union array member makes that
  member active. A pack index of class type is converted to `size_t`. A
  name-independent `_` may follow a member function, a parameter or a
  function named `_`. A member `get` that cannot be called is reported
  once per structured binding declaration. In C23 and C++26, extra
  `va_start` arguments, discarded as before, now warn `-Wvarargs` unless
  they name the last parameter, as in GCC. `cargo xtask gxx-dg` lifts
  kiln's and clang's error limit.
- Found running GCC's C++ testsuite (WP-1485, WP-1487): a postcondition
  of a templated function with a deduced return type is accepted
  ([dcl.contract.res]/2); the ambiguous derived-to-base conversion error
  is reported at the operand and names the types as clang does; a
  braced list in a function-like macro argument is diagnosed with
  clang's notes and recovered from.
- Reflection, found running GCC's C++ testsuite (WP-1482): a range
  argument of `reflect_constant_array`, `define_static_array`,
  `substitute` and the other range-taking metafunctions is iterated once
  (`views::transform`, `views::filter`, a fixed-extent `span`, input
  ranges, a user's container), where it was read from its memory layout
  and rejected; the standard contiguous classes are still read directly.
  `extract` reads an automatic variable whose lifetime began in the
  evaluation and rejects one whose frame has returned.
  `access_context::current()` in a class template's member initializer
  names the specialization. A member of a namespace-scope anonymous union
  is a class member, and `members_of` of the namespace lists the union's
  variable. `data_member_spec` rejects built-in trait names.
- Reflection follow-ups, found running GCC's C++ testsuite (WP-1489): an
  iterated range argument whose elements are `bool` (`vector<bool>`) or
  arrays is read as its value type. Member template splices
  `x.template [:r:]` and `x.template [:r:]<args>`, splice type specifiers
  in base specifiers, mem-initializers and using-enum declarators, and a
  splice of a class or alias template used as a deduced class type are
  accepted. A splice operand of class type converts to
  `std::meta::info`. `^^C<args>::m` parses. `members_of` skips consteval
  blocks and lists the variables behind a tuple-like structured binding.
  `^^` of an anonymous union member and the union's member are the same
  reflection. `has_identifier` of a parameter depends on the evaluation
  point. `std::is_structural_type` works with GCC's `<meta>`.
- Temporary lifetime found running GCC's C++ testsuite (WP-1459): a
  reference bound through `.*` with a pointer to data member, a
  base-to-derived or dynamic cast, or a reversed subscript now extends the
  temporary; a conditional whose arms are temporaries constructs, extends
  and destroys only the chosen one, in constant evaluation too; the
  temporaries of a tuple-like structured binding's initializer live until
  its last `get` call (CWG2867, as GCC; clang 19/22 destroy them first).
  `-frange-for-ext-temps` and `-fno-range-for-ext-temps` (GCC's) turn
  P2718R0's range-for extension on before C++23 or off from it, and
  `__cpp_range_based_for` follows them.
- Template argument deduction found running GCC's C++ testsuite
  (WP-1465): of several base classes a call argument's class could
  deduce from, the most derived wins (CWG2303); an implicit deduction
  guide checks access as its class does, so a constructor naming a
  private member type deduces ([over.match.class.deduct]); taking the
  address of, or deducing `auto` from, an overload set drops members
  whose constraints are not satisfied and keeps the more constrained
  (CWG2918); `auto` deduces through array declarators (`auto (*p)[1] =
  &v;`, CWG2397); `template<class T, T n> void f(int (&)[n])` deduces
  `T` as `std::size_t` from the bound ([temp.deduct.type]/20).
- C++26 forms found running GCC's C++ testsuite (WP-1464): the C23
  one-argument `va_start(ap)` and `__STDC_VERSION_STDARG_H__` at C++26
  (P3348R4); an array placement new at the first element of an array
  object (`new (a) int[3]` for `int a[3]`, `std::allocator` storage) is a
  constant expression (P2747R2; clang 22 rejects it); an iterating
  expansion statement whose range is not a constant but whose size is
  (`std::span<int, 3>`, a member `std::array`) is evaluated at run time
  (CWG3140); a `constexpr` tuple-like structured binding whose `get` is
  not `constexpr` is an error (P2686R5; clang 22 accepts it); GNU vector
  and `_Complex` types decompose in structured bindings, as in clang.
- Crashes and hangs found running GCC's C++ testsuite (WP-1456): an
  array type larger than the target's maximum object size is "array is
  too large" (it ran out of memory); a register `asm` output of an
  aggregate that is not register sized is "impossible constraint in asm"
  (the backend aborted); `__int128` to `__bf16` on AArch64 calls libgcc
  (LLVM aborted); a recursive constraint satisfaction check stops at the
  template depth limit; `this` in a default member initializer of a
  lifetime-extended `constexpr` temporary no longer overflows the stack;
  a non-`int` `main` under `-ffreestanding` is an ordinary function; and
  a pointer to a member template in a variable template no longer
  recurses without bound.
- Reflection (WP-1462): a default-initialized `std::meta::info` is the
  null reflection and a constant (`constexpr info r;`, arrays, members,
  `new info`, through PCH and modules); metafunctions can be called
  through a pointer or reference (`views::filter(is_template)`);
  `current_class`, `current_function` and `access_context::current` in a
  consteval block and in a default member initializer used by aggregate
  initialization answer for the right scope; `size_of`, `alignment_of`,
  `bit_size_of`, `is_bit_field` and `identifier_of` accept data member
  descriptions; `reflect_constant_string` keeps the terminator of a
  `string_view` that covers it; `extract` reads through a
  constant-initialized reference. GCC's `g++.dg/reflect` goes from 306 to
  384 of 565 cases passing.
- A regression from 1.1.0, present in 1.2.0 and 1.2.1 (WP-1455): an
  out-of-line constructor whose parameter names an enclosing class of
  its own class, `O::I::R::R(I::R &&) {}`, was rejected with "use of
  undeclared identifier 'I'" and, for a protected or private `I`, an
  access error. The names after the constructor's `(` are now looked
  up in the scope of its class, as the parameters are. Found building
  LLVM's DWARFLinker.
- `-fkiln-backend=hod` (WP-1477): when hod failed on a module and kiln
  fell back to `llc`, a plain `-Werror` turned the fallback's warning
  into an error and failed the build. The warning is now
  `-Whod-fallback`, which a plain `-Werror` leaves a warning;
  `-Werror=hod-fallback` makes it an error, and `KILN_HOD_STRICT=1`
  still does.
- Debug info (WP-1445): an explicitly specialized member function's
  declaration is at the specialization, and a class with a const member
  of class type is `DIFlagNonTrivial`, as in clang.
- A precompiled header is byte-reproducible (WP-1415): two builds of
  the same header with the same kiln, command line and working
  directory, over files with the same modification times, now write the
  same bytes. Before, hash tables in the snapshot were written in a
  per-process order. spec/pch.md "Reproducibility" lists the bytes that
  depend on the invocation (the working directory, each input's path,
  size and modification time, the identity).
- A by-copy lambda capture of a variable of incomplete or abstract class
  type is clang's error (WP-1426); kiln did not diagnose it before. An
  explicit capture `[v]` of an SVE variable is checked against the
  target's features at the capture, as clang.
- C++: a typedef or alias-declaration of a class's or enumeration's name
  in its scope that names another type (`struct t1 {}; typedef long t1;`)
  is now clang's "typedef redefinition with different types", in
  namespace, class and block scope and through a using-declaration; the
  name keeps denoting the class. An alias template's conflicts are
  reported at its `using`, as clang does. After "typedef 'X' cannot be
  referenced with the 'struct' specifier" no incomplete-type errors follow
  (WP-1435).
- C: before C23, `-pedantic` now diagnoses converting, comparing or
  choosing with `?:` between pointers to differently qualified arrays
  (`int (*)[3]` and `const int (*)[3]`), as GCC does
  (`-Wc23-array-pointer-qualifiers`; clang is silent). Integer constant
  expression errors point at the first non-constant operand, without
  notes, as clang's do (WP-1435).
- `-mcmodel=` is checked against the target and the relocation model as
  clang's driver checks it (WP-1419). On AArch64, `-mcmodel=large` with
  the default PIE, `-fpic`, `-fPIE` or `-static` is now clang's error
  "invalid argument '-mcmodel=large' only allowed with '-fno-pic'";
  before, kiln wrote the large model beside PIC and the backend quietly
  emitted small-model PIC code. `kernel` and `medium` on AArch64, `tiny`
  on x86-64 and other values are "unsupported argument ... for target".
  x86-64 `large` with PIE is still accepted.
- Debug info: a function declared through a function typedef, an alias
  or `decltype` and named by a using-declaration without a definition
  in the unit (`namespace N { typedef int Fn(int); Fn h; } using N::h;`)
  no longer loses the whole module's debug information to LLVM's
  "invalid subroutine type" (WP-1434); every subprogram's type reads
  through the sugar, `noreturn` carried by the typedef included.
- Debug info: a function type with a non-default calling convention
  (`ms_abi`, `preserve_most`, `aarch64_vector_pcs`, ...) carries
  `DW_AT_calling_convention` as clang 19 writes it (WP-1434).
- A lambda's static invoker marks its call of a `noreturn` call operator
  `noreturn` and ends there, and a function whose type is `noreturn`
  ends in `unreachable` instead of `ret`, as clang emits them
  (WP-1434).
- C: a struct, union or enum tag and a typedef of the same name in one
  scope no longer conflict (WP-1427). Tags and ordinary identifiers are
  separate name spaces (C17 6.2.3), so `typedef enum {...} state;
  struct state {...};` compiles in either order, and inside the struct
  the bare name is the typedef. C++ still reports the conflict.
- C: a pointer to an array of `T` converts to a pointer to an array of
  `const T` without a diagnostic (WP-1427), as clang 19 and C23 6.7.3
  allow. Before, kiln reported "incompatible pointer types" (an error
  at the clang 22 level). The reverse conversion is now reported as
  "discards qualifiers", comparing the two pointers no longer warns, and
  `?:` gives the qualified pointer type where clang warns "pointer type
  mismatch".
- `#pragma clang attribute push(..., apply_to = function)` now reaches
  friend functions defined or declared in a class (hidden friends of
  class templates too), lambdas' call operators, explicit
  specializations of function and variable templates, deduction guides,
  and structured binding and exception variables, as in clang (WP-1417).
  A pushed `target("avx2")` over a SIMD wrapper's hidden friend
  operators no longer fails with an `always_inline` target-feature
  error, and that error now names an operator or lambda caller
  (`'operator=='`, `'operator()'`) instead of printing an empty name.
- On AArch64, a function whose `target` attribute, written or pushed,
  enables SVE (`sve`, `+sve`, `sve2`, `arch=armv9-a`, an SVE CPU) may use
  the SVE types in its signature and body without `+sve` on the command
  line (WP-1417). A lambda inside such a function gets the features of
  its own call operator, as in clang. With `sme` but no `sve` the error
  is clang's non-streaming one. A by-copy capture of an SVE variable is
  now the clang error instead of a crash in code generation.
- With `-fkiln-backend=hod`, the checks that send a unit to `llc`
  instead (function-entry instrumentation, `alwaysinline`, coroutines)
  read the function attributes themselves, not the module text
  (WP-1411): a unit that only mentions such a name in a string constant
  or a quoted symbol name is no longer sent to `llc` for it.
- An explicit instantiation of a class template no longer instantiates
  members whose trailing `requires` clause the arguments do not satisfy
  ([temp.explicit]/10; WP-1422), also in nested classes and with
  `extern template`; the emitted symbols are clang 19's. Found building
  an open-source linker.
- C: an `_Atomic` object takes the initializers, assigned values and
  arguments that its non-atomic type takes (WP-1422): `_Atomic(int *)`
  from `NULL`, `(void *)0`, `0` or `nullptr`, `_Atomic(struct S)` from a
  `struct S`, in braced, designated, nested and compound-literal
  initializers. An `_Atomic` structure is read by its value when it
  initializes, is assigned, passed, returned or cast. A C structure with
  an `_Atomic` member has clang's layout, and atomic accesses use the
  atomic type's alignment. Found building an open-source allocator.
- An explicit instantiation definition that is the first declaration of
  a namespace block (`namespace n { template class X<int>; }`, typically
  after `extern template class X<int>;`), or the first declaration after
  a namespace (`namespace a { ... } template int X<int>::f();`), defines
  each member function once (WP-1425). Each such function was built with
  two copies of its body, each with its own debug subprogram, so at `-g`
  LLVM rejected the debug info and dropped it for the whole object file.
  The second copy could not run, so the generated code was not wrong.
- A member declaration whose declarator is a parenthesized name after the
  class name is no longer read as a constructor (WP-1423): `friend S
  (::g)(int);`, `friend constexpr S (::operator""_s)(const char *,
  std::size_t);`, `S (N::g)(int);`, `S (S::*pm)(int);` and `S (f)(int);`
  in class `S` declare `::g`, the literal operator, the member pointer
  and the member function, as in clang and GCC. Before, such a friend was
  rejected, so it did not grant access. `S ((int));` is now clang's error.
- At `-g`, a class-type template argument reached only through a local
  variable's debug record (`auto w = nf<n::ic<bool, false>{}>();`) no
  longer makes the backend reject the module with "invalid type for
  undef constant" (WP-1424): the class's IR type is now defined in the
  module. libc++'s `std::not_fn<f>` tests failed to compile with `-g`.
- `offsetof` with an array index that is not a constant
  (`offsetof(struct flex, data[count])`) compiles in C and C++, as a GNU
  extension clang and GCC accept (WP-1433): the value is computed at run
  time as clang computes it, and the expression is not a constant
  expression. It was rejected as not supported before, and so was a
  constant `offsetof` whose negative index makes the offset wrap.
- Debug info: template specialization names spell value arguments as
  clang does (WP-1414): only the value's own class is named
  (`T<Out{{2}, 2.500000e-01}>`), a floating-point argument is
  `1.500000e+00` (was `1.5e0`), `INF` or `nan`, and a lambda is
  `(lambda at file:line:col)`. Diagnostics name such arguments the same
  way.
- Debug info: `defaulted: true` marks every template argument equal to
  its default, not only a trailing run, and the arguments of an explicit
  specialization (WP-1414); an alias template specialization keeps its
  typedef (`V<int>`); a member pointer keeps its pointee's typedefs; a
  generic lambda's `__invoke<T>` is artificial; a template parameter
  object used in a function template no longer adds its class to the
  retained types (WP-1414).
- A member function declared `__attribute__((used))` (or `[[gnu::used]]`)
  in a class template is instantiated and emitted when the class is
  implicitly instantiated, as clang does (WP-1430): constructors,
  destructors and conversion functions too, `linkonce_odr` and listed in
  `@llvm.compiler.used`. An ill-formed body is now diagnosed at the
  class's point of instantiation. Before, such members were emitted only
  when called, and an in-class `used` definition in a non-template class,
  or a `used` constructor or destructor, was not emitted at all.
- Explicit instantiation definitions written inside a namespace,
  `extern "C++"` or `export` block are emitted before the block's other
  declarations, as clang emits them (WP-1430); so is a static data
  member's definition at an explicit instantiation of the member or of
  its class (it was emitted with its template's namespace). The vtables
  of explicitly instantiated classes and of classes with a key function
  are emitted in one source order. Only the order of definitions in the
  output changes.
- C++: an explicit instantiation whose name is qualified must appear in a
  namespace that encloses its template ([expr.prim.id.qual]/3), as clang
  checks it: "explicit instantiation of 'E' not in a namespace enclosing
  'N'", or "must occur at global scope"; before C++23 an unqualified one
  must be in the template's namespace (WP-1448). kiln accepted these
  before. From C++23 an unqualified one is accepted (the rule was
  removed; docs/status/deviations.md).
- C++: a typedef-name declared after a using-declaration that named a
  typedef-name is checked against it: another type is "typedef
  redefinition with different types", the same type is valid (WP-1448).
- Fewer follow-on errors (WP-1448): a second `struct I` naming the same
  typedef in a block is not diagnosed again; a constructor with a
  parameter of an unknown type is not also "cannot be redeclared"; an
  invalid class gets no "must explicitly initialize the reference
  member"; a constraint that is not `bool` is not reported again as an
  invalid reference. At `-fclang-compat=22` an explicit instantiation of
  a member whose constraints are not satisfied is clang 22's "no viable
  candidate for explicit instantiation".
- A `used` member function template of a class template that is never
  defined gets `-Wundefined-inline` when the class is instantiated, as
  clang (WP-1448). A `used` destructor of a class template is accepted,
  where clang rejects it (docs/status/deviations.md).
- C: `-fsanitize=enum` (and `undefined`) no longer reports a valid C
  enumeration value as out of range (WP-1449): C has no enum range
  check, as clang; `_Bool` is still checked, and the handler data names
  C types as clang does (`'_Bool'`, `'struct S'`). At
  `-fclang-compat=19` pointer-overflow uses clang 19's C rule.
- C: a structure or union initialized from, or cast from, a `volatile`
  or `const` one (`struct S t = *vp;`, `(struct S)*vp`) is accepted; it
  was rejected as "no matching constructor" (WP-1449). An assignment to
  a `volatile` structure is a volatile copy. A same-type structure cast
  gets clang's `-Wpedantic` warning.
- Contracts, found running GCC's C++ testsuite (WP-1463): a result name
  with the name of a parameter is an error; a constructor's
  precondition or a destructor's postcondition that reaches a member
  through an implicit `this` is an error; a postcondition of a function
  template that names its result now checks each specialization's
  result. A local class's member function defined in a lambda in a
  contract or a default member initializer uses its own `this`.
- C codegen matches clang (WP-1449): padding in C constant aggregates
  and list initializations is zero at `-fclang-compat=20` and later,
  bit-field constants are one byte per element, `_Atomic` arrays are
  initialized element by element, and objects defined only by tentative
  definitions are emitted at the end of the translation unit in clang's
  order.
- `-flto -moutline` on x86-64 warns once more for the link job, as clang
  (WP-1449).
- A destructor that throws at the end of a full-expression destroys
  every object already completed in it (WP-1458; clang and GCC leak
  some): the variable, member, base, array element, delegating
  constructor's object or returned object being initialized, and
  lifetime-extended temporaries. A delete-expression whose destructor
  throws still calls `operator delete`. A class-typed statement
  expression's result is destroyed after its own temporaries.
- Wrong code found running GCC's C++ testsuite (WP-1461): a member
  partial specialization with a non-type argument of the enclosing
  template is chosen again; `T::template B` in a partial
  specialization's arguments is a non-deduced context; an explicit
  specialization of a constrained member specializes the most
  constrained matching member; `typeid` of an array of cv class drops
  the cv-qualifiers from the element too (`A3_A4_1S`); and the
  elements of a structured binding of an array are copy-initialized
  for `= e` and direct-initialized for `{e}` and `(e)`. In C, the
  constants of an enumeration whose values do not all fit in `int`
  have the enumeration's type (C23, as clang 22 and GCC).
- g++.dg wrong-code (WP-1460). A C-style or functional cast that casts
  away cv-qualification on the way to a base at a non-zero offset is a
  static_cast followed by a const_cast, so it adjusts the address
  ([expr.cast]/4.3). A base or virtual-base initializer that uses `this`
  sees the construction vtable ([class.cdtor]). `new T[n]` with a
  negative or too large bound, or with fewer elements than initializers,
  throws `std::bad_array_new_length`, or yields null with a non-throwing
  allocation function ([expr.new]/8). clang passes SIZE_MAX to the
  allocation function, and GCC throws from the nothrow form.
  `-fno-kiln-new-bound-check` restores clang's behaviour. Sized
  deallocation is on by default only from C++14, as in clang.
- C++98 to C++14: a dynamic exception specification (`throw(A, B)`,
  `throw()`) filters at run time, as clang and GCC do: a throw it does not
  allow calls the unexpected handler (`std::set_unexpected`), and what the
  handler throws propagates, becomes `std::bad_exception`, or calls
  `terminate`; kiln let the exception through or called `terminate`. In
  C++98/03 a destructor or class-specific `operator delete` declared
  without a specification may throw (it was implicitly `noexcept`)
  (WP-1457).
- Constant evaluation: a read of a union's active member through a
  glvalue of a similar type (a `const T *const &` bound directly to a
  `T *` member) was rejected as "read of member 'm' of union with active
  member 'm'"; `std::vector<T>::insert(pos, n, value)` with spare capacity
  hit it for an element owning a vector. The Oven now identifies the
  object an access reaches and the member recorded active by one rule
  ([basic.lval]/11). Found in downstream code (WP-1490).
- Reflection: `members_of` (and `static_data_members_of`) of a namespace or
  class listed partial specializations of class and variable templates,
  whose constructors' parameters had no type; they are not
  members-of-representable and are left out. Found in downstream code
  (WP-1490).
- Parser rejects-valid found running GCC's C++ testsuite (WP-1466).
  `X<T>()` and `~X<T>()` as a constructor or destructor declarator in
  class `X` are accepted with a warning in C++20 and later (CWG2237,
  `-Wtemplate-id-cdtor`, an error under `-pedantic-errors`). GNU
  attributes may be mixed with `[[...]]` in any order before a
  statement, a label or a declaration, and `[[]] [[]];` declares
  nothing silently; attribute names may be alternative tokens
  (`[[and]]`). `__extension__` silences pedantic diagnostics in the whole
  declaration or expression it starts, also in template instantiations.
  Friend declarations of constructors, destructors and conversion
  functions (`friend A::A(int);`, `friend X<T>::~X();`) befriend them.
  `p->template A<T>::f()` and `x.A<int>::template g<char>()` name a
  member. Access declarations (`A::x;` in a class) are a using-declaration
  with a deprecation warning in C++98 and an error from C++11, as clang.
  `(T())++` and `N::C::~C();` in a member function parse, and anonymous
  unions and structs in class templates are instantiated.
- Invalid code kiln accepted, found running GCC's C++ testsuite (WP-1471).
  A return-type requirement must be a type-constraint (`{ t } -> bool;` is
  an error), and one whose own arguments are dependent is checked per
  specialization. A destructor template is an error. Lambda template
  parameter lists, `[=, this]` and init-capture packs warn before C++20
  (`-Wc++20-extensions`). `typedef int Q::K;` is an error. The try block
  and handlers of a function-try-block may not redeclare a parameter.
  Too many braces around a scalar or reference initializer is an error
  under `-pedantic-errors`. A member enumeration redeclared with a
  different access is an error. A dependent placeholder constraint
  (`static inline same_as<T> auto v = 0;`) is checked in each
  specialization. A pack expansion into a non-pack alias parameter is
  caught through a template template parameter too (CWG1430). A default
  template argument that makes a later parameter's type invalid is a
  deduction failure. A member pointer to `void` or to a reference formed
  by substitution is an error. `A<n>::B` without `template` is not a
  template template argument. `= delete` after a definition is a
  redefinition, and a deduction guide may not be redeclared.
- Code clang accepts that kiln rejected, found running GCC's C++ testsuite:
  a string literal converts to `char*` with a warning, as in clang
  (`-Wwritable-strings` ExtWarn since C++11 and ranked worst in overload
  resolution; `-Wc++11-compat-deprecated-writable-strings` in C++98); a
  C-style cast between pointer-to-member types is a `reinterpret_cast`
  ([expr.reinterpret.cast]/10); `register` is a warning before C++17
  (`-Wdeprecated-register` in C++11/14); `-f[no-]access-control`; an
  inherited default constructor can be trivial (CWG2799); unnamed bit-fields
  count for standard layout (CWG1813); the injected-class-name of an
  enclosing class template names the class as a base; the implicit
  `operator=` hides a `using B::operator=`; placement `new` passes its
  default arguments to the matching placement `operator delete`; a dependent
  `T::template operator-<U>` names a function template; C++98 template
  arguments have no narrowing check. At `-fclang-compat=22`,
  `-Wno-error=<group>` keeps an extension a warning under `-pedantic-errors`
  (WP-1467).
- A call of a member function template through an object whose type is
  the current instantiation (`[o] { o.f<int>(); }`, `p->f<int>()` in a
  class template) is resolved when the member is instantiated; a const
  object calling a non-const member template was an error even in a
  member never instantiated. In a lambda capturing `*this` with a deduced
  explicit object parameter, a member of the copy is const when the
  closure object is (`decltype((x))` is `const int&`) (WP-1472).
- Constant evaluation, from GCC's C++ testsuite (WP-1468): modifying a
  const member, a const array element, a const subobject reached through
  a base or union member, a const temporary or a `new const T` object
  (by assignment, `++`, compound assignment or `memcpy`) is now an error,
  while the object's own constructor and destructor and `mutable`
  members may still write; an aggregate initializer that reads a later
  element of the same aggregate is an error; before C++26 a constexpr
  local whose value points to itself is rejected, with clang's "may
  differ on each invocation" note. Valid code that was rejected now
  compiles: a variable template specialization whose initializer binds
  a temporary, an out-of-class definition of a static constexpr array
  member with a dependent bound, `this` in a union member's default
  initializer, a block-scope `extern` declaration in a constexpr
  function (CWG2552) and a value-initialized vector in a constant
  initializer. A constexpr function calling a `<cmath>` builtin the
  constant evaluator computes exactly (`sqrt`, `fma`, the rounding,
  remainder and exponent families) is no longer reported as never
  constant before C++23.
- Variable length arrays (GNU extension), found running GCC's C++ testsuite
  (WP-1469): a lambda that captures one by reference, names a typedef or
  `decltype` of one, or takes `sizeof` of one captures its bounds as hidden
  members, so the body sees the size the array was created with;
  range-based `for` over a VLA; VLAs of class objects with constructors and
  destructors (cleanup on scope exit and on a throw); an explicit cast to a
  variably modified type evaluates its bounds, also under `sizeof`; an
  initialized VLA whose bound folds becomes a constant array
  (`-Wgnu-folding-constant`), as in clang. `-Wvla-cxx-extension` is an
  extension warning, so `-pedantic-errors` makes it an error, and
  `-Wpedantic` includes every off-by-default extension, so
  `-Wno-error=pedantic` and `#pragma GCC diagnostic ignored "-Wpedantic"`
  reach `-Wc99-extensions` and `-Wzero-length-array`.
- Accepts-invalid found running GCC's C++ testsuite (WP-1470), now
  errors as in clang: aggregate initialization that leaves a reference
  member uninitialized; before C++20, a defaulted special member whose
  type differs from the implicit one (deleted from C++20, P0641) and a
  parenthesized list initializing an array; an explicit specialization of
  a member whose requires-clause the class's arguments do not satisfy,
  and of a constrained variable template with arguments that do not
  satisfy it; an ambiguous class-scope `operator new`; a class argument
  to `...` whose copy constructor is inaccessible, a C++14 elidable copy
  or move whose constructor is unusable, and `&A::m` of a protected
  member through the base; an explicit-object member function
  redeclaring an implicit-object one ([basic.scope.scope]/4); a default
  argument added to an out-of-line member function of a class template;
  an empty pack initializing an array of unknown bound; and a class
  named as complete inside an instantiation its own definition triggers.
- A copy or move of a prvalue of the same class is elided again, as in
  clang: `B b(1)` with constructors `B(...)` and `B(const B&)` (and
  `B{1}`, `new B(1)`, `B(1)` as a cast) no longer runs the copy
  constructor at run time or in constant evaluation. A virtual base is
  now destroyed when a temporary in its own member-initializer throws
  from its destructor. `-fno-elide-constructors` (and
  `-felide-constructors`) are accepted from C++17, where they turn off
  NRVO and these copies; before C++17 the flag is rejected.
- More valid code from GCC's C++ testsuite is accepted (48 → 111 of
  133 cases of the deduction and overload rows): placement `new (0) X`
  chooses its `operator new` by overload resolution; a template-id
  naming an overload set is resolved after the call's other deductions;
  in C++98 an integral constant expression of value zero is a null
  pointer constant, with clang's warnings; aggregate class template
  argument deduction for a member template of a class template
  specialization and from nested braced lists (also with `#embed`);
  alias template deduction with a defaulted parameter (a C++20
  extension warning before C++20); inherited-constructor guides through
  a base named by a typedef; a defaulted `operator=` matching a
  `noexcept` pointer to member; and the signature of a function template
  befriended by a later redeclaration has the friend's access.
- Valid code found running GCC's C++ testsuite is accepted (WP-1495):
  a dependent qualified type before `operator` in a member declaration
  (`T::type operator+(int);`); an out-of-line constructor of a class
  template whose parameter has a member typedef type
  (`foo<C, T>::foo(bar) {}`); `class N::template X<T>` in an
  elaborated-type-specifier; a friend function with a dependent
  qualifier (`friend R T::f();`), which now befriends the member at
  instantiation; the default base access from the class-key of the
  definition (`class A; struct A : B {};` derives publicly); C++11
  keywords used as identifiers in C++98 (`struct static_assert;`); and
  C++98 access declarations with a template-id in the qualifier. GNU
  `static template` and `inline template` explicit instantiations are
  accepted with clang's `-Wstatic-inline-explicit-instantiation`
  warning, a friend constructor or destructor that matches nothing gets
  clang's candidate notes, and `-C`/`-CC` without `-E` is clang's error
  instead of feeding comments to the parser.
- Union lifetimes in constant evaluation (WP-1494): a read of a union
  member after `destroy_at` is a read of an object outside its lifetime.
  `union { int a; int b; }` members of one type at one offset are told
  apart by the glvalue that names them, for a read, a placement new and
  an explicit destruction. A single-object placement new into an element
  of an inactive array member activates the array and ends the old
  member.
- A virtual call, `typeid` or `dynamic_cast` on a sibling base of the
  object under construction or destruction is not a constant expression
  ([class.cdtor]/4-6); kiln reports it with a note naming the class
  under construction. Reads before an object's lifetime began (a later
  member in a mem-initializer, a local in its own initializer) now say
  "read of object outside its lifetime" as clang 22 does.
- C++20 and C++23 programs found running GCC's C++ testsuite: a class
  non-type template argument holding a lambda through an alias template
  or a member template's default; `decltype` of a static data member no
  longer instantiates its initializer; a qualified call to a static
  member deduces its return type before its constraints are checked; an
  init-capture (also a pack) named in its lambda's requires-clause;
  `typename T::type()` as a simple requirement; an overload set's address
  failing a requirement; a concept naming a non-static data member;
  defaulted comparisons of unions without variant members; a strong
  ordering library without `equivalent`; `static_cast` to an array type;
  libstdc++'s `source_location::current()` in a generic function; and
  `__builtin_is_pointer_interconvertible_with_class` /
  `__builtin_is_corresponding_member` on a member of an anonymous struct
  in a union.
- Valid code found running GCC's C++ testsuite is accepted (WP-1496):
  access during partial-specialization matching and template argument
  deduction is checked in the template's context (a failure in a
  function template's signature is a substitution failure), and a
  default template argument's access in its template's context;
  `V a{{}}` with an `initializer_list` constructor gives a list of one
  element; a deleted or undefined function template named in `decltype`
  is no longer diagnosed before overload resolution selects it; a friend
  function template defined in a class template is found whichever
  specialization defines it; a base class's conversion function no
  longer ranks worse than the derived class's own (`bool(y)`); an
  operator in a template finds the non-member operators visible at its
  definition, plus argument-dependent lookup at instantiation, and a
  non-dependent one keeps the function selected at definition; before
  C++17 the elidable copy or move constructor of a temporary is
  selected, access-checked and considered by `noexcept`. A `static_cast`
  to a pointer from a non-pointer (a member pointer to `void*`
  included) has clang's "cannot cast from type ... to pointer type".
- Variable length arrays and expansion statements (WP-1499): a variable
  length array of arrays of class objects (`S a[n][3]`) is constructed
  and destroyed like a flat one, also on unwinding; a lambda captures the
  bounds of every variably modified type its body uses (`typeof`, casts,
  local declarations), not only those it names; in C a bound that reads a
  `const` object makes a variable length array, folded to a constant
  array with clang's `-Wgnu-folding-constant` where one is needed. An
  iterating expansion statement checks the iterator's `++`, `!=` and `*`
  and needs both member `begin` and `end`; an uncaptured range in a lambda
  is one error, not two; and a destructuring expansion statement binds its
  range by reference instead of copying it, so a non-constant range is
  rejected as GCC does.
- Invalid code found running GCC's C++ testsuite is now rejected as
  clang 22 rejects it: `new T[n](args)` before C++20; a default
  argument on an out-of-line member of a class template or on an
  explicit specialization; a direct-initialization at C++11/14 that two
  conversion functions make ambiguous; a pack-expansion fold constraint
  before C++26 (atomic there, so neither declaration is more
  constrained); a member that would hide a using-declared base member
  whose requires-clause names different class arguments; and a class
  template's injected-class-name as its own (incomplete) base. C++11
  mode warns that a `constexpr` member function will not be implicitly
  `const` in C++14 (`-Wconstexpr-not-const`); errors in an implicit
  member or element initializer carry clang's "in implicit
  initialization of ..." note.
- A class template's friend constructor, destructor or conversion function
  whose class or type depends on the template (`friend Z<T>::Z(typename
  T::t);`) is matched again in each specialization and befriends that
  member, or is diagnosed there when no member matches. A precompiled
  header or module keeps these friends. `friend void A::f(int);` naming a
  specialization of a member function template befriends it, and is
  rejected when the template is inaccessible. A friend class declared by a
  local class names a class in the innermost enclosing block scope
  ([class.friend]/11), as GCC does. `X<T>::X(nosuch)` is one error, the
  column of a friend's qualifier points at the qualifier, and a member of a
  class template with dependent bases named through its own class in C++98
  is reported as "no member named".
- Reflection (WP-1505): `^^` of a name found through a using-declarator is
  ill-formed ([expr.reflect]/5); a splice of a template without `template`
  before it is a `-Wmissing-template-keyword` warning, as GCC, and
  `template` before a splice that is not a template is an error;
  `define_aggregate` of a type alias is rejected; only the condition of an
  `if constexpr` is constant-evaluated, not its init-statement; an
  annotation after a ref-qualifier, in a new-declarator or on an unnamed
  bit-field is rejected; `extract` of a reflected reference parameter
  reads the object it refers to. clang 22's type-trait keywords revert to
  identifiers exactly where clang 22 reverts them (`-Wkeyword-compat`), so
  `int __is_convertible;` as a member is rejected as clang and GCC do.

- The Oven tells union members apart through more pointers (WP-1508): the
  member a pointer designates survives pointer arithmetic and a store to
  memory (an array of pointers, a class member, a lambda capture, a
  reference member), and the address of a subobject reached through a
  member (`&u.s.k`, `&u.arr[1]`, `(&u.s)->k`, a base of a member) names
  that member at each union level, so reading through it while another
  member is active is rejected as clang does. Subtracting pointers to two
  members of one union is rejected ("subtracted pointers are not elements
  of the same array"); destroying an inactive union member is rejected in
  C++20 and C++23 too, as clang does; copying a union whose member was
  destroyed (or one with no active member) is accepted in C++23 as in
  C++26, and the copy keeps the member ended.
- More concepts code found running GCC's C++ testsuite is handled. A
  concept-id argument that fails to substitute makes only the atomic
  constraints that use its parameter unsatisfied, as in clang 22
  (`C<T, T&>` with `void` can still be satisfied through an operand that
  does not use the second argument), and `t.*pmf` naming a member
  function as a simple requirement is a substitution failure of that
  requirement, not an error. These are accepted now: an out-of-line
  member of a constrained partial
  specialization (matched by its whole template head), a
  using-declaration hidden by a member that corresponds only after
  substitution, a template template parameter constrained by `C<Ts...>`
  for an argument with the same constraint, an alias-sugared base pack
  expansion and a consteval default constructor in an instantiated
  array. A cast to a deleted converting constructor gets clang's
  candidate notes, and a call or operator that only finds a function
  declared after the template is clang's "neither visible in the
  template definition nor found by argument-dependent lookup" error.

- Template argument deduction and class template argument deduction
  found running GCC's C++ testsuite: with `-fclang-compat=22` the
  associated constraints are checked before the non-dependent conversions
  that may use a user-defined conversion (CWG2369, in clang 22's order); an
  aggregate deduction candidate skips a pack that is not trailing;
  copy-initialization CTAD considers only guides callable with one
  argument, and a guide whose `explicit(b)` depends on a template
  parameter is resolved per deduction; `auto(x)` with a dependent operand,
  `new char[]("abc")` (C++20) and `new char[]{"abc"}`, function parameter
  packs of function type (`T... f(int)`), constant expressions such as
  template arguments inside `sizeof` and `decltype`, dependent functional
  casts to a reference type, and, in C++11, implicit `const` on a
  constexpr member function with a trailing return type and classes with
  default member initializers not being aggregates. Candidate notes name
  an implicit copy or move assignment operator, report "packs of
  different lengths" and the value of an `explicit(b)` guide, and alias
  guides print no `<null expr>` default arguments.

- Found running GCC's C++ testsuite (WP-1503, WP-1504, WP-1511): a
  single-inheritance chain of any depth gets its vtables (the builder
  ran out of fuel past about 250 classes, and virtual-base walks stopped
  at depth 256); `static int a[10] = {a[3] = 5}` is initialized
  dynamically as clang does instead of being folded to a constant that
  dropped the store; copy-initialization with an explicit copy
  constructor reports an array structured binding at clang's location,
  naming the array type, and adds the parameter note for a function
  argument; `-fcontracts-client-check=pre|all` compiles calls with
  `_Complex` arguments or results and checks the allocation and
  deallocation functions of new- and delete-expressions, as GCC 16.1
  does.

- More invalid code is rejected, found running GCC's C++ testsuite
  (WP-1513). A friend declaration of a constructor, destructor or
  conversion function template of a class template befriends it, also
  through a precompiled header or a module, so access through it is no
  longer denied. A static data member defined out of its class template
  keeps its access, and naming a private variable or function template
  through a template-id is checked. Before C++17 a copy-initialization by
  user-defined conversion checks the copy from the temporary, so the old
  `auto_ptr` idiom is rejected as clang does. Before C++20 a defaulted
  `==` or `!=` warns that it is a C++20 extension and a defaulted
  relational operator is rejected. `new C[](8, 9)` no longer needs a
  default constructor, and an element of a parenthesized array new is
  diagnosed at the allocated type. An access declaration whose qualifier
  is a template-id is parsed in every mode, and a using-declaration that
  names a constructor is rejected before C++11. `cargo xtask libstdcxx`
  takes `--std c++11` and `--std c++14`.

- A variable, parameter or structured binding whose only use is its
  reflection (`^^v`, for example as a template argument of a template
  that only queries it or uses its type) no longer gets
  `-Wunused-variable`, `-Wunused-but-set-variable` or, at file scope,
  `-Wunneeded-internal-declaration`. Reported by a user.

- Expansion statements and variable length arrays, found running GCC's
  C++ testsuite (WP-1512). A generic lambda that odr-uses an uncaptured
  local as the range of a `template for` is rejected when it is
  instantiated (it used to fail in code generation), while reading the
  value needs no capture. A non-`constexpr` `operator++` of an iterating
  expansion statement's iterator is rejected. The destructuring form
  binds a prvalue range with `constexpr auto&&`, as the working draft
  says, instead of copying it. Initializers of C++ variable length arrays
  are diagnosed where clang 22 diagnoses them, and before C++11 a `{`
  after a declarator is the error "expected ';' at end of declaration".
  In C, `sizeof(int[m])` with a file-scope `const int m` is no longer a
  constant, and a compound literal of such a type is folded with
  `-Wgnu-folding-constant` or rejected, as clang does. `cargo xtask test
  --compiler clang++-N` compares against clang N's output when no
  `--oracle` is given.

- The linkage of a `constexpr` static data member now matches clang and
  GCC. Before C++17 the member is not inline: its out-of-class
  definition (`constexpr int A::n;`) is a strong external definition,
  emitted even when unused, so two units that both define it fail to
  link, and a unit that uses the member without that definition refers
  to the external symbol instead of defining a private copy. From C++17
  the redundant out-of-class redeclaration makes the member `weak_odr`
  and always emitted, so a C++14 unit can link against it. The
  redeclaration is no longer a redefinition in C++17 (a second one is
  accepted), `-Wdeprecated-redundant-constexpr-static-def` no longer
  fires before C++17, and `alignas` on the out-of-class definition no
  longer errors. Objects built by earlier versions have the weaker
  linkage for these members (`linkonce_odr`, dropped when unused).
  Reported from linking kiln objects with mortar.

- Reflection fixes found running GCC's C++ testsuite (WP-1515, WP-1521).
  `std::meta::extract<int>(^^b)` of a block-scope constexpr variable gives
  its constant, also through a template argument, and `extract` of a
  reflected reference local works; a local whose lifetime ended in the
  same evaluation is still rejected, also by `constant_of`. `^^B::S`
  reflects the member `S` where it is not a type, `^^N::template T` and
  `^^T::template N` reflect a template, and `template` before a name that
  is not a template is rejected. `namespace A = [: r :];`, `~typename
  [: r :]()` and `typename [: r :]<args>::member` are accepted. A dependent
  namespace alias splice is reported as not supported yet.

- Regressions found by the second full run of GCC's C++ testsuite
  (WP-1473), each a behaviour kiln had before a recent fix row:
  a by-copy capture of a reference to a function is valid again; `explicit`
  in a friend declaration and a friend destructor template are rejected;
  `&f<X>` picks the more constrained template only when the two
  specializations have the same parameter-type-lists (CWG2918), so
  `&g<int>` of `g(T)` and `g(T*)` is ambiguous; a converted constant
  expression rejects a conversion function to `double` for an `int`
  template parameter; C++98 accepts an explicit instantiation in an
  enclosing namespace, as C++11 does; an array from an empty parenthesized
  pack expansion is value-initialized; before C++17 a template-id's
  argument value does not deduce the type of `template<class T, T N>`;
  before C++17 an in-class `constexpr` static data member without an
  initializer is an error again; before C++17
  the copy that ends a copy-initialization by converting constructor
  direct-initializes, so an explicit copy constructor serves it; an
  out-of-line C++11 definition of a static `constexpr` member function
  template is not made `const`; `C::m()` inside a member of a sibling
  class `B` (both derived from `A`) is a call without an object; and
  `__is_pod` of a class with `const` members is true in C++98.
- Found running GCC's C++ testsuite (WP-1547, WP-1542, WP-1545): a
  function-local `static` initialized with label addresses in a
  constructor of a class with a virtual base no longer jumps into the
  other constructor variant's code (a crash at run time); a static union or struct whose flexible array member is
  initialized gets its elements; in C++26 the destructor of an object whose
  class has a virtual base is called again (it was skipped); a string user-defined literal calls
  `operator""X(str, len)` before the GNU character-pack template;
  passing a class through `...` whose copy would select a variadic
  constructor is an error instead of a stack overflow; and from C++11 a
  candidate that needs no string-literal-to-`char*` conversion beats
  one that does, so `s != "x"` with `s` converting to `char*` is no
  longer ambiguous.
- Member templates named after a qualifier in a class member access
  (WP-1530, [temp.names]/7, [basic.lookup.qual.general]/3):
  `p->N::f<U>()` with a type-dependent `p` and a non-dependent class `N`
  (as in `decltype(impl::create<T>()->impl::create<U>())`), and
  `this->C::f<int>()` / `this->B::f<int>()` naming an overloaded member
  template of the current instantiation or of a non-dependent base, take
  a template argument list without `template`.
- nlohmann_json 3.12.0 and range-v3 0.12.0 build again (WP-1571); they
  failed after unreleased changes made for GCC's C++ testsuite (WP-1470,
  WP-1509, WP-1513), and no released version was affected. Before C++17
  the copy of the temporary in a copy-initialization by conversion copies
  the cv-unqualified temporary, so `const T x = ...;` no longer picks a
  `const T&&` constructor template or a deleted copy constructor; a member
  access from a template pattern into a class still being instantiated is
  accepted again; a dependent `= {}` default argument after a temporary in
  the parameter's type is not converted from `void`; and a nested
  concept-id with a pack parameter under an argument that fails to
  substitute is unsatisfied instead of "an atomic constraint that stays
  value-dependent".

- Constant evaluation, found running GCC's C++ testsuite (WP-1526..WP-1529):
  P2280 (references of unknown value) applies in every mode from C++11, as a
  defect report, and `this` as the object expression of a class member
  access (`this->m.f()`, `inner.size()` in a template argument of a
  non-constexpr member function) designates an object of unknown value;
  placement new or `std::construct_at` into an inactive `T[1]` union member
  activates it; `__builtin_bit_cast` of classes with base classes or
  bit-fields is constant ([bit.cast]/2; unnamed bit-fields are padding).

- Declaration specifiers and attributes, found running GCC's C++ testsuite
  (WP-1541, WP-1536): `virtual`, `explicit` and `_Noreturn` on a structured
  binding are errors, as are function specifiers on a for-range
  declaration, an expansion statement and a condition, and `constexpr`,
  `consteval` and `constinit` in any other type-specifier-seq (spec/sema.md
  has the table). An attribute-specifier-seq before an asm-declaration is
  accepted, with each attribute ignored (CWG2262). Keywords and alternative
  tokens are attribute-tokens (`[[gnu::const]]` in C, `[[using gnu: const]]`).
  A scoped attribute after a `using` prefix is an error, and a `using`
  prefix before C++17 is an extension warning. `inline template` keeps
  clang's warning; GCC rejects it.

- Constant evaluation (WP-1574, WP-1568, WP-1575, WP-1569, WP-1567):
  `std::optional::emplace` after an earlier `emplace` or `reset` is
  constant again with libstdc++ 14 and libc++ at C++20 and C++23 (an empty
  base class was taken as destroyed when the member at its address was);
  the destructors of empty virtual bases run in constant evaluation
  ([class.dtor]/13), so a constant-destruction variable of a class with
  virtual bases is no longer guarded in code generation; an assignment
  through an inactive union member of class type whose default
  constructor is not trivial, or is deleted, is not a constant expression
  ([class.union.general]/6); a union's flexible array member initializer
  with elements of class type is accepted (GNU); and `alignof` of a
  cv-qualified array typedef carrying `__attribute__((aligned))`, or of
  an array of one, is the typedef's alignment.
- GCC compatibility (WP-1587, WP-1549, WP-1551, WP-1550): a C cast to a
  union type from the type of one of its members (GNU) is accepted again,
  as in clang (`-Wgnu-union-cast` under `-pedantic`); GCC's
  `__builtin_add_overflow_p`, `__builtin_sub_overflow_p` and
  `__builtin_mul_overflow_p` are implemented, constant when their
  operands are (integer constant expressions in C); in C++ a plain
  function pointer converts implicitly to one whose function type adds
  GNU `noreturn`, as GCC accepts, with a
  `-Wincompatible-function-pointer-types` warning (in C it is clang's
  error); an `extern inline` redefinition in a GNU dialect and an unknown
  `__builtin_` function in C get clang's wording.

- Accepts-invalid cases found running GCC's C++ testsuite (WP-1532..WP-1535):
  the inherited-constructor tie-break needs equal parameter types
  ([over.match.best.general]/2.7); deduction from a derived class whose two
  bases match with different template arguments fails
  ([temp.deduct.call]/5); a copy and a move constructor reached through
  different conversion functions stay ambiguous from C++17 although a
  conversion function yields the class ([over.ics.rank]/3.3); a class-scope
  using-declaration that conflicts with a member (a class and a typedef of
  another type, an enumerator of the class's own enumeration) is an error;
  a function template specialization named as a template argument in a
  template body is instantiated with that body ([basic.def.odr]/7); a
  discarded volatile glvalue of class type must be complete from C++11
  ([expr.context]/2); a discarded constant is not odr-used, so a lambda
  does not capture it.
- Concepts, found running GCC's C++ testsuite (WP-1531, WP-1579, WP-1553):
  a requires-expression in a generic lambda checks its requirements in
  order, so a requirement after one that fails is no longer substituted
  early into a hard error ([expr.prim.req.general]/5); a concept-id whose
  default template argument fails to substitute leaves the atoms that do
  not name that parameter to be evaluated as usual, including in a nested
  requirement ([temp.constr.atomic]/1, /3); a concept with a trailing pack
  parameter gets the same per-atom treatment, so a disjunction with a
  satisfied alternative is satisfied and the notes name the failing atom;
  and GCC's `-ftemplate-depth-N` spelling is accepted as
  `-ftemplate-depth=N`.

- Sema follow-ups from GCC's C++ testsuite (WP-1593, WP-1595, WP-1572,
  WP-1588, WP-1591): a member of a nested anonymous union with a default
  member initializer no longer deletes the enclosing union's default
  constructor and is the active member in constant evaluation, and an
  anonymous union object at namespace or block scope runs its default
  member initializers (they were skipped at run time); an array whose
  element size is not a multiple of its alignment is an error ([dcl.array]);
  `this` in a static member function's trailing return type,
  noexcept-specifier or requires-clause is an error ([expr.prim.this]/3); a
  structured binding declaration in a condition whose variable is an array
  is an error ([stmt.pre]/5) and the pre-C++26 extension warning uses clang
  22's wording; a structured binding declaration at class scope gets one
  error; a function template specialization named as a template argument
  in a lambda body inside a template is instantiated.
- GCC builtins found running GCC's C++ testsuite: `__builtin_shuffle`
  with one or two vectors and a mask that need not be constant (one
  `pshufb`/`tbl` at -O2 for 16 bytes), `__builtin_shufflevector` with
  vectors of different lengths, `__builtin_speculation_safe_value` (with
  GCC's speculation barrier: `lfence` on x86_64, `isb; dsb sy` on
  AArch64), `__builtin_assoc_barrier` (an arithmetic fence under
  `-ffast-math`), `__builtin_ffsg`, `__builtin_clrsbg`,
  `__builtin_parityg` and `__builtin_puts`. clang lacks the new names, so
  `__has_builtin` answers 0 for them, as clang does. A deprecated or
  unavailable conversion function is diagnosed where it is used
  implicitly, and so is an enumerator of an instantiated class
  template's member enumeration.

- Oven fixes found running GCC's C++ testsuite (WP-1590, WP-1594,
  WP-1573): an odr-use of a by-copy capture in a constant expression
  evaluated in the lambda body names the closure's member, so reading it or
  taking its address there is not constant ([expr.prim.lambda.capture]/11,
  [expr.const]); a delete-expression deallocates the storage when the
  destructor throws ([expr.delete]/7); after a bit-cast, an indeterminate
  `unsigned char` bit-field that shares a byte with a determinate one is
  not readable in a constant expression, also through copies ([bit.cast]/2).
- Reflection cases that kiln rejected, found running GCC's C++ testsuite
  (WP-1537): `identifier_of` and `display_string_of` of a base relationship
  and of a function parameter pack; `data_member_spec` checks its
  annotations; `type_of` of a structured binding and of an enumerator inside
  its enum-specifier; `is_final` of a class being defined; access checks
  for reflected variant members and protected members; `extract` of a
  pointer variable and of a closure converted to a function pointer;
  `access_context::current()` in inheriting constructors and trailing
  requires-clauses; a splice of a base relationship accesses the direct
  base; a constructor parameter may start with a splice; `==` and `!=` on
  `std::meta::info` operands of class type that convert to it
  ([over.built]/16); a reflection of a dependent member template-id; the
  elements of an expansion statement over a non-lvalue are xvalues
  (CWG3149); `>=` after a splice in a template argument list; defaulted
  comparisons with an explicit object parameter taken by value; default
  arguments of `extern "C"` functions in different namespaces.
- Anonymous unions and structs, enumerations with an aligned attribute,
  case values in lambdas and copy-initialization through conversion
  functions (WP-1604, WP-1606, WP-1592): an anonymous union or struct
  rejects a nested type, a member function, a static member and a
  non-public data member as clang does ([class.union.anon]/1;
  `-Wnested-anon-types` for an unnamed nested class when named), and C's
  anonymous unions are unchanged; `alignof` of an enumeration with an
  aligned attribute is that alignment (it was the underlying type's), and
  an array of one whose size is not a multiple of it is an error, also in
  a template; an odr-use of a by-copy capture in a case value or a GNU
  case range outside a template is rejected (the case value now ends its
  full-expression before it is evaluated); from C++17, `B b(u)` with
  `operator B() const` beside a better `operator B&()` initializes `b`
  through `operator B() const` as clang does instead of being ambiguous
  (a recorded deviation, docs/status/deviations.md).
- Templates and builtins, found running GCC's C++ testsuite (WP-1609,
  WP-1610, WP-1612): a call through `this` of the current instantiation
  that names an overload set or a member template (`this->f(1)`,
  `(*this).f`, an implicit `this`, `C::f`, a non-dependent base's
  `Q::f`, explicit template arguments) is resolved at definition and is
  not type-dependent when instantiation cannot choose differently, so a
  `<` after a member template of its result needs no `template`; a
  type-constraint `C<A...>` is checked as the concept-id `C<X, A...>`, so
  a pack expansion onto a non-pack parameter of the concept and a wrong
  number of arguments are rejected as clang does; `__builtin_parityg`
  rejects `bool` and enumeration operands as GCC does; a specialization
  whose template arguments hold an error is dropped instead of being
  followed by a `redefinition` error.
- Required diagnostics found running GCC's C++ testsuite: pointer-to-member
  conversions through an inaccessible or ambiguous base; `inline`,
  `virtual`, `explicit` and `_Noreturn` on parameters, qualified parameter
  declarators, misplaced `void` parameters, cv-qualifiers on conversion
  functions, specifiers on free-standing tag declarations, a template
  typedef, a typedef-name destructor (`-Wdtor-typedef`), inline variables
  before C++17 (`-Wc++17-extensions`); friend forms (explicit
  specializations, tagless friend type templates, `friend` not first,
  unqualified friend constructors, destructors and conversions); `C::C`
  naming a type (accepted under `-fpermissive`), `typename` naming a
  non-type template, ambiguous qualified injected-class-names, a type alias
  template naming a non-class in a nested-name-specifier, and a repeated
  `using E::x;` in a class; x86-64 `"+f"` asm outputs are rejected as clang
  22 does instead of crashing the backend.
- `<stddef.h>` declares `::nullptr_t` in C++11 and later; floating and
  complex floating `static const` members with an in-class initializer at
  C++98 are accepted as the GNU extension (`-Wgnu-static-float-init`); a
  by-copy capture odr-used in a local class's or enumeration's scope
  (enumerator values, bit-field widths, member array bounds) is diagnosed;
  a template argument's narrowing check no longer folds it before its
  full-expression ends; a class declared after a using-declarator naming a
  type of the same name in that scope is rejected; an rvalue reference
  bound through a conversion function yielding a derived class beside a
  better lvalue conversion function now selects the copy constructor or
  `const T&` overload as clang and GCC do (WP-1615, WP-1616, WP-1621,
  WP-1622, WP-1624, WP-1625).
- Reflection inside templates and in constant evaluation (WP-1613,
  WP-1614; found running GCC's C++ testsuite): a member splice in a
  template is substituted from the member it names, not looked up again
  by name; a namespace alias of a dependent splice-specifier; a
  block-scope opaque enumeration and its later definition are one
  enumeration per instantiation; `declval` of an abominable function
  type is not formed in a type trait; a substituted constant template
  argument is manifestly constant-evaluated; a constexpr structured
  binding whose `get` returns a prvalue, and constexpr expansion
  statements over such elements, work in templates ([dcl.struct.bind]/4);
  `reflect_constant` of a class value copies it as a template parameter
  object and throws when the copy is not template-argument-equivalent
  ([temp.arg.nontype]/4); a constexpr-unknown function reference binds an
  unspecified function ([expr.const]/10), so `reflect_function` and
  `reflect_object` of one work and calling through it is diagnosed as
  clang 22 does; `define_aggregate` runs in what `substitute` instantiates;
  `variable_of` in a nested evaluation sees the function of that
  evaluation's point (CWG3162). A nested evaluation still sees classes
  the enclosing evaluation completed with `define_aggregate`.
- More required diagnostics found running GCC's C++ testsuite: an overload
  set with no target in a `for` increment or a conditional operand;
  `static_cast` of an array of `const` to `void *` and downcasts that cast
  away qualifiers; closures have no default constructor before C++20 and
  none with captures; a member of an unspecialized class template
  specialized without its own `template <>`; class template parameters are
  not forwarding references in implicit deduction guides; an inaccessible
  move constructor in a returned local is not replaced by the copy
  constructor; access of a class template named as a template template
  argument; members of an anonymous union declared private or protected;
  later-standard features in earlier modes (`typename`-free dependent types
  before C++20, `static` and `thread_local` variables in constexpr
  functions before C++23, class templates without arguments before C++17,
  static and multi-parameter `operator[]` and static `operator()` before
  C++23); an ambiguous injected-class-name found through dependent bases
  at instantiation; a friend redeclaration of an implicit special member
  without its exception specification. A class's using-declarator of its
  own enumerator is accepted, as clang 22 does (WP-1630, WP-1631).
- `sizeof` of a variable length array type, and every other use of it, in
  an instantiated function template is rebuilt per specialization (it
  failed in code generation). A module importer no longer runs an
  initializer for an out-of-class definition of a static data member
  initialized in the class, and refers to an imported strong inline member
  as `available_externally`, as clang does. Caller-side contract checks
  now cover function template specializations that are only declared in
  the unit, including explicit specializations. Before C++20 a captureless
  closure is not trivial (no default constructor), so a local one is no
  longer copied from a constant. Deep single-inheritance chains below a
  virtual base compile faster (3.7x at depth 240). The test harness has
  `clang-unit:` (a unit compiled by clang) and `link: fail`
  (WP-1520, WP-1523, WP-1525).

## [1.2.1] - 2026-11-14

A bug-fix release: one ABI fix for x86-64 code built with AVX enabled.
Nothing else differs from 1.2.0, and nothing in it changes what
[docs/STABILITY.md](docs/STABILITY.md) promises. The gate results are in
[docs/status/release-1.2.1.md](docs/status/release-1.2.1.md).

### Fixed

- **ABI fix, x86-64 (WP-1418).** x86-64 code built with AVX enabled that
  passes or returns 256-bit (512-bit with AVX-512) vectors by value
  across a boundary with code built by another compiler, or by an older
  kiln, changes its calling convention to the standard one. With `avx`
  in the translation unit's features (`-mavx`, `-mavx2`, `-march=haswell`,
  `-march=x86-64-v3`, ...), a named 256-bit vector argument or return
  value, and a struct, union or one-element array whose only member is
  such a vector, now goes in a `ymm` register as the psABI, clang and GCC
  do, not on the stack and through a hidden return pointer; with
  `avx512f` the same holds for 512-bit vectors in `zmm` registers.
  Earlier kiln releases used the no-AVX convention whatever the
  features. kiln objects built before and after this fix do not mix for
  such functions: rebuild every object that defines or calls one. As
  in clang 21 and later, at the default `-fclang-compat=22` a class with
  a base narrower than itself (`struct D : Empty { __m256 v; }`) stays
  in memory; at `-fclang-compat=19` it goes in a register, as in clang 19.
  Vectors passed through `...`, code built without AVX, `ms_abi`
  functions and AArch64 are unchanged.

## [1.2.0] - 2026-11-14

Precompiled headers that are restored instead of parsed again, the
experimental hod back end for `-O0`, and the fixes made since 1.1.0. The
gate results are in
[docs/status/release-1.2.0.md](docs/status/release-1.2.0.md).

### Added

- The PCH options are documented and supported (WP-1404; `--help`,
  "PRECOMPILED HEADERS"): `-fkiln-pch-snapshot` (the default) and
  `-fno-kiln-pch-snapshot`, `-fkiln-pch-snapshot=write` (write the
  snapshot, replay when consuming), `=load` (restore, write none),
  `=check` (restore and replay, report the first difference), and
  `-fkiln-pch-validate=mtime` (the default: an input file with the
  recorded size and time is not read, one with another time is
  compared by content) or `=content` (every input file is compared by
  content). The release gate gains `scripts/pch-release-gate.sh`.
- The PCH contract is spec/pch.md (WP-1397): the file layout, the
  snapshot section and its versioning, the identity, the search-path
  rule, the producer, the consumer's fallbacks and their warnings. Each
  snapshot format is recorded with its schema hash in
  `crates/kiln_driver/pch-snapshot-format.txt`, and a test fails, saying
  what to bump, when the schema changes without a new format version. A
  PCH from another kiln build is never misread: another snapshot format
  or schema replays with `-Wpch-snapshot-unused`, another kiln version is
  `PCH file built from a different branch`, and kiln 1.0 and 1.1 PCHs
  are `uses an older format`; rebuild the PCH.
- Experimental `-fkiln-backend=hod` (also `KILN_BACKEND=hod`; WP-1371):
  at `-O0`, an object's module is handed, before `opt`, to the hod back
  end (github.com/christopherfretz/hod, an external executable found with
  `--hod=`, `KILN_HOD`, next to `llc`, or on `PATH`), with `opt` + `llc`
  as the fallback whenever hod declines or fails. The default stays
  `llvm`. spec/driver.md "Back-end selection".
- `-fkiln-backend=hod`: coroutine translation units are handed to hod
  before `opt` too (WP-1398): a module whose only `opt` triggers are
  `alwaysinline` and coroutines goes to hod, which compiles the
  coroutines itself; when hod declines, kiln runs the coroutine passes
  and `llc` as before. `KILN_STATS_LOG` lines gain `hod_skip`, the reason
  a job was not offered to hod. spec/driver.md "Back-end selection",
  "Compile statistics log".
- `-g`: imported entities (WP-1365, reported by hod). Using-directives,
  using-declarations and namespace aliases, at namespace and block scope,
  get clang's `DW_TAG_imported_module` / `DW_TAG_imported_declaration`, so
  gdb and lldb resolve names reached through them. Nothing changes at
  `-gline-tables-only` or without `-g`.
  spec/landed/codegen/WP-1365.md.
- `cargo xtask libcxx` cross runs (WP-1391, proposed by hod):
  `KILN_LIBCXX_TARGET=<triple>` compiles and links the suite for another
  Linux target (the `target=` feature follows; a cross target links the
  static archives of its `.toolchain/libcxx-N-root` tree),
  `KILN_LIBCXX_EXEC="<words>"` runs the test binaries and `%{exec}`
  through an executor such as `qemu-x86_64`, and
  `KILN_LIBCXX_TIMEOUT_SCALE=<n>` stretches both run limits (10 under
  emulation). Unset, nothing changes. spec/libcxx.md "Cross targets".

### Changed

- Precompiled headers are fast by default (WP-1404): a PCH now carries a
  snapshot of the compiler's whole front-end state at the end of the
  header, and a consumer restores it instead of processing the header
  again. On `<bits/stdc++.h>` a `-fsyntax-only` consumer takes
  0.57 s against 1.48 s for the replay and 0.67 s for clang 19's
  PCH ([docs/status/pch.md](docs/status/pch.md)). Whenever the snapshot
  cannot be used the consumer replays the header as before, so a build
  never fails for it: silently (a `-v` note) for the cases that replay by
  design, and with one `-Wpch-snapshot-unused` warning when the
  configuration differs (another `-std`, target, `-D` list, warning
  options, a changed input file, another kiln build). A restored
  consumer does not repeat the header's warnings. A header whose state
  at its end cannot be saved (an open global module fragment, for
  example) still gets its PCH and one warning, `precompiled header
  '<pch>' cannot carry a snapshot (<why>); its consumers process the
  header again [-Wpch-snapshot-unused]`. The PCH is larger: 35.2 MB
  for `<bits/stdc++.h>` (clang 19: 26.7 MB), 8.5 MB for four
  common headers. A PCH is a trusted artifact: it is tied to the exact
  kiln binary that wrote it and to its input files (both are checked,
  and a mismatch replays or is rejected as in clang), and it is not
  meant to be shared between builds of kiln. `-fno-kiln-pch-snapshot`
  gives the 1.1 behaviour on either side. `-Wpch-snapshot-unused` only
  reports lost time, so a plain `-Werror` leaves it a warning (WP-1397,
  as clang's `DefaultWarnNoWerror`); `-Werror=pch-snapshot-unused` makes
  it an error, and a producer that cannot save its snapshot then fails
  and writes no PCH. The PCH differences from clang are in
  [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md#precompiled-headers).
  spec/pch.md.
- The PCH snapshot (WP-1403): a consumer whose include search path
  differs from the producer's (an
  extra `-I`, another order) now restores the snapshot when every header
  lookup the prefix made finds the same file, as a user or system header,
  as before; the producer records those lookups in the PCH (section
  format version 2; a version 1 snapshot is refused and replayed). A
  prefix built from absolute paths restores from any working directory,
  and `-nostdinc*` are compared through the search list they build. A
  snapshot refused for its configuration now warns once (`warning:
  precompiled header 'p.pch' was built with a different configuration
  (<why>); the header is processed again [-Wpch-snapshot-unused]`, on by
  default) and replays; `-v` names the first lookup that changed.
  spec/pch.md "Lookup-tolerant search path".
- `-g` (the default limited kind) describes a class this unit never
  required complete as a declaration, as clang does
  (`isCompleteDefinitionRequired`, WP-1389): a class used only through
  pointers, references or typedefs is completed by the debugger from the
  unit that requires it. Standard-library units shrink to clang's
  size (`<string>`: 16784 -> 15042 bytes of `.debug_info`, clang 19
  15126). A using-declaration of a function with a deduced return type
  now follows clang's emission point. Nothing changes without `-g`.
  spec/landed/codegen/WP-1389.md.

- Consteval-only values follow P4101R1 (the working draft's value-based
  model) instead of P2996R13's consteval-only types (WP-1328, first
  part). What is ill-formed is now decided by value: an immediate
  object (one holding a non-null reflection, or a pointer to an
  immediate function or into an immediate object) must be a `constexpr`
  variable, a template parameter object, or live within a constant
  expression, and an expression naming one is an immediate constant
  expression, so it escalates or is an error outside an immediate
  function context. Newly accepted: variables, members and pointers
  whose reflections are all null (`std::meta::info x;`,
  `thread_local info t;`), non-`constexpr` references bound to an
  immediate object, and run-time reads of the other members of a
  `constexpr` object whose reflections are null. Newly rejected: an
  immediate object used by an odr-use before its definition. Still
  rejected, as in 1.1.0: a run-time read of a member of an immediate
  object (`const char* n = e.name;`). Wording: "consteval-only value"
  replaces "consteval-only type". These are divergences from GCC 16.1,
  listed in [docs/status/deviations.md](docs/status/deviations.md).
- P4101R1, second part (WP-1361). Newly accepted (each rejected by
  1.1.0 and GCC 16.1): run-time `std::meta::info` objects, which hold
  the null reflection and are emitted as a pointer-sized null, passed
  and returned like `std::nullptr_t`; non-`consteval` functions with
  `info` in their type (parameters, return types, members, virtual
  functions, lambdas, templates, function and member pointers);
  `constexpr` pointers to immediate functions (`constexpr auto b =
  plus1;`, arrays and aggregates of them, `constexpr auto e = g();`)
  and calls through them in constant expressions (P3603); run-time
  `__builtin_bit_cast` from or to `info`, now trivially copyable,
  trivial and POD as a scalar type (a constant-evaluated one is "not a
  constant expression"). Newly rejected: a consteval override of a
  non-consteval virtual function in a class with an `info` member
  ([class.virtual]/18 as the working draft words it; a system header
  is exempt, so libstdc++ 16.1 `<meta>` still compiles). Changed:
  `__cpp_consteval` is 202606L with reflection enabled (202211L with
  `-fno-reflection`); immediate-escalation notes name the callee
  unqualified and say "evaluates the address of an immediate function
  'operator()'" for a lambda call operator, as clang 19/22 do; a
  default argument holding a consteval-only value is reported at the
  call that uses it.

### Removed

- The experimental PCH declaration image (WP-1397). Its option,
  `-fkiln-pch-image[=write|check|load]`, is accepted and ignored with
  `warning: '-fkiln-pch-image' is ignored: the precompiled header
  snapshot replaced it [-Woption-ignored]`; `-fno-kiln-pch-image` is
  accepted silently. The snapshot covers everything the image carried,
  and the PCH is the same with or without the option. The module-file
  declaration image (`-fkiln-bmi-image`) is unchanged, and module files
  are byte-identical. `cargo xtask test --pch-diff` keeps `replay` and
  `snapshot`; `load` and `check` are gone with the image.

### Fixed

- PCH snapshot hardening (WP-1396, WP-1399). A prefix file that changes
  after the consumer's check (a same-size edit with its time put back,
  or any edit under `-fno-validate-pch`) ends the compile with `fatal
  error: file '<f>' has been modified since the precompiled header
  '<pch>' was built` instead of compiling against text it did not check;
  a damaged lookup log or section is refused and replayed, never
  misread. A pending `#pragma weak` (also `weak alias = target`) and a
  pending `#pragma redefine_extname` cross the PCH as they do with
  `-include`; a header that ends inside `#pragma clang attribute push`
  is an error, as in clang.
- Sema correctness (WP-1405). In C++ a variable length array parameter
  keeps its type, so an out-of-line member definition, a redeclaration or
  a friend whose VLA parameter differs no longer matches (as in clang). A
  lambda cannot implicitly capture a member of a block-scope anonymous
  union. A class or variable template partial specialization whose
  non-type parameter's type differs from the primary's no longer matches
  by converting the value ([temp.deduct.type]/20, as GCC). Calling a
  deprecated member template by name warns for the template, then the
  specialization. Clang's wording for partial pack lengths with explicit
  arguments, a deduced template that does not match, constraint notes on
  `sizeof...` and folds, `-Wpointer-arith` on pointers to a zero-size VLA,
  and the `aka` of a pointer to a typedef'd VLA.
- Sema correctness (WP-1359, WP-1334, WP-1386). `Z<TTs<Us>...>` in a
  member template of a class template with a template template parameter
  pack `TTs` is expanded (deduction, explicit arguments, return types,
  requires-clauses; mangled with clang's `_SUBSTPACK_`) instead of being
  rejected as not implemented. In a template, `this->sm` naming a
  `constexpr` static data member of dependent type that holds a
  reflection is a consteval-only leaf in each instantiation. A variably
  modified type is no template argument, so a coroutine with a variably
  modified parameter is an error as in clang. In C, two pointers to
  variable length arrays of compatible element types can be subtracted.
- Coroutine fixes (WP-1374, found by the hod project). A coroutine's own
  parameter objects are kept out of the coroutine frame, as clang 22
  does, so a parameter the ramp destroys after the frame is freed (a
  `trivial_abi` class, or one whose copy into the frame throws) no
  longer reads freed memory. The return object is kept out of the frame
  when a postcondition names it, so `post(r: ...)` checks it after the
  coroutine completed in its ramp. A variable length array in a
  coroutine is the error "variable length arrays in a coroutine are not
  supported" (as clang), and a call to `__builtin_alloca`, `alloca` or
  their aligned forms in a coroutine is an error too (stricter than
  clang, whose sema accepts it); both used to stop the build in LLVM
  ("Coroutines cannot handle non static allocas yet"). One LLVM bug
  remains: when `get_return_object()` throws, LLVM can keep the ramp's
  exception slots in the freed frame
  ([docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md)).
- A class-type (or array) non-type template argument with a pointer or
  reference subobject designating a string literal, `__func__`, a
  temporary or a `typeid` result (`X<T{"x"}>`, at any depth: members,
  bases, array elements, a union's active member) is now rejected, as
  [temp.arg.nontype]/6 requires and clang and GCC do (WP-1362; a
  `fixed_string` holding a `char[N]` copy stays valid). A top-level
  `__func__` argument is named "predefined '__func__' variable", as
  clang does.
- An error in a non-type template argument of a template-id formed by an
  instantiation is reported at the argument, not at a stale location of
  another template-id or at the template name (WP-1362).
- A reference template argument naming a bit-field is reported as
  "reference cannot bind to bit-field in converted constant expression"
  (clang's wording) instead of as binding a temporary (WP-1362).

## [1.1.0] - 2026-11-12

C mode, and the fixes made since 1.0.2. The gate results are in
[docs/status/release-1.1.0.md](docs/status/release-1.1.0.md).

### C mode

kiln compiles C. Its C behaviour is tested against clang (`-x c`),
diagnostic wording included. What is not done is in
[docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md#c-mode), and the
measurements are in [docs/status/c-mode.md](docs/status/c-mode.md).

- Drivers and dialects: `kiln` compiles `.c` (and `.h`, `.i`) as C, and
  `-x c` selects C under either driver. `kiln++` compiles a `.c` file as
  C++, with clang++'s deprecation warning. `-std=c89` (`c90`,
  `iso9899:1990`), `c99`, `c11`, `c17` (`c18`), `c23` (`c2x`), each
  with its `gnu` dialect; the default is `gnu17`, as in clang 19. C has
  its own lexing (no C++-only punctuators, user-defined literals or
  alternative tokens; no `//` comments in strict C89), C keywords
  (`_Imaginary` and `_Decimal32/64/128` are rejected as unsupported
  types, as in clang), each standard's identifier characters for UCNs,
  clang's C predefined macros and feature-test answers
  (`__has_c_attribute`; `__has_cpp_attribute` is C++ only), and C
  linkage (unmangled names, `fn.var` static locals),
  so C and C++ objects link together (WP-1275, WP-1290).
- Assembly inputs (WP-1331): `.s` (`-x assembler`) is assembled and
  `.S` / `.sx` (`-x assembler-with-cpp`) preprocessed by kiln in
  assembler-with-cpp mode (`__ASSEMBLER__`, `#` comments and `'` passed
  through) and then assembled, by `-c` or in a link, mixed with C and
  C++ inputs, with `-Wa,` / `-Xassembler` passed to the assembler (the
  LLVM installation's `clang++ -c -x assembler`). `-E`, `-S` and
  `-fsyntax-only` on a `.S` write the preprocessed text; on a `.s` they
  warn `'assembler' input unused`, as clang. Before, `-c x.s` warned
  `'linker' input unused`, wrote no object and exited 0.
- `<tgmath.h>` compiles in every C dialect (WP-1331): before C23 a
  function declared with only `...` is accepted when it is
  `overloadable`, as clang does; before, the header was rejected below
  `-std=c23`, including at the default `gnu17`.
- `-std=` takes every spelling clang 19 and 22 take, with their meaning
  (WP-1331): `-ansi` (`c89` for C, `c++98` for C++; the last of it and
  `-std=` wins), C94 `iso9899:199409` (`__STDC_VERSION__ 199409L`,
  digraphs), `c2y` / `gnu2y` (`202400L`), `iso9899:199x` and
  `iso9899:201x`; `iso9899:2024` is invalid at `-fclang-compat=19`, as in
  clang 19. Strict `-std=c89` no longer lexes digraphs. OpenCL and HLSL
  standards are "not allowed with 'C'" (or `'C++'`), and an invalid value
  lists the language's standards in notes, as clang does.
- `#embed` in C (WP-1331): no warning at `-std=c23`/`gnu23` and later
  (only `-Wpre-c23-compat`), and "#embed is a C23 extension" before C23,
  as clang 19/22; before, every C dialect warned "#embed is a Clang
  extension", which broke `-Werror` C23 builds.
- C23 `_BitInt` literal suffixes `wb` / `uwb` (WP-1331, any of clang's
  spellings: `WB`, `uWB`, `wbU`, ...): the type is the narrowest
  `_BitInt` holding the value (`0wb` is `_BitInt(2)`, `0uwb`
  `unsigned _BitInt(1)`), in code and in `#if`, with clang's
  "a C23 extension" warning before C23 and `-Wpre-c23-compat` from C23
  on; C++ still rejects them, as clang. Before, they were "invalid
  suffix". A literal over 128 bits is still "too large" on x86-64, where
  clang accepts it.
- C semantics, each where C differs from C++ (WP-1276, WP-1285 to
  WP-1287, WP-1301, WP-1311):
  - conversions: `void *` to and from any object pointer, integer and
    pointer conversions and incompatible pointer types diagnosed as
    clang does (`-Wint-conversion` is an error by default;
    `-Wincompatible-pointer-types` is an error by default, as in clang
    22, and a warning at `-fclang-compat=19`), pointer casts to narrower
    integers, `void *` and function-pointer arithmetic (GNU), C pointer
    comparisons and `?:` composite types, no C++ narrowing in brace
    initializers or case labels;
  - types: character literals are `int`, string literals are not
    `const`, `!`, comparisons and `&&`/`||` yield `int`, an enumeration
    with no negative enumerator promotes to `unsigned int`, `_Generic`
    matches by C compatibility;
  - declarations: the separate tag name space, tentative definitions
    (`-fcommon` and the `common` attribute), C99 and GNU89 `inline`
    (`-fgnu89-inline`), functions without a prototype, K&R
    definitions, implicit `int` and implicit function declarations
    (warnings in C89, errors from C99, as in clang), implicit
    declarations of the C library functions clang predeclares, no
    overloading (`__attribute__((overloadable))` keeps it), and
    prototype-scope tags and enumerators;
  - arrays and objects: variable length arrays, `[static N]`,
    `[restrict]` and `[*]` in array parameters, compound literals as
    lvalues (and file-scope compound literals as static objects),
    initialization of a `char` array by a string that exactly fills
    it, flexible array members initialized in static objects (GNU);
  - static initializers fold what clang folds (address constants
    through casts and offsets, pointer and label-address differences,
    IEEE floating results), and a C static is never initialized at run
    time: a non-constant initializer is clang's error;
  - statements: `goto` past an initialized declaration (but not into
    the scope of a VLA), and falling off the end of a non-void function
    without a trap (C only makes using the value undefined);
  - C99, C11 and C23 constructs: `_Bool`, `_Complex`, designated
    initializers, `_Static_assert`, `_Noreturn`, `_Alignas`, `_Alignof`,
    `_Thread_local`, `_Atomic`, anonymous structs and unions, C23
    `bool`/`true`/`false`, `nullptr`, `constexpr` objects, `auto`,
    `typeof`/`typeof_unqual`, `enum E : T`, `static_assert` without a
    message, `= {}`, `[[attributes]]` and `_BitInt(N)`.
- GNU C extensions, as clang implements them: statement expressions,
  `typeof`, `?:` with an omitted middle operand, case ranges and
  designator ranges, labels as values and computed `goto`, local labels
  (`__label__`), old-style `field:` designators, zero-length arrays,
  `transparent_union`, `cleanup`, `packed`, `__int128`, vector types,
  inline assembly and `__builtin_types_compatible_p` (WP-1276,
  WP-1287, WP-1288, WP-1290).
- What builds: five C projects with their own build systems pass their
  tests with kiln as the C compiler: zlib 1.3.2, sqlite 3.54.1, Lua
  5.4.8, c-ares 1.34.9 and zstd 1.5.7 (2026-11-11). GCC 16.1's
  `gcc.c-torture` suites at `-std=gnu23`, `-O0` and `-O2`: 6912 of 7572
  cases pass, against 6887 for clang 19 (2026-11-11, kiln at
  `-fclang-compat=19`). Every case kiln fails, clang 19 fails too
  (WP-1277, WP-1282, WP-1295, WP-1301, WP-1304).
- Diagnostics for the C-only rules are clang 22's, or clang 19's with
  `-fclang-compat=19`, including `-Wstrict-prototypes`,
  `-Wdeprecated-non-prototype`, `-Wincompatible-library-redeclaration`,
  `-Warray-parameter`, `-Wsizeof-array-argument` and
  `-Wsizeof-array-decay` (the last two in C++ too), C types printed as C
  (`struct S`, `_Bool`) (WP-1312).
- Linking: `kiln` links as `clang` does, so a C program does not pull
  in libstdc++ or libm; link C++ objects with `kiln++` (WP-1282).
- Precompiled C headers: `-x c-header` (or a `.h` input under `kiln`)
  writes a PCH that a C compile takes with `-include-pch` or by the
  `-include` probe (WP-1275, WP-1303).
- C mode matches clang on excess initializers (a -Wexcess-initializers
  warning, an error only under `-pedantic-errors`; the extra clauses are
  dropped and never evaluated), rejects `extern "C"` ("expected
  identifier or '('"), treats every `?:` as a non-lvalue, and rejects
  unary `+`, `-` and `~` of an array or function (WP-1318).

### Added

- Precompiled headers: kiln's PCH works as in 1.0 (`-x c++-header`,
  `-emit-pch`, `-include-pch`, and now `-x c-header`): by default the
  consumer validates the PCH and then replays the header's tokens, so a
  PCH build costs what the same build with `-include` costs. 1.1.0 adds
  an experimental serialized form behind
  `-fkiln-pch-image[=write|check|load]` (`-fno-kiln-pch-image`), off by
  default: the producer also writes the header's preprocessor state
  and, when it can carry every declaration of the header, a
  declaration image; a `load` consumer then builds the declarations
  from the image instead of parsing the header, and `check` builds both
  ways and reports the first difference. A header holding anything the
  image cannot carry, and every C header, gets no image, and its
  consumers replay it; the two paths are never mixed. The image
  does not yet carry most standard library headers, so real prefixes
  still replay ([docs/status/pch.md](docs/status/pch.md)). It is
  planned as the default for 1.2.0 (spec/driver.md "The declaration
  image", WP-1280, WP-1036, WP-1293).
- GNU extensions in C++ too: local labels (`__label__`), old-style
  `field:` designators (`-Wgnu-designator`),
  `__builtin_extract_return_addr`, `__builtin_frob_return_addr` and
  `__builtin___clear_cache` (WP-1288).
- Driver: `-rpath <dir>` is passed to the link; `-Wa,<args>` and
  `-Xassembler <arg>` take the integrated assembler's options clang
  takes (`--noexecstack` and others) (WP-1288).
- Profile-guided build of kiln itself: `cargo xtask build --pgo` and
  `cargo xtask dist --pgo` build instrumented, train on a fixed corpus
  from `bench/`, merge with `llvm-profdata` (rustc's LLVM major, found in
  kiln's LLVM or rustup's `llvm-tools`) and rebuild with the profile. That
  is about three times the build time, for a faster compiler with the
  same memory use. `cargo xtask pgo-profile --out <file>` is the pre-step
  alone, for a packaging system that runs its own `cargo build --release`
  with `-Cprofile-use=<file>`. `cargo xtask pgo-train` is the training
  alone. Nothing changes without the switch (README.md "Profile-guided
  build", WP-1308).
- `-fkiln-memory-report` has two exact rows for constant evaluation:
  `oven.heap-peak`, the most evaluation memory held at once in the
  translation unit (memory freed again during an evaluation still counts),
  and `oven.static-peak`. The total counts them instead of the capacity of
  the Oven's memory buffers, now shown as `oven.mem.capacity` outside the
  total (spec/oven.md "Memory report: the Oven's peaks", WP-1315).

### Changed

- The experimental declaration image of module files and PCHs
  (`-fkiln-bmi-image`, `-fkiln-pch-image`; both off by default) carries
  more of C++: locations in other files and in macro expansions,
  declaration attributes, linkage specifications, inline and unnamed
  namespaces, class, alias and variable templates with their
  specializations and member templates, friend templates, explicit
  instantiations, constraints, deduction guides, expressions in types
  and template arguments, block-scope `extern` declarations and
  `static_assert` in templates. Whatever it does not carry falls back
  to the 1.0 replay. `-fkiln-bmi-image-census` prints, per reason, what
  a unit's image would refuse (WP-1070, WP-1278, WP-1279, WP-1283,
  WP-1284, WP-1292, WP-1293, WP-1300, WP-1307, WP-1310).

### Fixed

- Compile time: a large braced initializer whose elements name
  constant variables (a generated table such as `{names[3], 4}, ...`
  over a `constexpr` array, or `{v, v, ...}`) was quadratic in its
  element count. Since 1.0.0 that included elements that subscript a
  constant array or access a member of one, so an 8,000-element constant
  table took 1.2 s instead of 0.5 s, and 16,000 elements 2.8 s; it is
  linear again (1.0 s at 16,000) (WP-1335).
- C++: unary `+` of a function (`+fn`) is a function pointer instead of
  an error, and `-`/`~` of an array or function report the decayed
  pointer type, as clang (WP-1318).
- Crash: a GNU binary conditional whose condition is a logical,
  relational or `!` expression (`(a || b) ?: c`) no longer stops code
  generation with the internal error "phi operand %N is defined", in C
  and C++ (WP-1288).
- Crash: an `asm` statement with a `_Complex` operand no longer crashes
  the backend; the operand is passed indirectly, as clang does
  (WP-1288).
- `__builtin_constant_p` of a value that is not a constant expression
  is left to the optimizer (`llvm.is.constant`), as in clang, so it is
  1 once inlining makes the argument constant. It was always 0
  (WP-1288).
- AArch64: the NEON table builtins `vqtbl2` to `vqtbl4`, `vqtbx1` to
  `vqtbx4`, `vtbl3`, `vtbl4` and `vtbx1` to `vtbx4` were "not supported
  in code generation". They are lowered as clang 19 lowers them
  (WP-1288).
- Parsing: a later declarator of a group may begin with GNU attributes
  (`void f(void), __attribute__((noreturn)) g(void);`), in a member
  declaration too (WP-1301). A compound literal after `sizeof` or
  `alignof` (`sizeof (struct P){ 0 }`) is the operand expression, not a
  type-id followed by a stray brace (WP-1287).
- Accepts-invalid:
  - a `static` namespace-scope variable after a declaration with
    external linkage (a block-scope `extern` included), and a
    non-`static` one after a `static` first declaration, are errors
    ([dcl.stc]/4); a block-scope `extern` declaration now redeclares a
    later declaration in any block of its namespace ([basic.link]/6)
    (WP-1293);
  - a function whose first declaration lacks `[[noreturn]]` while a
    later one has it is an error ([dcl.attr.noreturn]/1) (WP-1311);
  - a `reinterpret_cast`, or a C-style cast performing one, in the
    initializer of a `constexpr` or `constinit` variable is an error
    ([expr.const]/5.15). The same initializer of a variable that is
    neither is still static data, with no guard or dynamic
    initialization (WP-1295);
  - pointer arithmetic on a pointer to a variable length array (the GNU
    extension in C++) is not a constant expression (WP-1287).
- Module and PCH outputs: `--precompile -o /dev/null` or
  `-emit-pch -o /dev/null` (any path that exists and is not a regular
  file) is written in place, as clang does. Run as root, kiln replaced
  `/dev/null` with a regular file (WP-1281).
- Diagnostics: an error in an `#if` expression has clang's caret
  ranges (WP-1290).
- **Source-compatibility note: previously accepted code is now
  rejected.** A run-time use of an expansion-statement (`template for`)
  element variable whose type holds a reflection was accepted:
  `template for (constexpr auto e : plan) std::puts(e.name);` with
  `struct E { std::meta::info r; const char* name; }`. It is ill-formed
  (P2996R13 [expr.const]: `e` is an expression of consteval-only type
  outside a constant-evaluated context), as the same use of a plain
  `constexpr E e = plan[0];` already was, and is now rejected with the
  same "consteval-only expressions are only allowed in a
  constant-evaluated context" in every expansion form, for `constexpr
  const auto&` and `constexpr auto&&` element variables, for a
  structured-binding element declaration, and in function templates
  (also for a `constexpr T x` of a function template instantiated with
  such a class, and a binding of a `constexpr` structured binding of
  one). GCC 16.1 rejects it too. Write `constexpr const char* name =
  e.name; std::puts(name);` instead: the read is then inside a constexpr
  variable's initializer. No file in kiln's own tree relied on the old
  acceptance (WP-1330).

### Testing and tooling (not user-visible)

- The OSS harness builds C with kiln (`kiln-cc`, a per-project `c` key,
  per-language TU counts), with the five C rows above (WP-1277).
  `cargo xtask gcc-torture` runs GCC 16.1's `gcc.c-torture` suites
  through kiln, GCC and clang and writes the generated tables of
  docs/status/c-mode.md (WP-1277, WP-1282). The `tests/c/` corpus has
  clang 19 `-x c` as its oracle (WP-1275).
- PCH: `cargo xtask test` and `cargo xtask libcxx` take
  `--pch-diff load|replay|check`, a differential of the image and the
  replay path, run over a corpus slice in `cargo xtask ci` (WP-1036).
  `cargo xtask bench bench/pch` measures kiln's PCH against clang 19's
  and writes docs/status/pch.md, judging kiln's accounted memory rather
  than peak RSS (WP-1294, WP-1303).
- `cargo xtask bench` judges every kiln entry of `frontend`, `oven`,
  `overload` and `reflect` on kiln's accounted memory (the
  `-fkiln-memory-report` total, deterministic) at 2%, from one extra
  unmeasured compile, beside the RSS gate; the table shows `acct MB` and
  a `judged` cell per row. A kiln version bump no longer makes kiln's own
  baselines advisory: since the 1.0.0 bump every main-group comparison
  had been advisory and no gate judged anything. The four groups are
  re-recorded. A reflection shape's `--gcc-toolchain=` path is recorded
  as a placeholder (spec/bench.md, WP-1315).

## [1.0.2] - 2026-11-11

A bug-fix release: fixes found while building downstream code,
each with a test. Nothing in it changes what
[docs/STABILITY.md](docs/STABILITY.md) promises.

### Fixed

- Wrong code: the default member initializers of a member class template
  were lost when a member declaration of the enclosing class instantiated
  the template before the end of that class
  (`struct Outer { template <class T> struct F { bool set{false}; T v; }; F<bool> f; };`).
  A constructor or aggregate initialization then left those members
  uninitialized. The initializer is now found at its first use; a use
  before it is parsed is the error it is for a non-template class
  ([class.mem.general]/8).
- Linking against libstdc++: a member explicitly defaulted on its first
  declaration is defined where it is used even when its class has an
  explicit instantiation declaration, as clang does (`linkonce_odr`).
  `std::filesystem` directory iteration failed to link without it.
- Debug info: a constexpr member of an explicitly instantiated class
  template (`template class std::vector<T>;`) could be emitted twice
  when an earlier member called it, and with `-g` LLVM then dropped the
  translation unit's debug info.
- Debug info: an explicit instantiation definition of a class template
  (`template struct V<int>;`) completes the specialization's type and
  keeps it in the compile unit's retained types, as clang does. The type
  was described only if a member that referred to it was emitted.
- Linking: a function called in the operand of `typeid` whose result is a
  glvalue of polymorphic class type is odr-used, so it is defined or
  instantiated: `typeid(*p)` with a class template's member `operator*`
  no longer leaves an undefined symbol at link time. A non-polymorphic
  operand still names nothing it odr-uses ([expr.typeid]/3).
- Overload resolution: a comparison whose regular and reversed rewritten
  candidates tie (`a < b` through `operator<=>(const Base&)`, or `a == b`
  through the pre-C++20 `operator==(const Base&)`) is a substitution
  failure in a requires-expression, a concept or a SFINAE'd signature, as
  in clang and GCC; outside SFINAE it is still accepted with
  `-Wambiguous-reversed-operator`.
- Lookup: the namespace-name of a using-directive or of a namespace alias
  is looked up among namespaces only ([basic.lookup.udir]). A class of
  the same name in a nearer scope no longer gives "expected namespace
  name".
- Constraints: a function's constraints are checked from the function, so
  a hidden friend's trailing requires-clause may name a private member of
  the befriending class. Such a friend was dropped from overload
  resolution, and a deduced-return template was chosen in its place.
- Placement new: a placement parameter of type reference to a base class
  accepts an argument of a derived class
  (`operator new(std::size_t, Allocator &)` with a derived allocator).
- Abstract classes: binding a reference to an abstract class from an
  object of an unrelated class through a converting constructor is
  ill-formed ([class.abstract]/3), and a substitution failure where one
  is allowed. GoogleTest's printer selection relied on it.
- Parsing: in `X<args>::name`, `name` is classified in the explicit
  specialization the template-id names when every argument is a decimal
  literal or `sizeof(name)`. Nested `BOOST_SCOPE_EXIT` blocks need it.
- Warnings:
  - a comparison or conditional expression with an unnamed enumeration
    warns under `-Wdeprecated-anon-enum-enum-conversion`, as in clang, so
    `-Wno-deprecated-anon-enum-enum-conversion` silences it;
  - `-Wcomment` no longer reports a multi-line `//` comment when the
    spliced-in line is itself a comment.

## [1.0.1] - 2026-11-11

A bug-fix release. Nothing in it changes what [docs/STABILITY.md](docs/STABILITY.md)
promises.

### Fixed

- Reflection mangling (the ABI note in
  [docs/STABILITY.md](docs/STABILITY.md#covered-stable-within-a-major-version)): two
  different dependent reflected template-ids could get the same symbol
  (`^^AT<T>` and `^^TT<T*>`, and `^^F<T,int>` and `^^F<T,long>` in gcc
  mode). They no longer collide (WP-1250). GCC 16.1's spelling is used
  wherever it cannot collide. Three of its spellings lose information,
  and GCC itself fails to assemble the resulting overload pairs: a
  dependent function template-id without its arguments, a qualified
  variable template-id without its qualifier, and an alias template-id
  whose aliased type does not pin down the alias's arguments. kiln keeps
  its own spelling in those three cases, so the names differ from GCC's
  there; spec/abi.md §8.14 lists them. A reflected alias whose parameter
  appears only in a non-deduced context now gets GCC's `ta` form when
  its argument is dependent, so it links with g++-16 (WP-1260). A value-dependent reflection is no longer evaluated before
  instantiation (WP-1260).
- A consteval-only reference in a template is checked again in each
  instantiation. A dependent initializer such as
  `constexpr bool b = m == members[0];` in a `template for` body is no
  longer rejected (WP-1263, P3032R2).
- A failed `consteval` block in a lambda body leaves the lambda valid
  (WP-1257). A failed `static_assert` in a lambda body no longer stops
  evaluation, so a second refusal is reported too (WP-1247).
- Diagnostics:
  - When a reflection query triggers an instantiation error, the
    diagnostic names the consteval call frames (WP-1248). Those frames
    still show under copy elision and nesting, and spec names keep their
    template arguments (WP-1258).
  - Text the program provides (an uncaught exception's `what()`,
    attribute messages, reflected names) is escaped as clang escapes it
    (WP-1259).
  - An uncaught exception's `what()` keeps the characters read before a
    fault, and its length cap does not split a UTF-8 sequence (WP-1251).
  - An invalid escape in an unevaluated string (a `static_assert`
    message, `[[deprecated]]`, `[[nodiscard]]`, `extern "…"`, asm) no
    longer causes follow-on errors. The construct is dropped, as in
    clang 22. Escape diagnostics quote the whole escape and use clang 22's
    source ranges (WP-1268).
  - Column widths and the escaping of unprintable characters use LLVM
    22's Unicode 15.1 tables, so an unassigned code point is shown as
    `<U+XXXX>` as clang shows it (WP-1268).
- Invalid UTF-8 in an ordinary string or character literal is the
  `-Winvalid-source-encoding` warning, not an error, as in clang
  (WP-1267).
- Under `-fkiln-source-path-style=gcc`, header paths are spelled as
  g++-16 spells them (the `-I` directory exactly as given, `"../x.h"`
  relative to the includer). This is the name that `source_location` and
  `std::meta::source_location_of` report (WP-1262).
- C++20 modules: the experimental declaration image
  (`-fkiln-bmi-image`) now covers imports, partitions and
  using-declarations (WP-1068).

### Testing and tooling (not user-visible)

- The harness can run GCC 16.1 as the compiler under test
  (`--compiler $KILN_GCC`). It runs clean over the reflection and
  contracts tests (WP-1249), and every harness command maps its flags for
  GCC (WP-1261, WP-1266). Test compiles pass `-fno-crash-diagnostics`
  (WP-1264).
- The clang oracle retries once when a compile dies on a signal
  (WP-1254).
- The sweep workflow's `oss` job fails when it produces no report
  (WP-1241). It runs as one `oss-matrix` job over the `sweep = true` rows
  of `projects.toml` (WP-1265).
- `release.yml` uploads the dist archives from the workspace, and can be
  started by hand as a dry run (WP-1253).
- Spec notes with pinning tests: `type_of(reflect_constant(^^int))`
  (WP-1255), and `reflect_constant` of a function reflection's value,
  which instantiates nothing (WP-1252).

## [1.0.0] - 2026-11-10

The first release. This section summarises what kiln 1.0 does. Measured
results are in [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md), and the
limits are in [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md).

### Language

- C++98 through C++23, and C++26 as in the Croydon DIS draft: 260 of the
  261 rows of [docs/STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md) are
  done. The one partial row is P2786 (trivial relocatability), which was
  removed from C++26 at Kona and is not in the DIS.
- C++26 static reflection (P2996, `<meta>`), on by default at
  `-std=c++26` (`-fno-reflection` turns it off). The `<meta>` header is
  GCC 16.1's libstdc++ one, selected with `--gcc-toolchain=`; libstdc++
  14 and libc++ 22 do not ship it. When `#include <meta>` is not found at
  C++26, a note names the GCC installation (or libc++) kiln selected, and
  a GCC 16 found through `g++-16` or `g++` on `PATH`.
- C++26 contracts (P2900): `pre`, `post`, `contract_assert`, the four
  evaluation semantics (`-fcontract-evaluation-semantic=`), and a
  replaceable violation handler.
- C++26 erroneous behaviour (P2795): at `-std=c++26` an uninitialized
  automatic variable (up to 4096 bytes) is zero-filled at its
  declaration. clang 22 does not do this. `[[indeterminate]]` on the
  variable, or `-ftrivial-auto-var-init=uninitialized`, opts out.
- C++20 modules, header units and `import std;`, in one invocation or in
  clang's two-step build: `--precompile`, `-fmodule-output`,
  `-fmodule-file=`, `-fprebuilt-module-path=`. kiln's module files are
  its own format. The declaration image (`-fkiln-bmi-image`, which builds
  an imported module from a serialized image instead of replaying it) is
  experimental and off by default; it does not yet cover `import std;`.
- Dependency scanning: `kiln-scan-deps` (also installed as
  `clang-scan-deps-22` and `clang-scan-deps`) writes P1689r5, so CMake
  3.28+ with Ninja builds C++20 module targets unchanged.
- Coroutines, exceptions, RTTI, `thread_local`, and clang's and GCC's
  common extensions and builtins.

### Constant evaluation (the Oven)

- All constant evaluation (`constexpr`, `consteval`, `static_assert`,
  template arguments, reflection) runs in the Oven, kiln's bytecode
  evaluator. It has exact per-evaluation budgets for steps, depth and
  live memory (`-fconstexpr-steps=`, `-fconstexpr-depth=`,
  `-fconstexpr-memory=`), and memory freed during an evaluation is
  reclaimed during that evaluation.
- On kiln's benchmark corpus, compared with clang 19: geometric mean
  0.21x the time and 0.17x the peak memory
  ([docs/status/perf.md](docs/status/perf.md)).
- Reports: `-fconstexpr-report=` (NDJSON per evaluation),
  `-fkiln-instantiation-report`, `-fkiln-memory-report`.

### Standard libraries and compatibility

- libc++ 22.1.8 test suite at `-std=c++26`: 99.4%. libc++ 19.1.7: 98.3%.
  Every public libstdc++ 14 header compiles at `-std=c++20` and
  `-std=c++26`.
- 47 open-source projects built with their own build systems and test
  suites, including all of LLVM, Boost, folly, Abseil, DuckDB and CMake:
  46 at parity with clang 19 or better on every test; the Boost
  metaprogramming libraries match clang 22.
- clang-compatible command line and predefined macros. CMake and
  autotools work unchanged. kiln reproduces clang 22 by default and
  clang 19 with `-fclang-compat=19`.

### Code generation and tools

- Writes LLVM IR as text and drives LLVM's tools. It never links against
  LLVM. Supports LLVM 22 (preferred) and LLVM 19, found on its own
  (`--llvm-dir=` and `KILN_LLVM_DIR` name one); `--version` and `-v`
  print the LLVM in use.
- Itanium C++ ABI as clang implements it, on Linux aarch64 and x86_64.
- `-O0` to `-O3`, `-Os`, `-Oz`; LTO (`-flto`, `-flto=thin`);
  floating-point model flags; AArch64 branch protection.
- DWARF debug information (`-g` and its levels, `-gdwarf-N`) that gdb and
  lldb read as they read clang's.
- Sanitizers: address, hwaddress, thread, memory, leak, fuzzer, and
  undefined behaviour.
- Coverage: `--coverage` (gcov) and `-fprofile-instr-generate`
  counters. Coverage mapping and PGO are not in 1.0.

### Robustness and distribution

- An internal compiler error prints a report and writes a reproducer
  (`.ii` and `.sh`) instead of a panic
  ([docs/REPORTING-BUGS.md](docs/REPORTING-BUGS.md)). kiln runs out of
  stack only as a diagnostic.
- Every crate is `#![forbid(unsafe_code)]`. The MSRV is Rust 1.95.
- Release archives from `cargo xtask dist`: relocatable,
  `kiln-<version>-<triple>.tar.xz` with a stripped binary, plus a
  `-symbols` archive and `SHA256SUMS`.

### Fixed in the final release candidate

These landed after the first 1.0 release candidate was measured.

- x86 builtins: every x86 builtin that clang lowers to generic IR is now
  lowered (AVX and AVX2 extract, insert and `perm2x128`, F16C, the FMA
  family and more). The builtins kiln declares but cannot lower went
  from 595 to 23 (`maskmovq`, Key Locker, the AMX internals), and those
  23 answer `__has_builtin` with 0. A call of an x86 builtin sets
  `min-legal-vector-width`, which fixes an LLVM backend crash at
  `-march=skylake-avx512 -O2`. OpenCV's AVX2 dispatch TUs now compile
  (WP-1242).
- Reflection: `extract` of a function now instantiates and odr-uses it
  (WP-1244).
- Constant evaluation: an uncaught exception in a constant evaluation
  prints its `what()` whole, up to 64 KiB, after which it ends with
  "… [N more characters]" (WP-1245).
- Reflection: `^^` accepts operator, literal-operator, conversion and
  dependent template-ids, such as `^^ns::operator==<int>` (WP-1246).
- Reflection: `members_of` leaves out members whose type is an undeduced
  placeholder, and `type_of` on one throws, as in GCC.
  `display_string_of` prints requires-clauses as GCC 16.1 does
  (WP-1243).

## Before 1.0

Before 1.0 kiln was developed in the open on `main`, at version 0.0.1,
with no tagged releases. The history is in the git log, the milestones
in [docs/ROADMAP.md](docs/ROADMAP.md), and each work package's record in
[docs/BACKLOG.md](docs/BACKLOG.md) and `spec/landed/`.

[1.0.2]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.2
[1.0.1]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.1
[1.0.0]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.0
