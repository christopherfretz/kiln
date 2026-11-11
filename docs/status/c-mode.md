# C mode status

Where `kiln` (the C driver) stands on real C code, measured by WP-1282 on
2026-11-11 as the input for the wave-2 C work packages (WP-243). Two builds
were measured:

- **main**: kiln at `5a58f96ab` (wp/1282, which is origin/main `cc2f76a8f`
  plus the measurement fixes below). It has the WP-1275 C front end
  (language bit, lexing, linkage) but none of the WP-1276 C semantics.
- **wp/1276**: the in-flight WP-1276 branch (conversions, tentative
  definitions, the tag namespace, K&R and implicit int, and more), built
  read-only into a private target dir: the OSS rows at `80e415482`, the
  torture run and every repro at `dd6ba3cf7`. It shows what is left once
  WP-1276 lands; that remainder is the wave-2 list.

The oracles are clang-19 (`clang-19 -x c`, the control for every OSS row) and
GCC 16.1 (torture only). Re-run everything with the commands in each section;
`cargo xtask gcc-torture` fetches the GCC tests outside the repo.

## Summary

- Current (WP-1295, 2026-11-11): all five C OSS rows (zlib, sqlite, lua,
  c-ares, zstd) build with kiln and pass their tests; see "C OSS rows". The
  bullets below are the WP-1282 baseline.
- C mode on main compiles roughly 71% of gcc.c-torture cases at gnu23
  (clang-19: 91%). With wp/1276 that rises to 86%, and the "wrong" class
  falls from 67 to 36 cases, 34 of which clang-19 gets wrong too.
