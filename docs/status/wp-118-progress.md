# WP-118 progress — codegen follow-ups from WP-100

Branch `wp/118-codegen-new-cleanups`, worktree `kiln/.worktrees/wp-118`.
Details: spec/codegen.md §7g (codegen), spec/sema.md §7.5 (sema).

## 1. Array-new / member array ctor loops — done
Test `tests/codegen/new_array_ctor_loops.cpp`.

## 2. Array cookies — done
Tests `tests/codegen/new_array_cookie.cpp`, `expr.new/run-array-cookie`.
Also: sema picks sized global `operator delete[]` for destructed
elements; array-new marks the element dtor referenced (fixes nounwind of
implicitly-noexcept dtors); conditional regular partial-array cleanup
(nothrow new[]); partial-array EH cleanup in the `delete[]` destroy loop.

## 3. Aligned new cleanup delete — done
Test `tests/codegen/eh_new_aligned.cpp`.

## 4. Deferred EH cleanups — partial
- Done: partial-array EH cleanup in normal-path array destroy loops
  (locals, array members, static arrays); tests
  `tests/codegen/eh_array_destroy_throwing.cpp`,
  `except.ctor/run-array-dtor-throws`.
- Deferred (still `CodegenError` / noted in spec §7b Left):
  lifetime-extended temporaries' deferred EH cleanups; conditional
  temporaries at a computed address; VTT in a delegating ctor's EH
  cleanup with virtual bases; conditional `new` with a class-type
  placement argument.

## 5. Smaller items — done
- `unreachable` after `[[noreturn]]`, static-local guards (WP-178), anon
  union type name (`tests/codegen/anon_union_names.cpp`): already done.
- -Wexceptions member refs in ctor/dtor function-try-block handlers:
  `except.handle/cdtor-handler-member.cpp`.
- Throw-escape reachability (`try_stmt/reach.rs`):
  `except.terminate/throw-in-noexcept-reach.cpp`.

## Follow-ups found
- Mangling: namespace-scope anonymous union variable `_ZLUt_` vs clang
  `_Z2ga` (WP-117 area).
- GNU case ranges unsupported in sema (`case 1 ... 3`).
