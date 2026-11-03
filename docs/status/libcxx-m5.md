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
| after WP-446 fixes (`ced0e9ad`, main `b4e8dfb3` merged; the 196 re-run) | 9247 | 8708 | 48 | 36 | 455 (31) | 0 | **99.0%** |

¹ The run started on `c77a092a`; the report stamps `c2e88d0a`, the head
when it finished. WP-446 fixes were built into the same harness target
during the 69 min run, so late tests may have used a newer kiln++.

Pass rate = pass / (total − unsupported). `std/`: 97.9% → 99.4% (12 fail
left); `libcxx/`: 89.3% → 92.7% (36 fail left). The second row re-runs
only the 196 first-run FAIL/TIMEOUT tests (the full suite was not rerun);
the quick CI corpus and the M3 gate guard the rest.

Left, by class: (a) 11, (b) 33, (c) 2, (d) 2. The TIMEOUT
(`ifstream.members/offset_range.pass.cpp`, 20 s) passes at 240 s under
kiln and clang 22: it is in the long-test class, not a cluster.

## Open clusters

Reproducers are `xfail(kiln)` corpus tests (clang 22 the oracle) unless
the cell says otherwise.

| class | count | proposed WP | libc++ construct / first error | reproducer | tests |
|---|---:|---|---|---|---|
| (a) | 6 | `[[clang::trivial_abi]]`: attribute, then register passing/return of a non-trivial class (attr + codegen) | `unique_ptr`/`shared_ptr`/`weak_ptr` under `_LIBCPP_ABI_ENABLE_*_TRIVIAL_ABI`; kiln ignores the attribute ("unknown attribute 'trivial_abi' ignored") and returns through the caller's slot; run asserts | `class.temporary/trivial-abi-run.cpp` | `libcxx/memory/trivial_abi/*` (shared_ptr_arg, unique_ptr_arg, unique_ptr_array, unique_ptr_destruction_order, unique_ptr_ret, weak_ptr_ret) |
| (a) | 3 | `__builtin_elementwise_add_sat` / `_sub_sat` (sema builtin table, Oven, codegen `llvm.[su]{add,sub}.sat`) | `std::add_sat` / `std::sub_sat` ([numeric.sat.func]): "use of undeclared identifier '__builtin_elementwise_add_sat'" | `ext.gnu.builtins/elementwise-sat-run-clang22.cpp` | `numeric.ops.sat/add_sat.pass`, `sub_sat.pass`, `libcxx/numerics/nodiscard.verify` |
| (a) | 1 | clang `enable_if` attribute (attr + overload viability, "candidate disabled: <msg>") | `_CmpUnspecifiedParam`'s literal-0 constructor (`<__compare/ordering.h>`); kiln ignores the attribute and accepts `strong_ordering < 1` | `dcl.attr/enable-if-attr.cpp` | `cmp.categories.pre/reject-other-than-literal-zero.verify` |
| (a) | 1 | Modules: an imported unit built in-process keeps its GMF declarations apart from the importer's GMF (spec/modules.md §6) | `std.compat.cppm` over `std.cppm`, both GMFs including `<cstddef>`: "missing '#include <__cstddef/byte.h>'; 'byte' must be declared before it is used" at `__type_traits/is_trivially_lexicographically_comparable.h:50` | `module.global.frag/gmf-dedup-import-gmf.cpp` (+ `Inputs/gdx-*.cppm`) | `std/modules/std.compat.pass` |
| (b) | 14 | diagnostic wording: `[[nodiscard]]` reach | missing "ignoring return value of function declared with 'nodiscard' attribute"; one sub-cause is a `[[nodiscard]]` conversion function called by `static_cast` (kiln: "expression result unused"), others are `get`, `any_cast`, ranges iterators, `<chrono>` | `.verify` tests | `nodiscard.verify` in array, complex.number, numarray, range.filter, range.join.with ×3, range.iota.view, range.repeat.view, time, any, tuple, pairs, variant |
| (b) | 5 | diagnostic wording: `nonnull` | missing "null passed to a callee that requires a non-null argument" | `.verify` tests | `nonnull.verify` in specialized.algorithms, mem.res, print.fun, basic.string, string.view |
| (b) | 4 | diagnostic wording: `lifetimebound` | missing "temporary bound to local reference '…' will be destroyed at the end of the full-expression" | `.verify` tests | `lifetimebound.verify` in algorithms, as-lvalue, function.objects, utility/forward |
| (b) | 3 | diagnostic wording: dependent member type | "no type named 'X' in 'Y'" / "in namespace 'std'" reported as "expected a type" | `.verify` tests | `unexpected_disabled_cpp17`, `futures.task.members/type`, `variant.hash/hash.depr` (WP-445) |
| (b) | 2 | diagnostic wording: thread-safety analysis | missing "writing variable '…' requires holding shared_mutex '…' exclusively" (`-Wthread-safety`) | `.verify` tests | `thread.shared_mutex`, `thread.shared_timed_mutex` `thread_safety.verify` |
| (b) | 2 | diagnostic wording: deprecation | missing "'…' is deprecated" on `rel_ops` and the C++17 adaptor typedefs | `.verify` tests | `rel_ops.depr_in_cxx20`, `typedefs.depr_in_cxx17` |
| (b) | 1 | diagnostic wording: returning a reference to a temporary | missing "returning reference to local temporary object" in `<tuple>` | `.verify` test | `make_from_tuple.verify` (WP-445) |
| (b) | 1 | diagnostic wording: constraint note | the "because … evaluated to false" note prints the substituted expression | `.verify` test | `libcxx/algorithms/cpp17_iterator_concepts.verify` |
| (b) | 1 | diagnostic wording: `alloc_align` | missing "requested alignment is not a power of 2" | `.verify` test | `allocate_vocabulary.attributes.verify` |
| (c) | 1 | harness: `-fexperimental-library` | kiln++ rejects the driver flag ("unknown argument") | the test itself | `libcxx/experimental/fexperimental-library.compile.pass` (driver `args.rs`, WP-445) |
| (c) | 1 | harness: scratch path length | `copy_file.pass` fails under the harness's long scratch `TMPDIR` (110+ characters) with kiln **and** clang 22, passes with a short one | the test, `TMPDIR` shortened | `fs.op.copy_file/copy_file.pass` |
| (d) | 1 | — | `RLIMIT_NPROC` does not limit root: thread creation never fails (clang 22 fails too) | — | `futures.async/thread_create_failure.pass` |
| (d) | 1 | — | the 3072 MiB address-space cap is below two `INT_MAX` strings (clang 22 fails too) | — | `stringstream.members/gcount.pass` |

Both (d) tests and `copy_file` are candidates for the environment
exclusion lists (`xtask/src/libcxx/run.rs`, `ROOT_UNSUPPORTED` and a
memory-cap list), which turns them into UNSUPPORTED with a reason.

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