- No C OSS row builds on main. With wp/1276, zlib is fully green (37/37 TUs,
  6/6 tests) and sqlite3.c compiles; lua, zstd and c-ares each stop on a
  single construct that WP-1276 does not cover (comparing a function pointer
  with `(void *)0`, and qualifiers inside a parameter's `[ ]`).
- grpc and simdjson keep `c = "clang"`: their C TUs do not all compile (no
  flip re-landed).
- Miscompiles and crashes, ranked first in every list below:
  1. falling off the end of a non-void C function traps or is unreachable
     on main (C allows it if the caller does not use the value). wp/1276
     covers it at `dd6ba3cf7`;
  2. an ICE on the GNU `a ?: b` with a logical condition, not covered by
     wp/1276.

## WP-1286 re-measure (wp/1286, 2026-11-11)

- gcc.c-torture, gnu23, `-O0,-O2`, kiln on main `92fc1f18e` against
  wp/1286 `052e337a6` (before the merge of WP-1288):
  - pass 6480 to 6646 of 7572 (clang-19: 6887); compile-reject 1035 to
    867; crash 19 to 21; wrong 36 to 36; timeout 2 to 2. No case regresses;
  - the 2 new crashes are compile/pr33382.c at both options: it used to be
    rejected at its flexible array initializer, and now reaches the GNU
    `?:` ICE that WP-1288 fixes.
- The kiln failures that clang-19 passes, by cluster:

| cluster | before | after | example |
|---|---|---|---|
| C++ narrowing in brace initializers and case labels | 58 | 0 | compile/20000326-1.c |
| exact-fit string initializers | 12 | 0 | execute/20010924-1.c |
| goto past an initializer | 12 | 0 | compile/irreducible-loop.c |
| redefinition | 12 | 2 | compile/pr46866.c (`transparent_union`, `tests/c/decl/transparent-union-redeclaration.c`, xfail) |
| tentative arrays | 8 | 0 | |
| array init from a string or compound literal | 8 | 0 | |
| `void main` | 8 | 0 | |
| flexible array member initialization | 8 | 0 | |
| incomplete type at a tentative definition | 6 | 0 | |
| qualified anonymous bit-fields and members | 4 | 0 | |
| forward-referenced enums | 4 | 0 | |
| other | 287 | 259 | |
| total | 427 | 261 | |

- sqlite 3.54.1 (`--compiler kiln`, harness build of `052e337a6`):
  configure ok. Both C TUs now compile: shell.c's narrowing initializers
  are gone, and sqlite3.c takes 32 s at 0.74 GB peak RSS. The `sqlite3` link
  fails on 3 block-scope statics whose initializers contain pointer casts
  (`trimFunc`, `sqlite3Initialize`, `sqlite3RegisterJsonFunctions`). Those
  casts do not fold, so the statics are initialized under a `__cxa_guard`
  that a C link cannot resolve. Repro:
  `tests/c/run/static-local-address-constant-cast.c` (xfail). The
  workload was not run.
- After the measurement (not re-measured):
  - K&R definitions whose declaration list starts with `const` now compile:
    compile/20040309-1.c, compile/980825-1.c and execute/930513-2.c.
    C parses no function qualifiers.
  - pr34334 (a file-scope anonymous union) compiles after the merge of
    main.
  - zstd's last failing C TUs failed on a function pointer compared with
    `(void *)0` ("comparison of distinct pointer types"), not on a
    declaration. WP-1285's `c_pointer_comparison` accepts it. On the
    merged tree, zstd v1.5.7 (`--compiler kiln`) builds (rc 0), and its
    tests pass (rc 0).

## WP-1288 re-measure (wp/1288, 2026-11-11)

- gcc.c-torture, gnu23, `-O0,-O2`, kiln on main `92fc1f18e` (WP-1276
  landed) against wp/1288 (`58a4203a8`, before the `_Complex` asm fix):
  - pass 6479 to 6534; compile-reject 1035 to 1000; crash 19 to 1; wrong 36
    to 34; timeout 3 to 3. 55 cases flip to pass (35 reject, 18 crash, 2
    wrong) and none regress;
  - the last crash (compile/20000804-1.c -O0, a `_Complex` `+r` asm
    operand) now gets clang's IR and clang's backend error;
  - by suite after: execute -O0 1480, -O2 1470; ieee 65 and 66; builtins
    32 and 27; compile 1697 and 1697.
- Fixed: the GNU `?:` ICE, `__label__`, GNU array-range and `.f =`-less
  designators, enumerators and tags in a C member list, K&R identifier
  lists, `__builtin_{extract,frob}_return_addr`, `__builtin___clear_cache`,
  NEON `vqtbl{2,3,4}`/`vqtbx`/`vtbl`/`vtbx`, `__builtin_constant_p` through
  `llvm.is.constant`, `-rpath DIR`, `-Wa,` and `-Xassembler`
  (spec/landed/{codegen,driver,sema}/WP-1288.md).
- OSS rows (`--compiler kiln`, harness build of wp/1288):
  - zlib v1.3.2: configure, build and tests ok; 37/37 C TUs, 8/8 links,
    6/6 tests;
  - zstd v1.5.7: configure ok, build fails, 0/3 tests; 48/74 C TUs. All
    26 failures are a function pointer compared with `(void *)0`
    (`zstd_compress_internal.h:1614` and the trace/alloc hooks;
    `tests/c/expr/fnptr-null-pointer-compare.c`, xfail).
    `-Wa,--noexecstack` is now accepted in its 78 commands. The ASM
    compiler identification is still "unknown" (see Driver follow-ups).
- grpc and simdjson stay `c = "clang"`:
  - simdjson: 4/5 C TUs; `cJSON.c:182` is the same function pointer
    against `(void *)0` compare;
  - grpc: the `vqtbl2q` blocker is fixed, but c-ares's configure probes
    still cascade on qualifiers inside a parameter's `[ ]`
    (`tests/c/decl/array-param-qualifiers.c`, xfail, WP-1287).

## C OSS rows

`scripts/oss/oss-build.sh <row> --compiler kiln` (each row sets `c = "kiln"`)
against `--compiler clang`, re-measured by WP-1295 on 2026-11-11 with the
harness build of wp/1295 at `e54b84796` (origin/main `c60f69910` merged,
WP-1286 included; sqlite at `38398d21b`, before that merge, and again at
the handback head, below). "TUs" are the build-phase TUs of the report
(`tus`, C and the C++ test drivers); "probes" counts configure-phase
compiles and how many fail.

| row (tag) | configure | build | tests | TUs | probes (fail) | clang-19 |
|---|---|---|---|---|---|---|
| zlib (v1.3.2) | ok | ok | 6/6, minigzip round trips ok | 37/37 | 19 (0) | 37/37, 6/6 |
| sqlite (3.54.1) | ok, probes match clang | ok | workload ok | 2/2 | 45 (4, the same 4 as clang) | 2/2, workload ok |
| lua (v5.4.8) | n/a (make) | ok | all.lua (`_U`) "final OK" | 34/34 | 0 | 34/34, ok |
| c-ares (v1.34.9) | ok | ok | ctest 2/2, arestest 1199 passed | 126/126 | 311 (29) | 99/99 C + 27 C++, 2/2 |
| zstd (v1.5.7) | ok, ASM identification unknown | ok | 3/3, playTests ok | 118/118 | 65 (9) | 118/118, 3/3 |

All five rows build with kiln and pass their tests: the C OSS bar of
1.1.0 is met. The failing probes are platform probes clang fails too
(sqlite: one `_x` negative-size probe and three expected
undefined-reference function probes; c-ares: missing headers such as
`sys/sockio.h`, `SOCKET` and `closesocket`, and `res_servicename`,
`inet_net_pton` and `__system_property_get`).

First blocker per row, and what cleared it:

- **zlib**: none since WP-1276.
- **sqlite**: three block-scope statics whose initializers are address
  constants through pointer casts (`trimFunc`, `sqlite3Initialize`,
  `sqlite3RegisterJsonFunctions`) were guarded with `__cxa_guard`, which a
  C link cannot resolve. WP-1295 folds them (spec/oven.md "Address
  constants") and makes a non-constant C static initializer an error
  (`tests/c/run/static-local-address-constant-cast.c`,
  `tests/c/run/static-address-constant-forms.c`,
  `tests/c/decl/static-init-not-constant.c`). The `case 0xca093fa0:` in
  `shell.c` was cleared by WP-1286. The `-lm` probe ("Checking libs for
  ceil...-lm") and `-rpath /usr/local/lib` (WP-1288) now match clang.
  Three of the expected function-probe failures still print `ld: DWARF
  error: mangled line number section (bad file number)` before the
  undefined reference; this is cosmetic (see the WP-1282 note).
- **lua**: the function pointer compared with `NULL` (`ldebug.c:135`) was
  cleared by WP-1285; no new blocker.
- **c-ares**: the qualifiers inside a parameter's `[ ]` (glibc
  `netdb.h:690`) were cleared by WP-1287; configure no longer cascades
  and no new blocker appeared.
- **zstd**: green since WP-1286. CMake's ASM compiler identification is
  still "unknown"; the build does not need it.

Compile cost (perf record, not a regression): sqlite's two TUs at `-O2`
(the row's `CFLAGS`, autotools `make`; kiln and kiln++ from the harness
profile, clang-19 the control; `.oss/build/sqlite/<compiler>/compile.jsonl`):

- `sqlite3.c`: kiln 33.6 s, 0.73 GB peak RSS; clang-19 30.6 s, 0.92 GB;
- `shell.c` (with `sqlite3.c` linked in one command): kiln 39.2 s,
  0.74 GB; clang-19 35.3 s, 0.93 GB.

## grpc and simdjson C halves (`c = "kiln"`, from 95ae39b29)

Both rows stay `c = "clang"`. The flip was measured with 95ae39b29 applied
locally and is **not** re-landed:

- **simdjson v4.2.4**: C 0/5 TUs (`void *` and `NULL` conversions, covered
  by wp/1276); C++ 134/134; tests 117/117 (they do not depend on the C TUs).
  The build exits 2 on the C TUs.
- **grpc v1.85.1**: configured with kiln-cc for C and clang++-19 for C++. Each
  of the 589 unique C files in `compile_commands.json` was replayed through
  kiln and through clang-19.
  - kiln compiles 21. clang compiles 498; the 90 clang also fails are c-ares
    TUs that inherit kiln's broken configure probes (the timeval cascade
    above).
  - Clusters:
    - 443 `third_party/upb/upb/mem/alloc.h:44`, "cannot initialize a
      parameter of type 'size_t *' with an rvalue of type 'void *'"
      (`NULL` passed for a pointer parameter; covered by wp/1276);
    - 90 c-ares timeval;
    - 17 `void *` return and 5 `void *` assignment (covered);
    - 4 old-style call (`address_sorting_posix.c:62`, covered);
    - 2 `__builtin_neon_vqtbl2q_v` not supported in codegen
      (`third_party/utf8_range/utf8_range.c` via `arm_neon.h:56634`; C++ is
      affected too, and wp/1276 does not cover it);
    - 2 `register` (`third_party/zlib/deflate.c:1350`, covered).
  - Slowest kiln TU 0.28 s, against 0.45 s for clang.

## gcc.c-torture

`cargo xtask gcc-torture --suite execute,ieee,builtins,compile --opt -O0,-O2`
at the default `-std=gnu23` and again with `--std gnu17`, 3813 tests and 7572
cases. The generated regions at the end of this page hold the full tables. The
wp/1276 row is the same command with `--compiler <wp/1276 build>/kiln
--oracles none --tsv <file>`.

### Pass rate by class (cases of 7572)

| run | pass | compile-reject | crash | wrong | timeout |
|---|---:|---:|---:|---:|---:|
| kiln main, gnu23 | 5355 (70.7%) | 2125 | 20 | 67 | 5 |
| kiln main, gnu17 | 5353 (70.7%) | 2127 | 20 | 67 | 5 |
| kiln wp/1276 `dd6ba3cf7`, gnu23 | 6480 (85.6%) | 1035 | 19 | 36 | 2 |
| clang-19, gnu23 | 6887 (91.0%) | 648 | 0 | 37 | 0 |
| clang-19, gnu17 (WP-1277 baseline) | 7057 (93.2%) | 478 | 0 | 37 | 0 |
| GCC 16.1, gnu23 | 7572 | 0 | 0 | 0 | 0 |

By suite and option set at gnu23, kiln main against clang-19 (GCC passes
every case):

- execute -O0: 1281 against 1540 (of 1677); execute -O2: 1274 against 1529;
- ieee -O0: 59 against 64 (of 69); ieee -O2: 60 against 65;
- builtins -O0: 21 against 35 (of 59); builtins -O2: 17 against 28;
- compile -O0: 1321 against 1813 (of 1981); compile -O2: 1322 against 1813.

kiln fails where clang-19 and GCC both pass in 1553 cases. kiln passes where
clang fails in 21 cases, all of them clang's own wrong or reject results.

### Crashes and wrong results (main, gnu23)

Wrong results come first: each is a miscompile.

1. **Falling off the end of a non-void function** (36 of the 37 wrong cases
   where clang passes). C (6.9.2p12) makes it undefined only if the caller uses the
   value; kiln emits the C++ trap at -O0 (SIGTRAP) and `unreachable` at -O2
   (wrong exit, SIGSEGV or a run timeout: 20010122-1, pr47925, pr65956).
   Example: execute/20000717-1.c. Covered by wp/1276 at `dd6ba3cf7` (36 of
   the 37 pass; the other is bcp-1.c -O2, item 2). Repro:
   WP-1276's `tests/c/run/fall-off-end.c` (and `fall-off-end-c89.c`).
2. execute/bcp-1.c -O2: `__builtin_constant_p` of a parameter of an inlined
   function folds to 0 in the front end, where clang emits `llvm.is.constant`
   and lets the optimizer decide. The result is legal, so this is a note, not
   a miscompile. execute/builtin-constant.c -O2 is the same issue under
   wp/1276.
3. A wp/1276 regression at `80e415482` (execute/20071202-1.c, a compound
   literal assigned over the object it reads, and two others) was reported
   and is fixed at `dd6ba3cf7`, which has no regression against main.

Crashes:

1. **ICE "phi operand %N is defined"** for GNU `a ?: b` whose condition is
   `||` or `&&` (18 cases: pr54103-1..4, pr42716, pr44119, pr89655,
   pr111414-1, execute/pr68249). C++ crashes too. Not covered by wp/1276.
   Repros: `tests/c/expr/gnu-binary-conditional-logical.c`,
   `tests/conformance/expr.cond/gnu-binary-conditional-logical-run.cpp`.
2. compile/pr65680.c (2 cases): `struct S { int f; } a[100000000000000001][3]
   = {0};` hits the out-of-memory abort instead of a diagnostic or a sparse
   initializer. clang-19 compiles the C form. clang++-19 crashes on the C++
   form, so the repro is inline here rather than a corpus test (the harness
   never masks an out-of-memory kill with xfail).

Timeouts: compile/limits-structnest.c (deeply nested structs; clang rejects it
on its bracket-nesting limit, GCC compiles it) and the three -O2 falloff run
timeouts above (they pass on wp/1276 at `dd6ba3cf7`).

### Failure clusters, ranked

Cases where kiln main fails at gnu23 and clang-19 passes, grouped by kiln's
first diagnostic. "wp/1276" gives how many of those cases pass at
`dd6ba3cf7`: yes means at least 90%, partial means some, no means none.
"now wrong" counts the cases that compile under wp/1276 but run wrong.

| cases | construct | kiln's first diagnostic | example | wp/1276 |
|---:|---|---|---|---|
| 502 | K&R definitions (`f(a, b) int a;`: identifier-list parameters parsed as type names); 10 are GNU `__label__` | unknown type name '…' | compile/20000326-2.c | partial (452/502) |
| 157 | implicit int | a type specifier is required for all declarations | compile/900313-1.c | partial (135/157) |
| 114 | call through an old-style (unprototyped) declaration | no matching function for call to '…' | compile/20030518-1.c | yes (109/114; 1 now wrong) |
| 76 | `void *` and pointer assignment conversions | incompatible pointer types assigning to '…' from '…' | compile/20070529-2.c | partial (68/76) |
| 72 | `register` storage class | ISO C++17 does not allow 'register' storage class specifier | compile/20010112-1.c | yes (68/72) |
| 61 | pointer conversions in initializers (lvalue) | cannot initialize a variable of type '…' with an lvalue of type '…' | compile/20030904-1.c | yes (59/61) |
| 55 | pointer conversions in initializers (rvalue) | cannot initialize a variable of type '…' with an rvalue of type '…' | compile/20040121-1.c | yes (51/55) |
| 46 | implicit function declarations, GNU builtins | use of undeclared identifier '…' | compile/20000224-1.c | partial (24/46) |
| 37 | falling off a non-void function (miscompile) | wrong result (SIGTRAP at -O0) | execute/20000717-1.c | yes (36/37) |
| 26 | pointer cast to a narrower integer | cast from pointer to smaller type '…' loses information | compile/20000105-2.c | no |
| 26 | C++ narrowing applied to C brace init (non-constant) | non-constant-expression cannot be narrowed … in initializer list | compile/icfmatch.c | no |
| 26 | arithmetic on `void *` (GNU) | arithmetic on pointers to void / on a pointer to void | compile/20021108-1.c | no |
| 26 | C++ narrowing applied to C brace init (constant) | constant expression evaluates to N which cannot be narrowed | compile/pr69989-2.c | no |
| 22 | variable length arrays | variable length arrays are not supported yet | compile/20010202-1.c | no (in WP-1276's stated scope) |
| 18 | address of a compound literal (an lvalue in C) | cannot take the address of an rvalue of type '…' | compile/compound-literal-2.c | no (in WP-1276's stated scope) |
| 16 | GNU `a ?: b` with a logical condition (crash) | ICE: phi operand %N is defined | compile/pr111414-1.c | no |
| 16 | redefinition (tentative definitions, compatible redeclarations) | redefinition of '…' | compile/20000120-2.c | partial (4/16) |
| 16 | qualifier-dropping assignment (a C warning) | assigning to '…' from '…' discards qualifiers | compile/20010426-1.c | yes (16/16) |
| 14 | uninitialized `const` object | default initialization of an object of const type '…' | compile/20041211-1.c | yes (14/14) |
| 14 | pointer conversions in `return` (lvalue) | cannot initialize return object … with an lvalue | compile/pr93927-1.c | partial (10/14) |
| 12 | goto past an initialized declaration | cannot jump from this goto statement to its label | compile/irreducible-loop.c | no |
| 12 | pointer conversions in `return` (rvalue) | cannot initialize return object … with an rvalue | compile/pr111059-10.c | yes (12/12) |
| 10 | pointer conversions in array element init | cannot initialize an array element … with an lvalue | compile/931004-1.c | yes (10/10) |
| 10 | `_Complex` conversions | implicit conversion from '…' to '…' is not permitted in C++ | compile/complex-1.c | yes (10/10) |
| 10 | pointer conversions in member init | cannot initialize a member subobject … with an lvalue | execute/20000722-1.c | partial (4/10) |
| 10 | string literal that fills a char array exactly | initializer-string for char array is too long | execute/memchr-1.c | no |
| 8 | file-scope `int a[];` (tentative array) | definition of variable with array type needs an explicit size | compile/920624-1.c | no |
| 8 | incomplete type at a tentative definition | variable has incomplete type '…' | compile/930525-1.c | no |
| 8 | array initialized from a string or expression (GNU) | array initializer must be an initializer list | compile/930927-1.c | no |
| 8 | `void main` | 'main' must return 'int' | compile/BUG12.c | no |

Clusters clang-19 rejects as well are not ranked; they are mostly GNU
extensions clang does not implement. The largest are 56 nested function
definitions ("function definition is not allowed here") and 32
variable-length struct fields ("fields must have a constant size").

### Wave-2 input: what wp/1276 leaves (gnu23, clang-19 passes)

435 cases of 7572 remain at `dd6ba3cf7`, in ranked order. The falloff
miscompile is gone; the two cases left in the wrong class are the
`__builtin_constant_p` note (bcp-1.c and builtin-constant.c at -O2).

1. 18 crash: the `?:` ICE.
2. 74 pointer-to-narrow-integer casts (37 tests).
3. 54 C++ narrowing in C brace initializers and case labels (non-constant 26,
   constant 26, case labels 2; sqlite shell.c too).
4. 36 `void *` arithmetic.
5. 24 implicit function declarations and missing GNU builtins (12 tests:
   `__builtin_setjmp`/`longjmp`, `__builtin_exit`, `__builtin_mul_overflow_p`,
   `__builtin_shuffle`, implicit `memcmp` and `strlen`).
6. 22 VLAs and 20 compound-literal lvalues (both in WP-1276's stated scope,
   and not yet passing at `dd6ba3cf7`).
7. 12 each:
   - redefinition;
   - unknown type name (10 are GNU `__label__` local labels);
   - goto past an initializer;
   - comparison of distinct pointer types (`pr106537-1.c`, and the lua and
     zstd function pointer against `NULL` case);
   - exact-fit string initializers.
8. 8 or fewer each:
   - tentative arrays;
   - array init from a string;
   - `void main`;
   - flexible array member initialization;
   - `restrict` on a member function;
   - pointer and integer comparison;
   - `void` operands of `?:`;
   - `sizeof` of a function type;
   - types defined in a cast, a `sizeof` or a return type;
   - forward-referenced enums;
   - assignment to a compound literal;
   - qualified anonymous bit-fields.

Plus, from the OSS rows:
- `[restrict]` and `[static N]` in parameters (c-ares, all 89 TUs);
- `__builtin_neon_vqtbl2q_v` codegen (grpc).

### Wave-2 progress

- **WP-1285** (C expressions, wp/1285 at `e3089e246`; generated region
  below), cases where clang-19 passes, before (`dd6ba3cf7`) and after:
  - pointer cast to a narrower integer: 74 to 0;
  - `void *` and function-pointer arithmetic: 36 to 0;
  - distinct pointer types and function pointer against `NULL`: 12 to 0;
  - pointer against integer comparison, `void` operands of `?:`, `sizeof`
    of a function or `void`, forward-referenced enums: 8 or fewer each,
    to 0;
  - tags defined in a cast, `sizeof`, return or parameter type: to 0 for
    the construct; compile/pr46866.c (2 cases) now stops on
    `transparent_union` (WP-1288);
  - `restrict` on a member function: 0 left in the expression path. The 6
    cases (20040309-1, 980825-1, execute/930513-2) are a K&R declaration
    list whose `const` parses as a function cv-qualifier (WP-1286);
    pr34885 is `transparent_union` (WP-1288) and pr34334 a file-scope
    anonymous union (WP-1286);
  - indirection through `void *` (compile/20000701-1.c, 2 cases): fixed at
    `a005754a2`, after the generated run.
  - Overall at gnu23: kiln 6480 to 6628 passing of 7572 (6630 with
    20000701-1); "kiln fails, every oracle passes" 435 to 287. The crash
    and wrong cases where clang passes are the `?:` ICE (WP-1288) and the
    `__builtin_constant_p` note.
  - OSS, `c = "kiln"`, at `8f3d0871e`: lua v5.4.8 34/34 TUs and its tests pass (wp/1276:
    stopped on the function pointer against `NULL`); zstd 116/118 C TUs,
    1/3 tests, zstd v1.5.7 (the two TUs left are WP-1286 declaration cases).

### Xfail repros (`xfail(kiln): WP-1282 follow-up, …`, all pass under clang-19)

- `tests/c/expr/gnu-binary-conditional-logical.c`: the `?:` ICE (C).
- `tests/conformance/expr.cond/gnu-binary-conditional-logical-run.cpp`: the
  `?:` ICE (C++).
- `tests/c/decl/array-param-qualifiers.c`: `[restrict]` and `[static N]` in
  parameters.
- `tests/c/expr/fnptr-null-pointer-compare.c`: function pointer compared
  with `(void *)0`. Passes since WP-1285.
- `tests/c/expr/init-list-no-narrowing.c`: C++ narrowing in C brace
  initializers.
- `tests/c/expr/cast-pointer-to-narrow-integer.c`: pointer cast to a narrower
  integer. Passes since WP-1285.
- `tests/c/expr/void-pointer-arithmetic.c`: GNU `void *` arithmetic. Passes
  since WP-1285.
- `tests/c/stmt/goto-past-initializer.c`: goto past an initialized
  declaration.
- `tests/c/decl/char-array-exact-string-init.c`: string literal that fills
  the array exactly.
- `tests/c/stmt/switch-case-value-converted.c`: case constants are converted,
  not narrowed.

## Wave-2 re-measurements

- **WP-1287** (array parameters, VLAs, compound literals, plus C
  subscripts, enum compound assignment and `transparent_union`):
  gcc.c-torture (all four suites, -O0 and -O2, gnu23, `cargo xtask
  gcc-torture --oracles clang`) goes from 6480/7572 at `92fc1f18e` to
  6534/7572 at `800e8d032`, 54 new passes and no regressions;
  compile/struct-non-lval-1.c (both option sets) passes after the
  subscript fix. The VLA cluster ("variable length arrays are not
  supported yet") is 22/22, the compound-literal cluster ("cannot take
  the address of an rvalue", "expression is not assignable") 26/26.
  c-ares (`scripts/oss/oss-build.sh c_ares --compiler kiln`) configures
  (the probes no longer cascade) and 73 of 91 C TUs compile, up from
  2; the remaining 18 stop only on a function pointer compared with
  `NULL` (WP-1285). With wp/1285 merged on top, c-ares is green:
  configure, build, and 2/2 tests.

## Driver and harness follow-ups (not C semantics)

- Fixed here: `crates/kiln_driver/src/codegen_cmd.rs`, the link-only path.
  It forwards library and linker inputs, so link probes such as sqlite's
  `HAVE_LOG` no longer diverge. Unit tests are in `link_*`.
- Fixed here: `xtask/src/gcc_tests/torture.rs`, `--report` keys its region by
  `--std`, so the gnu23 and gnu17 runs sit side by side in this page
  (spec/harness.md).
- Fixed in WP-1288: `-rpath DIR` (sqlite configure), `-Wa,` and
  `-Xassembler` (zstd's `-Wa,--noexecstack`), and `__builtin_constant_p`
  through `llvm.is.constant` (bcp-1.c).
- Open:
  - CMake's ASM compiler identification is "unknown": it looks for
    "clang version" in `--version`, which kiln deliberately does not print
    (docs/COMPATIBILITY.md). kiln-cc is still found as the assembler, so
    no row depends on it today;
  - link errors print with a `clang++:` prefix ahead of kiln's own line;
  - `ld` cannot read the `-O2 -g` line table of some configure probes;
  - compile/limits-structnest.c times out;
  - compile/pr65680.c runs out of memory (see Crashes).

## Perf log

Small C TUs: kiln's wall time is 1.3x to 1.6x clang-19's (startup and pipeline
dominated), and its peak RSS is about 35% lower. On sqlite3.c, the one large
C TU, kiln (wp/1276) is 11% slower and uses 20% less memory. Every entry runs
in the row's build dir, with the flags exactly as recorded in that row's
`compile.jsonl` (`argv`, `wall_s` and `max_rss_kb`); `<cc>` is `kiln` or
`clang-19`.

- zlib v1.3.2, `inflate.c`, from `.oss/build/zlib/<c>`:
  - command: `<cc> -DHAVE_HIDDEN -DNO_FSEEKO -DZLIB_BUILD -DZLIB_DLL
    -D_LARGEFILE64_SOURCE=1 -I. -I../../src/zlib -O3 -DNDEBUG -fPIC -c
    ../../src/zlib/inflate.c`;
  - kiln 0.384 s and 71132 KB; clang-19 0.244 s and 109744 KB.
- zstd v1.5.7, `lib/compress/huf_compress.c`, from `.oss/build/zstd/<c>/lib`:
  - command: `<cc> -DXXH_NAMESPACE=ZSTD_ -DZSTD_LEGACY_SUPPORT=5
    -DZSTD_MULTITHREAD -Dlibzstd_shared_EXPORTS -I. -I<src>/build/cmake/lib
    -Wall -Wextra -Wundef -Wshadow -Wcast-align -Wcast-qual
    -Wstrict-prototypes -Qunused-arguments -O3 -DNDEBUG -fPIC
    -DZSTD_DISABLE_ASM -c <src>/lib/compress/huf_compress.c`;
  - clang also gets `-Wa,--noexecstack`;
  - kiln 0.443 s and 70188 KB; clang-19 0.339 s and 114868 KB.
- lua v5.4.8, `lmem.c`, from `.oss/build/lua/<c>/make`:
  - command: `<cc> -Wall -O2 -Wfatal-errors -Wextra -Wshadow -Wundef
    -Wwrite-strings -Wredundant-decls -Wdisabled-optimization
    -Wdouble-promotion -Wmissing-declarations -Wdeclaration-after-statement
    -Wmissing-prototypes -Wnested-externs -Wstrict-prototypes -Wc++-compat
    -Wold-style-definition -std=c99 -DLUA_USE_LINUX -fno-stack-protector
    -fno-common -c -o lmem.o lmem.c`;
  - kiln 0.081 s and 65148 KB; clang-19 0.051 s and 101496 KB.
- sqlite 3.54.1, `sqlite3.c` (wp/1276 build), from
  `.oss/build/sqlite/<c>/autotools`:
  - command: `<cc> -c <src>/sqlite3.c -o sqlite3.o -O2 -g -I. -fPIC
    -DSQLITE_ENABLE_CARRAY -DSQLITE_ENABLE_DBPAGE_VTAB
    -DSQLITE_ENABLE_DBSTAT_VTAB -DSQLITE_ENABLE_FTS4 -DSQLITE_ENABLE_FTS5
    -DSQLITE_ENABLE_GEOPOLY -DSQLITE_ENABLE_MATH_FUNCTIONS
    -DSQLITE_ENABLE_PERCENTILE -DSQLITE_ENABLE_PREUPDATE_HOOK
    -DSQLITE_ENABLE_RTREE -DSQLITE_ENABLE_SESSION -DSQLITE_HAVE_ZLIB=1
    -DSQLITE_THREADSAFE=1`;
  - kiln 31.8 s and 741152 KB; clang-19 28.6 s and 926572 KB.
- simdjson v4.2.4's `yyjson.c` (WP-1288, harness build of wp/1288), from
  a clang-19 configure of the row (`compile_commands.json`, replayed):
  - command: `<cc> -DSIMDJSON_AVX512_ALLOWED=1 -DSIMDJSON_UTF8VALIDATION=1
    -isystem <build>/_deps/yyjson-src/src -O3 -DNDEBUG -c
    <build>/_deps/yyjson-src/src/yyjson.c` (best of 3);
  - kiln 2.10 s and 92072 KB; clang-19 1.80 s and 140640 KB.

## gcc.c-torture at -std=gnu23 (generated)

Written by `cargo xtask gcc-torture --report docs/status/c-mode.md` on wp/1285 (`e3089e246`, main `92fc1f18e` plus WP-1285); kiln runs at the clang oracle's `-fclang-compat=19`. Rows before WP-1285 are in the tables above.

<!-- gcc-torture:begin (generated by cargo xtask gcc-torture) -->

GCC C torture suites (`gcc.c-torture`, `releases/gcc-16.1.0`), host target `aarch64-unknown-linux-gnu`.

- tests: GCC `6afcc4f6da931eb93f3ab001a0dd9650ea71d1ea`; kiln `e3089e246`
- command: `cargo xtask gcc-torture -j 4 --suite execute,ieee,builtins,compile --opt -O0,-O2 --profile harness`
- kiln: kiln version 1.0.1 (C++26 frontend, LLVM IR backend) (-fclang-compat=19)
- gcc: gcc (GCC) 16.1.0
- clang: Debian clang version 19.1.7 (3+b1)
- option sets: -O0, -O2; -std=gnu23 first
- limits: 2048 MiB address space per step (plus kiln's stack reservation); compile/link timeout 120 s, run timeout 10 s; -j 4
- 3813 tests, 7572 cases run, 27 skip rows; wall 1822 s

### Results by class (cases)

| class | kiln | gcc | clang |
|---|---:|---:|---:|
| pass | 6628 | 7572 | 6887 |
| compile-reject | 885 | 0 | 648 |
| crash | 21 | 0 | 0 |
| wrong | 36 | 0 | 37 |
| timeout | 2 | 0 | 0 |
| total | 7572 | 7572 | 7572 |

### Pass by suite and option set

| suite | opt | cases | kiln | gcc | clang |
|---|---|---:|---:|---:|---:|
| execute | -O0 | 1677 | 1491 | 1677 | 1540 |
| execute | -O2 | 1677 | 1479 | 1677 | 1529 |
| ieee | -O0 | 69 | 65 | 69 | 64 |
| ieee | -O2 | 69 | 66 | 69 | 65 |
| builtins | -O0 | 59 | 35 | 59 | 35 |
| builtins | -O2 | 59 | 30 | 59 | 28 |
| compile | -O0 | 1981 | 1730 | 1981 | 1813 |
| compile | -O2 | 1981 | 1732 | 1981 | 1813 |

### kiln against the oracles (cases)

- kiln fails, every oracle passes: 287
- kiln fails, gcc fails too: 0
- kiln passes, gcc fails: 0
- kiln fails, clang fails too: 657
- kiln passes, clang fails: 28
- kiln failures GCC's testsuite marks xfail: 0

### kiln failure clusters (71 clusters, top 60)

| cases | cluster | examples |
|---:|---|---|
| 200 | compile-reject: error: unknown type name '…' | compile/20000326-2.c -O0, compile/20000326-2.c -O2, compile/20000420-1.c -O0 |
| 198 | compile-reject: error: use of undeclared identifier '…' | compile/20000105-1.c -O0, compile/20000105-1.c -O2, compile/20000224-1.c -O0 |
| 64 | compile-reject: error: function definition is not allowed here | compile/20010226-1.c -O0, compile/20010226-1.c -O2, compile/20020309-1.c -O0 |
| 60 | compile-reject: error: a type specifier is required for all declarations | compile/20080704-1.c -O0, compile/20080704-1.c -O2, compile/20101216-1.c -O0 |
| 36 | compile-reject: error: fields must have a constant size: '…' extension will never be supported | compile/20010605-1.c -O0, compile/20010605-1.c -O2, compile/20020210-1.c -O0 |
| 34 | compile-reject: error: variable length arrays are not supported yet | compile/20010202-1.c -O0, compile/20010202-1.c -O2, compile/20030716-1.c -O0 |
| 33 | wrong: signal: N (SIGABRT) | execute/20010122-1.c -O2, execute/20031003-1.c -O0, execute/20230630-2.c -O0 |
| 30 | compile-reject: error: non-constant-expression cannot be narrowed from type '…' to '…' in initializer list [-Wc++N-narrowing] | compile/icfmatch.c -O0, compile/icfmatch.c -O2, compile/labels-3.c -O0 |
| 20 | compile-reject: error: cannot take the address of an rvalue of type '…' | compile/compound-literal-2.c -O0, compile/compound-literal-2.c -O2, compile/compound-literal-3.c -O0 |
| 18 | crash: internal compiler error: phi operand %N is defined | compile/pc44485.c -O0, compile/pc44485.c -O2, compile/pr111414-1.c -O0 |
| 14 | compile-reject: error: constant expression evaluates to N which cannot be narrowed to type '…' [-Wc++N-narrowing] | compile/pr69989-2.c -O0, compile/pr69989-2.c -O2, execute/20100416-1.c -O0 |
| 14 | compile-reject: error: indirect goto in function with no address-of-label expressions | compile/920502-1.c -O0, compile/920502-1.c -O2, compile/920826-1.c -O0 |
| 12 | compile-reject: error: cannot jump from this goto statement to its label | compile/irreducible-loop.c -O0, compile/irreducible-loop.c -O2, compile/pr69740-1.c -O0 |
| 12 | compile-reject: error: constant expression evaluates to -N which cannot be narrowed to type '…' [-Wc++N-narrowing] | compile/pr45059.c -O0, compile/pr45059.c -O2, compile/pr60502.c -O0 |
| 12 | compile-reject: error: initializer-string for char array is too long, array size is N but initializer has size N (including the null terminating character) | execute/20010924-1.c -O0, execute/20010924-1.c -O2, execute/memchr-1.c -O0 |
| 12 | compile-reject: error: redefinition of '…' | compile/20000120-2.c -O0, compile/20000120-2.c -O2, compile/20011119-1.c -O0 |
| 10 | compile-reject: error: definition of variable with array type needs an explicit size or an initializer | compile/920624-1.c -O0, compile/920624-1.c -O2, compile/921203-1.c -O0 |
| 10 | compile-reject: error: initialization of a flexible array member with elements is not supported yet | compile/pr33382.c -O0, compile/pr33382.c -O2, compile/pr36154.c -O0 |
| 8 | compile-reject: error: '…' must return '…' | compile/BUG12.c -O0, compile/BUG12.c -O2, compile/dic.c -O0 |
| 8 | compile-reject: error: array initializer must be an initializer list | compile/930927-1.c -O0, compile/930927-1.c -O2, compile/pr48517.c -O0 |
| 8 | compile-reject: error: variable has incomplete type '…' | compile/pr85401.c -O0, compile/pr85401.c -O2, compile/pr96998.c -O0 |
| 6 | compile-reject: error: cannot jump from this indirect goto statement to one of its possible targets | compile/pr17913.c -O0, compile/pr17913.c -O2, execute/20071210-1.c -O0 |
| 6 | compile-reject: error: non-member function cannot have '…' qualifier | compile/20040309-1.c -O0, compile/20040309-1.c -O2, compile/980825-1.c -O0 |
| 6 | compile-reject: error: non-void function '…' should return a value | compile/20020418-1.c -O0, compile/20020418-1.c -O2, compile/20020927-1.c -O0 |
| 6 | compile-reject: link: undefined reference to `… | execute/941202-1.c -O0, execute/941202-1.c -O2, execute/builtins/abs-2.c -O2 |
| 4 | compile-reject: error: anonymous bit-field cannot have qualifiers | compile/pr43191.c -O0, compile/pr43191.c -O2, compile/pr98199.c -O0 |
| 4 | compile-reject: error: call to undeclared function '…'; ISO CN and later do not support implicit function declarations [-Wimplicit-function-declaration] | compile/udivmod4.c -O0, compile/udivmod4.c -O2, execute/20010605-2.c -O0 |
| 4 | compile-reject: error: cannot take the address of an rvalue of type '…' (aka '…') | compile/920501-11.c -O0, compile/920501-11.c -O2, execute/20071029-1.c -O0 |
| 4 | compile-reject: error: expression is not assignable | compile/compound-literal-1.c -O0, compile/compound-literal-1.c -O2, compile/struct-non-lval-1.c -O0 |
| 4 | compile-reject: error: illegal storage class on function | compile/20011023-1.c -O0, compile/20011023-1.c -O2, compile/nested-3.c -O0 |
| 4 | compile-reject: error: second parameter of '…' (argument array) must be of type '…' | compile/call.c -O0, compile/call.c -O2, compile/pr22269.c -O0 |
| 4 | compile-reject: error: static declaration of '…' follows non-static declaration | compile/20021120-1.c -O0, compile/20021120-1.c -O2, compile/20021120-2.c -O0 |
| 4 | compile-reject: error: void function '…' should not return a value | compile/20180605-1.c -O0, compile/20180605-1.c -O2, compile/920817-1.c -O0 |
| 3 | wrong: exit N | execute/20031003-1.c -O2, execute/921202-1.c -O2, execute/pr79286.c -O2 |
| 2 | compile-reject: error: '…' attribute cannot be applied to types | execute/pr68532.c -O0, execute/pr68532.c -O2 |
| 2 | compile-reject: error: '…' attribute is not supported in C++ | compile/pr43288.c -O0, compile/pr43288.c -O2 |
| 2 | compile-reject: error: address of vector element requested | execute/pr108292.c -O0, execute/pr108292.c -O2 |
| 2 | compile-reject: error: anonymous unions at namespace or global scope must be declared '…' | compile/pr34334.c -O0, compile/pr34334.c -O2 |
| 2 | compile-reject: error: array is too large (N elements) | execute/991014-1.c -O0, execute/991014-1.c -O2 |
| 2 | compile-reject: error: cannot combine with previous '…' declaration specifier | compile/20010903-2.c -O0, compile/20010903-2.c -O2 |
| 2 | compile-reject: error: cannot decrement value of type '…' (vector of N '…' values) | execute/pr123753.c -O0, execute/pr123753.c -O2 |
| 2 | compile-reject: error: case value evaluates to N, which cannot be narrowed to type '…' [-Wc++N-narrowing] | compile/20000326-1.c -O0, compile/20000326-1.c -O2 |
| 2 | compile-reject: error: conflicting types for '…' | compile/pr46866.c -O0, compile/pr46866.c -O2 |
| 2 | compile-reject: error: constant expression evaluates to -N which cannot be narrowed to type '…' (aka '…') [-Wc++N-narrowing] | compile/pr44687.c -O0, compile/pr44687.c -O2 |
| 2 | compile-reject: error: constant expression evaluates to N which cannot be narrowed to type '…' (aka '…') [-Wc++N-narrowing] | execute/simd-4.c -O0, execute/simd-4.c -O2 |
| 2 | compile-reject: error: expected '…' | compile/20010124-1.c -O0, compile/20010124-1.c -O2 |
| 2 | compile-reject: error: expected '…' after top level declarator | compile/990203-1.c -O0, compile/990203-1.c -O2 |
| 2 | compile-reject: error: expected '…' for function-style cast or type construction | execute/builtin-types-compatible-p.c -O0, execute/builtin-types-compatible-p.c -O2 |
| 2 | compile-reject: error: expected unqualified-id | compile/20010714-1.c -O0, compile/20010714-1.c -O2 |
| 2 | compile-reject: error: first parameter of '…' (argument count) must be of type '…' | compile/v.c -O0, compile/v.c -O2 |
| 2 | compile-reject: error: incompatible function pointer types initializing '…' with an expression of type '…' [-Wincompatible-function-pointer-types] | compile/pr43635.c -O0, compile/pr43635.c -O2 |
| 2 | compile-reject: error: incompatible integer to pointer conversion assigning to '…' from '…' [-Wint-conversion] | compile/930529-1.c -O0, compile/930529-1.c -O2 |
| 2 | compile-reject: error: incompatible integer to pointer conversion passing '…' to parameter of type '…'; take the address with & [-Wint-conversion] | compile/cmpsi386.c -O0, compile/cmpsi386.c -O2 |
| 2 | compile-reject: error: incompatible pointer to integer conversion initializing '…' (aka '…') with an expression of type '…' (aka '…'); dereference with * [-Wint-conversion] | compile/920501-23.c -O0, compile/920501-23.c -O2 |
| 2 | compile-reject: error: incompatible pointer to integer conversion initializing '…' with an expression of type '…' [-Wint-conversion] | compile/pr96796.c -O0, compile/pr96796.c -O2 |
| 2 | compile-reject: error: incompatible pointer to integer conversion initializing '…' with an expression of type '…'; dereference with * [-Wint-conversion] | compile/920501-8.c -O0, compile/920501-8.c -O2 |
| 2 | compile-reject: error: incompatible pointer to integer conversion returning '…' from a function with result type '…' [-Wint-conversion] | compile/920701-1.c -O0, compile/920701-1.c -O2 |
| 2 | compile-reject: error: indirection not permitted on operand of type '…' | compile/20000701-1.c -O0, compile/20000701-1.c -O2 |
| 2 | compile-reject: error: initialization of flexible array member is not allowed | execute/pr28865.c -O0, execute/pr28865.c -O2 |
| 2 | compile-reject: error: initializer element is not a compile-time constant | compile/init-1.c -O0, compile/init-1.c -O2 |

### Skips (rows; `*` rows cover every option set)

| rows | reason |
|---:|---|
| 13 | dg-do target does not match |
| 6 | requires effective target run_expensive_tests |
| 1 | dg-require-dll |
| 1 | dg-require-profiling |
| 1 | dg-require-stack-check |
| 1 | dg-skip-if: target ! { i?86-*-* x86_64-*-* } |
| 1 | requires effective target dfp |
| 1 | requires effective target fgraphite |
| 1 | requires effective target fopenmp |
| 1 | requires effective target nonpic |

### Flags a compiler rejected (dropped from its cases)

- kiln: `--param ggc-min-expand=0`, `--param ggc-min-expand=1`, `--param ggc-min-heapsize=1024`, `--param=max-sched-extend-regions-iters=10`, `-fconserve-stack`, `-fdump-tree-optimized`, `-fdump-tree-ssa`, `-fexpensive-optimizations`, `-ffat-lto-objects`, `-ffloat-store`, `-fgimple`, `-fharden-compares`, `-findirect-inlining`, `-fipa-icf`, `-fipa-modref`, `-fira-algorithm=priority`, `-flive-range-shrinkage`, `-fmodulo-sched`, `-fmove-loop-invariants`, `-fno-code-hoisting`, `-fno-dce`, `-fno-early-inlining`, `-fno-gcse`, `-fno-guess-branch-probability`, `-fno-if-conversion`, `-fno-inline-functions`, `-fno-ipa-cp`, `-fno-ipa-ra`, `-fno-ira-share-spill-slots`, `-fno-ivopts`, `-fno-move-loop-invariants`, `-fno-printf-return-value`, `-fno-tracer`, `-fno-tree-bit-ccp`, `-fno-tree-ccp`, `-fno-tree-ch`, `-fno-tree-coalesce-vars`, `-fno-tree-copy-prop`, `-fno-tree-dce`, `-fno-tree-dominator-opts`, `-fno-tree-dse`, `-fno-tree-forwprop`, `-fno-tree-fre`, `-fno-tree-loop-distribute-patterns`, `-fno-tree-loop-im`, `-fno-tree-pre`, `-fno-tree-sra`, `-fno-tree-vrp`, `-fno-vect-cost-model`, `-fnon-call-exceptions`, `-foptimize-strlen`, `-fpack-struct`, `-fsave-optimization-record`, `-fsched2-use-superblocks`, `-fschedule-insns2`, `-fselective-scheduling2`, `-ftracer`, `-ftree-loop-distribution`, `-ftree-loop-vectorize`, `-ftree-parallelize-loops=2`, `-funroll-all-loops`, `-funroll-loops`, `-funswitch-loops`, `-std=c2y`
- clang: `-fconserve-stack`, `-fdump-tree-optimized`, `-fdump-tree-ssa`, `-fgimple`, `-fharden-compares`, `-findirect-inlining`, `-fipa-icf`, `-fipa-modref`, `-fira-algorithm=priority`, `-flive-range-shrinkage`, `-fmove-loop-invariants`, `-fno-code-hoisting`, `-fno-dce`, `-fno-early-inlining`, `-fno-guess-branch-probability`, `-fno-if-conversion`, `-fno-ipa-ra`, `-fno-ira-share-spill-slots`, `-fno-move-loop-invariants`, `-fno-printf-return-value`, `-fno-tree-bit-ccp`, `-fno-tree-ccp`, `-fno-tree-ch`, `-fno-tree-coalesce-vars`, `-fno-tree-copy-prop`, `-fno-tree-dominator-opts`, `-fno-tree-dse`, `-fno-tree-forwprop`, `-fno-tree-fre`, `-fno-tree-loop-distribute-patterns`, `-fno-tree-loop-im`, `-fno-tree-pre`, `-fno-tree-sra`, `-foptimize-strlen`, `-fsched2-use-superblocks`, `-fselective-scheduling2`, `-ftree-loop-distribution`, `-ftree-loop-vectorize`, `-ftree-parallelize-loops=2`

<!-- gcc-torture:end -->

## gcc.c-torture at -std=gnu17 (generated)

Written by `cargo xtask gcc-torture --std gnu17 --report docs/status/c-mode.md`.

<!-- gcc-torture:begin std=gnu17 (generated by cargo xtask gcc-torture) -->

GCC C torture suites (`gcc.c-torture`, `releases/gcc-16.1.0`), host target `aarch64-unknown-linux-gnu`.

- tests: GCC `6afcc4f6da931eb93f3ab001a0dd9650ea71d1ea`; kiln `120df355b`
- command: `cargo xtask gcc-torture -j 4 --suite execute,ieee,builtins,compile --opt -O0,-O2 --std gnu17 --profile harness`
- kiln: kiln version 1.0.1 (C++26 frontend, LLVM IR backend)
- gcc: gcc (GCC) 16.1.0
- clang: Debian clang version 19.1.7 (3+b1)
- option sets: -O0, -O2; -std=gnu17 first
- limits: 2048 MiB address space per step (plus kiln's stack reservation); compile/link timeout 120 s, run timeout 10 s; -j 4
- 3813 tests, 7572 cases run, 27 skip rows; wall 975 s

### Results by class (cases)

| class | kiln | gcc | clang |
|---|---:|---:|---:|
| pass | 5353 | 7570 | 7057 |
| compile-reject | 2127 | 2 | 478 |
| crash | 20 | 0 | 0 |
| wrong | 67 | 0 | 37 |
| timeout | 5 | 0 | 0 |
| total | 7572 | 7572 | 7572 |

### Pass by suite and option set

| suite | opt | cases | kiln | gcc | clang |
|---|---|---:|---:|---:|---:|
| execute | -O0 | 1677 | 1280 | 1676 | 1576 |
| execute | -O2 | 1677 | 1273 | 1676 | 1565 |
| ieee | -O0 | 69 | 59 | 69 | 68 |
| ieee | -O2 | 69 | 60 | 69 | 69 |
| builtins | -O0 | 59 | 21 | 59 | 35 |
| builtins | -O2 | 59 | 17 | 59 | 28 |
| compile | -O0 | 1981 | 1321 | 1981 | 1858 |
| compile | -O2 | 1981 | 1322 | 1981 | 1858 |

### kiln against the oracles (cases)

- kiln fails, every oracle passes: 1715
- kiln fails, gcc fails too: 2
- kiln passes, gcc fails: 0
- kiln fails, clang fails too: 504
- kiln passes, clang fails: 11
- kiln failures GCC's testsuite marks xfail: 0

### kiln failure clusters (105 clusters, top 60)

| cases | cluster | examples |
|---:|---|---|
| 704 | compile-reject: error: unknown type name '…' | compile/20000326-2.c -O0, compile/20000326-2.c -O2, compile/20000403-1.c -O0 |
| 240 | compile-reject: error: a type specifier is required for all declarations | compile/20040317-3.c -O0, compile/20040317-3.c -O2, compile/20080704-1.c -O0 |
| 180 | compile-reject: error: use of undeclared identifier '…' | compile/20000105-1.c -O0, compile/20000105-1.c -O2, compile/20000224-1.c -O0 |
| 118 | compile-reject: error: no matching function for call to '…' | compile/20030518-1.c -O0, compile/20030518-1.c -O2, compile/20040214-2.c -O0 |
| 82 | compile-reject: error: cannot initialize a variable of type '…' with an lvalue of type '…' | compile/20030904-1.c -O0, compile/20030904-1.c -O2, compile/20071118-1.c -O0 |
| 74 | compile-reject: error: ISO C++N does not allow '…' storage class specifier [-Wregister] | compile/20010112-1.c -O0, compile/20010112-1.c -O2, compile/20011114-4.c -O0 |
| 72 | compile-reject: error: incompatible pointer types assigning to '…' from '…' | compile/20070529-2.c -O0, compile/20070529-2.c -O2, compile/20070915-1.c -O0 |
| 56 | compile-reject: error: function definition is not allowed here | compile/20010226-1.c -O0, compile/20010226-1.c -O2, compile/20020309-1.c -O0 |
| 42 | compile-reject: error: cannot initialize a variable of type '…' with an rvalue of type '…' | compile/20040121-1.c -O0, compile/20040121-1.c -O2, compile/20080419-1.c -O0 |
| 34 | compile-reject: error: variable length arrays are not supported yet | compile/20010202-1.c -O0, compile/20010202-1.c -O2, compile/20030716-1.c -O0 |
| 34 | wrong: signal: N (SIGABRT) | execute/20031003-1.c -O0, execute/20060930-1.c -O2, execute/20230630-2.c -O0 |
| 32 | compile-reject: error: fields must have a constant size: '…' extension will never be supported | compile/20010605-1.c -O0, compile/20010605-1.c -O2, compile/20020210-1.c -O0 |
| 26 | compile-reject: error: cast from pointer to smaller type '…' loses information | compile/20000105-2.c -O0, compile/20000105-2.c -O2, compile/20020910-1.c -O0 |
| 26 | compile-reject: error: non-constant-expression cannot be narrowed from type '…' to '…' in initializer list [-Wc++N-narrowing] | compile/icfmatch.c -O0, compile/icfmatch.c -O2, compile/pr34856.c -O0 |
| 20 | wrong: signal: N (SIGTRAP) | execute/20000717-1.c -O0, execute/20000717-5.c -O0, execute/20010122-1.c -O0 |
| 18 | compile-reject: error: cannot initialize a variable of type '…' (aka '…') with an rvalue of type '…' | compile/pr106537-1.c -O0, compile/pr106537-1.c -O2, compile/pr106537-2.c -O0 |
| 16 | compile-reject: error: assigning to '…' from '…' discards qualifiers | compile/20010426-1.c -O0, compile/20010426-1.c -O2, compile/20120727-1.c -O0 |
| 16 | compile-reject: error: cannot take the address of an rvalue of type '…' | compile/compound-literal-2.c -O0, compile/compound-literal-2.c -O2, compile/compound-literal-3.c -O0 |
| 16 | compile-reject: error: redefinition of '…' | compile/20000120-2.c -O0, compile/20000120-2.c -O2, compile/20011119-1.c -O0 |
| 16 | crash: internal compiler error: phi operand %N is defined | compile/pr111414-1.c -O0, compile/pr111414-1.c -O2, compile/pr44119.c -O0 |
| 14 | compile-reject: error: arithmetic on a pointer to void | compile/20180309-1.c -O0, compile/20180309-1.c -O2, compile/20190827-1.c -O0 |
| 14 | compile-reject: error: arithmetic on pointers to void | compile/20021108-1.c -O0, compile/20021108-1.c -O2, compile/labels-1.c -O0 |
| 14 | compile-reject: error: default initialization of an object of const type '…' | compile/20041211-1.c -O0, compile/20041211-1.c -O2, compile/20121107-1.c -O0 |
| 12 | compile-reject: error: cannot initialize return object of type '…' with an lvalue of type '…' | compile/pr93927-1.c -O0, compile/pr93927-1.c -O2, compile/pr93927-2.c -O0 |
| 12 | compile-reject: error: cannot jump from this goto statement to its label | compile/irreducible-loop.c -O0, compile/irreducible-loop.c -O2, compile/pr69740-1.c -O0 |
| 12 | compile-reject: error: constant expression evaluates to N which cannot be narrowed to type '…' [-Wc++N-narrowing] | compile/pr69989-2.c -O0, compile/pr69989-2.c -O2, execute/20100416-1.c -O0 |
| 11 | wrong: exit N | execute/20000717-1.c -O2, execute/20000717-5.c -O2, execute/20020404-1.c -O2 |
| 10 | compile-reject: error: cannot initialize a member subobject of type '…' with an lvalue of type '…' | execute/20000722-1.c -O0, execute/20000722-1.c -O2, execute/20010123-1.c -O0 |
| 10 | compile-reject: error: cannot initialize an array element of type '…' with an lvalue of type '…' | compile/931004-1.c -O0, compile/931004-1.c -O2, compile/gen_tst.c -O0 |
| 10 | compile-reject: error: cannot initialize return object of type '…' with an rvalue of type '…' | compile/pr111059-10.c -O0, compile/pr111059-10.c -O2, compile/pr111059-12.c -O0 |
| 10 | compile-reject: error: constant expression evaluates to -N which cannot be narrowed to type '…' [-Wc++N-narrowing] | compile/pr45059.c -O0, compile/pr45059.c -O2, compile/pr60502.c -O0 |
| 10 | compile-reject: error: definition of variable with array type needs an explicit size or an initializer | compile/920624-1.c -O0, compile/920624-1.c -O2, compile/921203-1.c -O0 |
| 10 | compile-reject: error: implicit conversion from '…' to '…' is not permitted in C++ | compile/complex-1.c -O0, compile/complex-1.c -O2, compile/pr25513.c -O0 |
| 10 | compile-reject: error: incompatible pointer types assigning to '…' (aka '…') from '…' | execute/20031201-1.c -O0, execute/20031201-1.c -O2, execute/20071120-1.c -O0 |
| 10 | compile-reject: error: initialization of a flexible array member with elements is not supported yet | compile/pr33382.c -O0, compile/pr33382.c -O2, compile/pr36154.c -O0 |
| 10 | compile-reject: error: initializer-string for char array is too long, array size is N but initializer has size N (including the null terminating character) | execute/memchr-1.c -O0, execute/memchr-1.c -O2, execute/pr86714.c -O0 |
| 10 | compile-reject: error: variable has incomplete type '…' | compile/pr85401.c -O0, compile/pr85401.c -O2, compile/pr96998.c -O0 |
| 8 | compile-reject: error: '…' must return '…' | compile/BUG12.c -O0, compile/BUG12.c -O2, compile/dic.c -O0 |
| 8 | compile-reject: error: array initializer must be an initializer list | compile/930927-1.c -O0, compile/930927-1.c -O2, compile/pr48517.c -O0 |
| 8 | compile-reject: error: cannot initialize a variable of type '…' (aka '…') with an lvalue of type '…' | compile/pr32372.c -O0, compile/pr32372.c -O2, compile/pr42705.c -O0 |
| 8 | compile-reject: error: indirect goto in function with no address-of-label expressions | compile/920826-1.c -O0, compile/920826-1.c -O2, compile/pr30984.c -O0 |
| 6 | compile-reject: error: cannot combine with previous '…' declaration specifier | compile/20010903-2.c -O0, compile/20010903-2.c -O2, compile/20011023-1.c -O0 |
| 6 | compile-reject: error: cannot initialize an array element of type '…' with an rvalue of type '…' | execute/builtins/fprintf.c -O0, execute/builtins/fprintf.c -O2, execute/builtins/fputs.c -O0 |
| 6 | compile-reject: error: comparison between pointer and integer ('…' and '…') | compile/20070419-1.c -O0, compile/20070419-1.c -O2, compile/pr106030.c -O0 |
| 6 | compile-reject: error: left operand to ? is void, but right operand is of type '…' | compile/20000211-1.c -O0, compile/20000211-1.c -O2, compile/pr82337.c -O0 |
| 6 | compile-reject: error: member access into incomplete type '…' | execute/pr22141-1.c -O0, execute/pr22141-1.c -O2, execute/pr22141-2.c -O0 |
| 4 | compile-reject: error: '…' cannot be defined in a type specifier | compile/20011217-2.c -O0, compile/20011217-2.c -O2, compile/920928-4.c -O0 |
| 4 | compile-reject: error: '…' cannot be defined in the result type of a function | compile/900116-1.c -O0, compile/900116-1.c -O2, compile/pr113616.c -O0 |
| 4 | compile-reject: error: ISO C++ forbids forward references to '…' types | compile/990928-1.c -O0, compile/990928-1.c -O2, execute/930408-1.c -O0 |
| 4 | compile-reject: error: anonymous bit-field cannot have qualifiers | compile/pr43191.c -O0, compile/pr43191.c -O2, compile/pr98199.c -O0 |
| 4 | compile-reject: error: expected '…' | compile/20010124-1.c -O0, compile/20010124-1.c -O2, compile/ifreg.c -O0 |
| 4 | compile-reject: error: expected '…' after top level declarator | compile/990203-1.c -O0, compile/990203-1.c -O2, compile/ext.c -O0 |
| 4 | compile-reject: error: expected unqualified-id | compile/20010714-1.c -O0, compile/20010714-1.c -O2, compile/layout.c -O0 |
| 4 | compile-reject: error: expression is not assignable | compile/compound-literal-1.c -O0, compile/compound-literal-1.c -O2, compile/struct-non-lval-1.c -O0 |
| 4 | compile-reject: error: invalid application of '…' to a function type | compile/20011106-1.c -O0, compile/20011106-1.c -O2, execute/align-3.c -O0 |
| 4 | compile-reject: error: invalid application of '…' to an incomplete type '…' | execute/20000223-1.c -O0, execute/20000223-1.c -O2, execute/20001203-2.c -O0 |
| 4 | compile-reject: error: no viable overloaded '…' | compile/pr101437.c -O0, compile/pr101437.c -O2, execute/20010518-2.c -O0 |
| 4 | compile-reject: error: non-void function '…' should return a value | compile/20020418-1.c -O0, compile/20020418-1.c -O2, compile/20020927-1.c -O0 |
| 4 | compile-reject: error: static declaration of '…' follows non-static declaration | compile/20021120-1.c -O0, compile/20021120-1.c -O2, compile/20021120-2.c -O0 |
| 4 | compile-reject: link: undefined reference to `… | execute/builtins/abs-2.c -O2, execute/builtins/uabs-2.c -O2, execute/ieee/fp-cmp-7.c -O0 |

### Skips (rows; `*` rows cover every option set)

| rows | reason |
|---:|---|
| 13 | dg-do target does not match |
| 6 | requires effective target run_expensive_tests |
| 1 | dg-require-dll |
| 1 | dg-require-profiling |
| 1 | dg-require-stack-check |
| 1 | dg-skip-if: target ! { i?86-*-* x86_64-*-* } |
| 1 | requires effective target dfp |
| 1 | requires effective target fgraphite |
| 1 | requires effective target fopenmp |
| 1 | requires effective target nonpic |

### Flags a compiler rejected (dropped from its cases)

- kiln: `--param ggc-min-expand=0`, `--param ggc-min-expand=1`, `--param ggc-min-heapsize=1024`, `--param=max-sched-extend-regions-iters=10`, `-fconserve-stack`, `-fdump-tree-optimized`, `-fdump-tree-ssa`, `-fexpensive-optimizations`, `-ffat-lto-objects`, `-ffloat-store`, `-fgimple`, `-fgnu89-inline`, `-fharden-compares`, `-findirect-inlining`, `-fipa-icf`, `-fipa-modref`, `-fira-algorithm=priority`, `-flive-range-shrinkage`, `-fmodulo-sched`, `-fmove-loop-invariants`, `-fno-code-hoisting`, `-fno-dce`, `-fno-early-inlining`, `-fno-gcse`, `-fno-guess-branch-probability`, `-fno-if-conversion`, `-fno-inline-functions`, `-fno-ipa-cp`, `-fno-ipa-ra`, `-fno-ira-share-spill-slots`, `-fno-ivopts`, `-fno-move-loop-invariants`, `-fno-printf-return-value`, `-fno-tracer`, `-fno-tree-bit-ccp`, `-fno-tree-ccp`, `-fno-tree-ch`, `-fno-tree-coalesce-vars`, `-fno-tree-copy-prop`, `-fno-tree-dce`, `-fno-tree-dominator-opts`, `-fno-tree-dse`, `-fno-tree-forwprop`, `-fno-tree-fre`, `-fno-tree-loop-distribute-patterns`, `-fno-tree-loop-im`, `-fno-tree-pre`, `-fno-tree-sra`, `-fno-tree-vrp`, `-fno-vect-cost-model`, `-fnon-call-exceptions`, `-foptimize-strlen`, `-fpack-struct`, `-fsave-optimization-record`, `-fsched2-use-superblocks`, `-fschedule-insns2`, `-fselective-scheduling2`, `-ftracer`, `-ftree-loop-distribution`, `-ftree-loop-vectorize`, `-ftree-parallelize-loops=2`, `-funroll-all-loops`, `-funroll-loops`, `-funswitch-loops`, `-std=c2y`
- clang: `-fconserve-stack`, `-fdump-tree-optimized`, `-fdump-tree-ssa`, `-fgimple`, `-fharden-compares`, `-findirect-inlining`, `-fipa-icf`, `-fipa-modref`, `-fira-algorithm=priority`, `-flive-range-shrinkage`, `-fmove-loop-invariants`, `-fno-code-hoisting`, `-fno-dce`, `-fno-early-inlining`, `-fno-guess-branch-probability`, `-fno-if-conversion`, `-fno-ipa-ra`, `-fno-ira-share-spill-slots`, `-fno-move-loop-invariants`, `-fno-printf-return-value`, `-fno-tree-bit-ccp`, `-fno-tree-ccp`, `-fno-tree-ch`, `-fno-tree-coalesce-vars`, `-fno-tree-copy-prop`, `-fno-tree-dominator-opts`, `-fno-tree-dse`, `-fno-tree-forwprop`, `-fno-tree-fre`, `-fno-tree-loop-distribute-patterns`, `-fno-tree-loop-im`, `-fno-tree-pre`, `-fno-tree-sra`, `-foptimize-strlen`, `-fsched2-use-superblocks`, `-fselective-scheduling2`, `-ftree-loop-distribution`, `-ftree-loop-vectorize`, `-ftree-parallelize-loops=2`

<!-- gcc-torture:end std=gnu17 -->
