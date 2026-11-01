# WP-151 progress — ambiguous member calls, simd_utils vectors, dependent access

Branch `wp/151-ambiguous-member-simd`, worktree `kiln/.worktrees/wp-151`,
base c4fb6bd (merged with main 0b5156c before CI). Also carries the WP-150
simd_utils.h:101 item.

libc++ M3 gate before: 312 pass / 273 fail (642 files).

## Items
1. [x] "call to member function '…' is ambiguous" (`::operator new(n, al,
   nothrow)` in `__memory/temporary_buffer.h` and similar). Root cause:
   the implicit global `operator new`/`delete` declarations were made
   lazily, the first time they were looked up. When that happened inside a
   template definition (`dependent_depth > 0`), `std::align_val_t` was
   declared as a dependent type. The aligned overloads then never matched,
   and the remaining candidates tied. Fix: `declare_global_new_delete`
   (`kiln_sema/src/new_delete.rs`) zeroes `dependent_depth` while it runs.
   The bug was not in using.rs. Test:
   `basic.stc.dynamic/implicit-align-val-t-in-template.cpp`.
2. [x] Vector types: `ext_vector_type(N)` and `vector_size(N)`.
   - Both attributes are now parsed with expression arguments.
   - They build `Vector`/`DependentVector` types on typedefs and aliases;
     `vector_size` also works on variables, fields and parameters
     (`kiln_sema/src/vector.rs`).
   - Substitution rebuilds dependent vectors, so the variable-template
     partial specialization `__simd_vector_size_v<__simd_vector<T, N>>`
     (simd_utils.h:101) matches and deduces.
   - Layout follows clang 19 getTypeInfo: a bool ext vector packs 1 bit per
     element, rounded up to at least 8 bits and to a power of two, with
     align = size capped at 16 bytes on aarch64 (`kiln_abi` layout).
   - The Oven gets vector sizes through `SemaBridge::opaque_size_align`.
   - Tests: `ext.vector/{dependent-alias,diagnostics,sizes}.cpp` and
     `tests/layout/ext_vector_bool.cpp` (N = 1…64).
   - Vector operations (`~v`, `__builtin_convertvector`,
     `__builtin_reduce_and`) are M4.
3. [x] Access granted only through a befriended specialization of a class
   template (e.g. `friend struct B<int>` matched speculatively against the
   pattern's `B<T>`) is recorded per pattern. It is replayed at each class,
   member class and member function instantiation, the way clang's
   `PerformDependentDiagnostics` does (`class/access.rs`
   `replay_dependent_access`, `templ/class.rs`). Test:
   `temp.friend/befriended-spec-access-reinstantiated.cpp`. Not covered:
   access to type names (pre-existing), and function-template
   specializations and lambdas (no replay hook).
4. [x] `act_on_param_default_list` review.
   - Braced default arguments of members of nested classes and of templates
     are handled correctly.
   - Found and fixed [dcl.init.list]/3.2: an aggregate initialized from
     `{x}`, where `x` is of the same or a derived class, is now initialized
     from `x` (`init.rs` `init_from_list_at`). Test:
     `dcl.init.list/aggregate-same-type-element.cpp`.
   - The partial-specialization-friend diagnostic range now covers only
     `<U*>`.
   - Gaps left as follow-ups:
     - default arguments of class-template members are instantiated
       eagerly; [temp.inst]/12 instantiates them at the call that uses
       them;
     - the "passing argument to parameter here" note is missing;
     - after an invalid default argument kiln also reports "too few
       arguments" at the call, which clang does not.
