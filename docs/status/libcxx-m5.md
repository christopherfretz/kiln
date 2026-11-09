# libc++ 22 full suite — M5 failure clusters (WP-446)

ROADMAP M5: the whole libc++ 22.1.8 lit suite (`llvmorg-22.1.8`, `std/` +
`libcxx/`) with kiln as `%{cxx}`, `-std=c++26 -fclang-compat=22`; the exit
is 100% pass or every remaining test triaged as UNSUPPORTED/XFAIL with a
written reason that is not "kiln bug". The run row is in
[libcxx.md](libcxx.md) "M5: libc++ 22 full suite"; this page is its
cluster table. Diagnostic wording (class b) does not gate M5 (operator,
2026-11-03): it is triaged here and kept visible as a count.

Classes: **(a)** kiln correctness (wrong answer, rejects-valid,
accepts-invalid, crash, codegen); **(b)** a `.verify` diagnostic-wording or
missing-warning gap; **(c)** harness/driver gap; **(d)** environment.

## Summary

| | total | pass | fail | xfail | unsupported (driver) | timeout | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| first run, base `c77a092a` (WP-418 harness)¹ | 9247 | 8562 | 195 | 34 | 455 (31) | 1 | 97.4% |
| after WP-446 fixes (`ced0e9ad`, main `b4e8dfb3` merged; the 196 re-run) | 9247 | 8708 | 48 | 36 | 455 (31) | 0 | 99.0% |
| after WP-461 (`db976360`; the 91 re-run²) | 9247 | 8716 | 9 | 64 | 457 (31) | 1 | **99.2%** |

¹ The run started on `c77a092a`; the report stamps `c2e88d0a`, the head
when it finished. WP-446 fixes were built into the same harness target
during the 69 min run, so late tests may have used a newer kiln++.

² The 48 FAILs left by WP-446 plus the driver-unsupported, clang-defect
XFAIL and environment tests near them (91 paths, -j 3, 157 s); the other
9156 keep their WP-446 outcome. 28 of the 64 XFAILs are the `M5_TRIAGED`
ledger below; 2 of the 457 UNSUPPORTED are the new environment exclusions.

Pass rate = pass / (total − unsupported). `std/`: 97.9% → 99.4% (12 fail
left) → 3 fail + 1 timeout; `libcxx/`: 89.3% → 92.7% (36 fail left) → 6
fail. The second row re-runs only the 196 first-run FAIL/TIMEOUT tests and
the third only its 91 (the full suite was not rerun); the quick CI corpus
and the M3 gate guard the rest.

