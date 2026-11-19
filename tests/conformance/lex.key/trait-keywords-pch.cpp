// clang 22's revertible type-trait keywords through a precompiled header
// (spec/parse.md §1, "Trait keywords", and "Snapshots"): a name the header
// reverted (`struct __is_arithmetic`, `struct __is_void`, `static const
// bool __is_signed`) stays an identifier in the file that includes the
// PCH, as clang keeps the revert in its AST file; followed by `(` it is
// still the trait, and a name the header did not revert is still a
// keyword. Reduced from libstdc++'s <cmath> after a PCH of <iostream>
// (WP-1721, a regression since 1.2.1).
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ pch: Inputs/trait_keywords_reverted.h
//@ diag: 22:5 error "expected unqualified-id"

static_assert(__is_arithmetic<int>::__value == 1);
__is_void v{3};
int __is_signed = N::__is_signed;
static_assert(__is_arithmetic(int));
static_assert(__is_void(void));
static_assert(__is_signed(int));

int __is_unsigned;
