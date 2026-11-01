# WP-118 progress — codegen follow-ups from WP-100

Branch `wp/118-codegen-new-cleanups`, worktree `kiln/.worktrees/wp-118`.
Details of what landed: spec/codegen.md §7d.

## 1. Array-new / member array ctor loops — done
Test `tests/codegen/new_array_ctor_loops.cpp`.

## 2. Array cookies — done
Tests `tests/codegen/new_array_cookie.cpp`, `expr.new/run-array-cookie`.
Also: sema picks sized global `operator delete[]` for destructed
elements; array-new marks the element dtor referenced (fixes nounwind of
implicitly-noexcept dtors); conditional regular partial-array cleanup
(nothrow new[]); partial-array EH cleanup in the `delete[]` destroy loop.

## 3. Aligned new cleanup delete — next