Left after WP-461: 9 FAIL, all class (a), and 1 TIMEOUT. Class (b) is
0 FAIL: 28 tests are XFAIL through the `M5_TRIAGED` ledger (see "Triaged
(b)" below), 5 now pass. Classes (c) and (d) are empty: both (c) tests
pass, and both (d) tests are UNSUPPORTED with a reason. The TIMEOUT
(`ifstream.members/offset_range.pass.cpp`) is a long test (240 s run
timeout) that timed out under shared-host load in this re-run; it passed
in the WP-446 re-run and passes at 240 s under kiln and clang 22. It is
in the long-test class, not a cluster.

WP-460, which landed after this run, fixes 8 of the 9 (trivial_abi ×6,
enable_if, std.compat; see "Fixed in WP-460"), leaving `make_from_tuple`,
which already passed by then (see "Fixed in WP-491").

## Open clusters

Reproducers are `xfail(kiln)` corpus tests (clang 22 the oracle) unless
the cell says otherwise.

| class | count | proposed WP | libc++ construct / first error | reproducer | tests |
|---|---:|---|---|---|---|
| (a) | 6 | `[[clang::trivial_abi]]`: attribute, then register passing/return of a non-trivial class (attr + codegen) | `unique_ptr`/`shared_ptr`/`weak_ptr` under `_LIBCPP_ABI_ENABLE_*_TRIVIAL_ABI`; kiln ignores the attribute ("unknown attribute 'trivial_abi' ignored") and returns through the caller's slot; run asserts | `class.temporary/trivial-abi-run.cpp` | `libcxx/memory/trivial_abi/*` (shared_ptr_arg, unique_ptr_arg, unique_ptr_array, unique_ptr_destruction_order, unique_ptr_ret, weak_ptr_ret) |
| (a) | 1 | clang `enable_if` attribute (attr + overload viability, "candidate disabled: <msg>") | `_CmpUnspecifiedParam`'s literal-0 constructor (`<__compare/ordering.h>`); kiln ignores the attribute and accepts `strong_ordering < 1` | `dcl.attr/enable-if-attr.cpp` | `cmp.categories.pre/reject-other-than-literal-zero.verify` |
| (a) | 1 | Modules: an imported unit built in-process keeps its GMF declarations apart from the importer's GMF (spec/modules.md §6) | `std.compat.cppm` over `std.cppm`, both GMFs including `<cstddef>`: "missing '#include <__cstddef/byte.h>'; 'byte' must be declared before it is used" at `__type_traits/is_trivially_lexicographically_comparable.h:50` | `module.global.frag/gmf-dedup-import-gmf.cpp` (+ `Inputs/gdx-*.cppm`) | `std/modules/std.compat.pass` |

The `__builtin_elementwise_add_sat`/`_sub_sat` cluster (3) passes after
main `db976360`.

## Triaged (b): `M5_TRIAGED` (WP-461)

Every test here passes under clang 22 with the same verify command; kiln
accepts the code correctly and only misses or words a diagnostic. The
ledger (`M5_TRIAGED`, `xtask/src/libcxx/run.rs`, libc++ 22 only) makes
each one `XFAIL` with the reason `m5: <reason>` (spec/libcxx.md "Outcomes" step 7);
an XPASS is the cue to drop its row.

| cause | count | WP | tests |
|---|---:|---|---|
| `-Wnonnull` "null passed to a callee that requires a non-null argument" | 5 | WP-433 | `nonnull.verify` in specialized.algorithms, mem.res, print.fun, basic.string, string.view |
| `-Wthread-safety` | 2 | none yet | `thread_safety.verify` in thread.shared_mutex, thread.shared_timed_mutex |
| "'…' is deprecated" on `rel_ops` and the C++17 adaptor typedefs | 2 | none yet | `rel_ops.depr_in_cxx20`, `typedefs.depr_in_cxx17` |
| `alloc_align` "requested alignment is not a power of 2" | 1 | none yet | `allocate_vocabulary.attributes.verify` |
| wording: the constraint note prints the substituted expression | 1 | none yet | `libcxx/algorithms/cpp17_iterator_concepts.verify` |
| wording: `std::hash<std::monostate>` printed qualified | 1 | WP-432 | `variant.hash/hash.depr.verify` |

## Regressions fixed in WP-980

Tests that passed in the WP-461 run and failed on main (found by the
WP-961 sweep). Each was bisected to the change that exposed it
(spec/landed/abi/WP-980.md).

| cluster | count | regressor | first error | fix | test |
|---|---:|---|---|---|---|
| chrono | 22 (std/time) | 9cab5cf2e (WP-818, PR #383) | "cannot yet mangle expression type MaterializeTemporaryExpr" in `operator<<`'s requires-clause | the mangler skips temporaries, as clang does (also WP-933) | `tests/mangle/materialized_temporary.cpp` |
| mdspan | 5 | c8ce199c7 (WP-802, PR #363) | "constant evaluation of CXXFoldExpr" (rank-0 `layout_right`/`layout_stride` `operator()`) | the pack-length sweep stops at a `FunctionParmPackExpr` | `temp.variadic/p7-empty-function-parameter-pack-in-generic-lambda-fold.cpp` |
| simd | 1 | 87f7d8f0f (WP-445, PR #189) | `simd_unary`: `!v` on an `unsigned char` vector rejected under `-flax-vector-conversions=none` | the runner applies `ADDITIONAL_COMPILE_FLAGS(clang)` to kiln | `ext.clang.vector_type/lax-*-logical-not-char-vector.cpp`, `directives.rs` `clang_dialect_flags` |
| stable_sort | 1 | none (signed plain `char` only) | `++` on `signed char` 127 faulted in the Oven | narrow signed `++`/`--` promote to `int` and wrap | `expr.pre.incr/narrow-signed-wrap-*.cpp` |

Subset counts with libc++ 22.1.8 on aarch64, before (main at
122d1442c, the branch base) and after:

| subset | before | after |
|---|---|---|
| std/time + std/containers/views/mdspan + std/experimental/simd (554) | 523 pass, 28 fail, 3 unsupported | 551 pass, 0 fail, 3 unsupported |
| libcxx/time + alg.sort/stable.sort (32) | not measured | 21 pass, 0 fail, 11 unsupported |

stable_sort's `test<char>()` passes on aarch64 either way because plain
`char` is unsigned there; it failed only on x86-64.

## Fixed in WP-674

The `M5_TRIAGED` rows this fixes are gone from the ledger (each test was
XPASS under kiln with libc++ 22.1.8, and passes with libc++ 19.1.7).

| class | count | construct / first error | fix | test |
|---|---:|---|---|---|
| (b) | 4 | `[[clang::lifetimebound]]` ignored: "temporary bound to local reference … will be destroyed at the end of the full-expression" never warned (`lifetimebound.verify` in algorithms, as-lvalue, function.objects, utility/forward) | clang 19's lifetime analysis with `lifetimebound` and gsl Owner/Pointer (spec/landed/sema/WP-674.md) | `class.temporary/lifetimebound.cpp`, `dangling-gsl-libcxx.cpp` |

## Fixed in WP-491

The `M5_TRIAGED` rows these fix are gone from the ledger (each test was
XPASS under kiln with libc++ 22.1.8).

| class | count | construct / first error | fix | test |
|---|---:|---|---|---|
| (b) | 7 | `[[nodiscard]]` on a later redeclaration of a function template not merged (libc++ forward-declares `get`, `any_cast`, `begin`/`end` without it): `nodiscard.verify` in array, complex.number, numarray, tuple, pairs, variant, any | a function template specialization takes the unused-result attributes of every declaration of its template seen when it is formed (`function_spec_attrs`; spec/landed/sema/WP-491.md) | `dcl.attr.nodiscard/redeclared-function-template.cpp` |
| (b) | 5 | a `[[nodiscard]]` hidden friend of a class template never warned: `nodiscard.verify` in range.filter, range.iota.view, range.repeat.view; join.with iterator `eq`, `iter_move` | an instantiated friend function gets the pattern's attributes (`friend_definition`/`function_decl`; spec/landed/sema/WP-491.md) | `dcl.attr.nodiscard/hidden-friend-class-template.cpp`, `hidden-friend-operator-clang{22,19}.cpp` |
| (a) | 1 | P2748R5 in `<tuple>`: `make_from_tuple<const int&>(tuple<char>{})` returns a reference bound to a temporary (`make_from_tuple.verify`) | already passing at this row's re-check (the C++26 error, WP-445 era); WP-491 adds the pre-C++26 `-Wreturn-stack-address` warning level and the in-tree reproduction | `stmt.return/returned-temporary-make-from-tuple{,-cxx23}.cpp` |

## Fixed in WP-461

| class | count | construct / first error | fix | test |
|---|---:|---|---|---|
| (b) | 2 | `[[nodiscard]]` through `static_cast` to a nodiscard conversion function (`<chrono>`), and a parenthesized nodiscard `operator==` (join.with sentinel) | sema `emit_unused` (clang `DiagnoseUnusedExprResult`) | `dcl.attr.nodiscard/through-cast.cpp` |
| (b) | 2 | dependent member type "no type named …" (`unexpected_disabled_cpp17`, `futures.task.members/type`) | WP-445, already on main | — |
| (c) | 1 | `-fexperimental-library` rejected | driver flag + `__has_feature(experimental_library)`, link `-lc++experimental` | `args.rs` `experimental_library`, `codegen_cmd.rs` `link_experimental_library` |
| (c) | 1 | `copy_file.pass` under a 110+ character scratch `TMPDIR` | harness scratch `t<index>` under a short temp root | xtask `short_scratch_paths` |
| (d) | 1 | `thread_create_failure`: `RLIMIT_NPROC` does not limit root | UNSUPPORTED when run as root (`ROOT_NPROC_UNSUPPORTED`) | xtask `env_exclusions` |
| (d) | 1 | `gcount`: two `INT_MAX` strings need more than the 3072 MiB cap | UNSUPPORTED below 7168 MiB (`MEM_UNSUPPORTED`, measured 6660 MiB; WP-491) | xtask `env_exclusions` |

## Fixed in WP-446

| class | count | construct / first error | fix (commit) | corpus test |
|---|---:|---|---|---|
| (a) | 115 | `forward_list` / `list` constexpr: `__delete_node` destroys a value that lives in a union member whose lifetime alone ended ("static assertion expression is not an integral constant expression") | Oven (`786c28b6`) | `class.union.anon/constexpr-destroy-member-then-object.cpp` |
| (a) | 4 | `stack` / `queue` `operator<`, `<=>`: a constrained friend template redeclaring a namespace template ("ambiguous", "static assertion failed") | sema friend constraints (`6f1df97a`) | `temp.friend/constrained-friend-template-*.cpp` |
| (a) | 5 | `map`/`set`/`unordered_map`/`unordered_set`/`deque` `abi.compile`: a specialization of a `final` class template is not final | templ (`50a27cae`) | `temp.inst/final-class-template.cpp` |
| (a) | 1 | `vector` `abi.compile`: `_LIBCPP_COMPRESSED_PAIR` puts an NSDMI in an anonymous member of a class template | deferred anonymous-member NSDMIs (`54e5a08d`) | `temp.inst/nsdmi-anonymous-member-deferred.cpp` |
| (a) | 5 | `std::function` (`F`, `F_assign`, `F_nullptr`, `invoke`, `noncopyable_return_type`): a pseudo-destructor on a pointer to member ("object expression of non-scalar type") | sema `is_scalar` (`d6162682`) | `expr.prim.id.dtor/pseudo-destructor-member-pointer-run.cpp` |
| (a) | 4 | `atomic_fetch_add`/`_sub`(`_explicit`) `.verify`: a function pointee reported as incomplete | sema C11 atomics (`20597114`) | `ext.c11.atomic/fetch-arith-function-pointer.cpp` |
| (a) | 1 | `bit-int.verify`: `_Atomic(_BitInt(N))` accepted | sema `act_on_atomic_type` (`d6162682`) | `ext.c11.atomic/atomic-specifier-bitint.cpp` |
| (a) | 2 | `expected` `transform_error.mandates.verify`: an NTTP naming an invalid variable, cascades from a failed specialization | sema (`c2e88d0a`) | `temp.arg.nontype/invalid-variable-specialization-argument.cpp`, `temp.inst/failed-static-assert-functional-cast.cpp` |
| (a) | 1 | `simd` `memory_alignment.verify`: a default argument substituted into a name that cannot match; failed construction quieted only inside an instantiation | sema (`1277d48f`) | `temp.inst/default-arg-elision-no-instantiation.cpp` |
| (a) | 2 | `segmented_iterator.compile`, `func.require/bullet_7`: [temp.deduct.type]/10 forwarding-reference adjustment when deducing from a function address | sema (`02c14595`) | `temp.deduct.funcaddr/forwarding-reference-*.cpp` |
| (a) | 1 | `product_iterator`: partial-specialization constraints check access from the matched specialization | sema (`683dd2e2`) | `temp.spec.partial.match/constraint-access-befriended-specialization.cpp` |
| (a) | 1 | `equality_comparable.compile`: C++26 array-comparison error during constraint satisfaction | sema (`12ca1758`) | `expr.eq/p2865-array-comparison-constraint.cpp` |
| (a) | 1 | `deque/asan_turning_off`: `delete[] (char*)p` parsed as a lambda | parse (`3e92879b`) | `expr.delete/array-delete-cast-operand.cpp` |
| (a) | 1 | `copy_move_trivial` (segfault): a file-scope function template named like a builtin hid the builtin | sema (`dc3b8556`) | `over.match.best/builtin-name-template.cpp` |
| (c) | 1 | `std/modules/std.pass`: kiln found the system LLVM 19 `std.cppm`, whose `headers_not_available` lists `<flat_map>` | harness `-fprebuilt-module-path` to the package tree (`Config::module_source_flags`, `d6162682`) | xtask unit test `clang_mode_flags` |
| (c) | 2 | `equality_comparable_with`, `three_way_comparable_with`: the `TEST_COMPILER_CLANG` branch pins llvm.org/PR171438; kiln follows the standard | now XFAIL with that reason (`CLANG_DEFECT_XFAIL`, `d4e0cff4`; spec/libcxx.md "Outcomes") | xtask unit test `clang_defect_xfail_list` |

## Fixed in WP-460

| class | count | construct / first error | fix (commit) | corpus test |
|---|---:|---|---|---|
| (a) | 6 | `[[clang::trivial_abi]]` `unique_ptr`/`shared_ptr`/`weak_ptr`: the attribute was ignored, so the class went through memory and the caller destroyed it | sema trivial-for-call bits, `kiln_abi` register passing, callee-destroyed parameters in codegen (`7b2cfd63`, `9c5f657f`; spec/sema.md "The `trivial_abi` attribute", spec/abi.md §8.26, spec/codegen.md §7ac) | `class.temporary/trivial-abi-run.cpp`, `class.temporary/run-trivial-abi-callee-destroys.cpp`, `dcl.attr/trivial-abi-attribute.cpp`, `tests/codegen/trivial_abi_*.cpp` |
| (a) | 1 | `cmp.categories.pre/reject-other-than-literal-zero.verify`: clang `enable_if` ignored, `strong_ordering < 1` accepted | sema `enable_if` viability and ranking, Oven `evaluate_with_params` (`daf9b450`, `f5c2d953`; spec/sema.md §4.5) | `dcl.attr/enable-if-attr.cpp`, `dcl.attr/enable-if-{viability,ranking,form}.cpp` |
| (a) | 1 | `std/modules/std.compat.pass`: an in-process imported unit's GMF declarations stayed apart from the importer's ("'byte' must be declared before it is used") | GMF entities merge across imports (`53258ba6`, `6d8d209c`; spec/modules.md §6) | `module.global.frag/gmf-dedup-import-gmf.cpp`, `module.global.frag/gmf-merge-importer.cpp` |

## Affected directories after the fixes

The directories the fixes touch, re-run whole on `ced0e9ad` (-j 3,
harness kiln, 20 s run timeout, 3072 MiB): 1136 tests, 955 pass, 11 fail,
2 xfail, 168 unsupported (98.7%; 666 s). The 11 FAILs are the open
clusters above (trivial_abi ×6, saturation ×2, enable_if, std.compat,
array `nodiscard.verify`).

| directory | tests | pass | fail | xfail | unsupported | pass rate |
|---|---:|---:|---:|---:|---:|---:|
| `std/containers/sequences/forwardlist` | 87 | 87 | 0 | 0 | 0 | 100% |
| `std/containers/sequences/list` | 89 | 89 | 0 | 0 | 0 | 100% |
| `std/containers/container.adaptors` | 445 | 445 | 0 | 0 | 0 | 100% |
| `libcxx/containers` | 201 | 64 | 1 | 0 | 136 | 98.5% |
| `std/utilities/expected` | 87 | 87 | 0 | 0 | 0 | 100% |
| `libcxx/utilities/expected` | 24 | 19 | 0 | 0 | 5 | 100% |
| `std/experimental/simd` | 33 | 33 | 0 | 0 | 0 | 100% |
| `libcxx/iterators` | 30 | 24 | 0 | 0 | 6 | 100% |
| `libcxx/ranges/range.adaptors/range.join` | 3 | 3 | 0 | 0 | 0 | 100% |
| `std/concepts` | 36 | 35 | 0 | 1 | 0 | 97.2% |
| `std/language.support/cmp` | 16 | 14 | 1 | 1 | 0 | 87.5% |
| `libcxx/memory/trivial_abi` | 6 | 0 | 6 | 0 | 0 | 0% |
| `std/numerics/numeric.ops/numeric.ops.sat` | 11 | 8 | 2 | 0 | 1 | 80.0% |
| `std/utilities/function.objects/func.wrap` | 39 | 34 | 0 | 0 | 5 | 100% |
| `libcxx/atomics` | 27 | 12 | 0 | 0 | 15 | 100% |
| `std/modules` | 2 | 1 | 1 | 0 | 0 | 50.0% |

## Rerunning

```
cd kiln && source scripts/env.sh
cargo xtask libcxx std libcxx --tag llvmorg-22.1.8 --no-fail -j 3 --timeout 20 --mem-limit 3072 --report m5.md
```

The full run takes about 70 min at `-j 3` on the shared host. Re-run a
cluster by naming its test paths instead of `std libcxx`.
