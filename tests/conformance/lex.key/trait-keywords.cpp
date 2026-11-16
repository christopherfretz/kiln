// clang 22's type-trait keywords used as identifiers (spec/parse.md §1,
// "Trait keywords"): a revertible trait name is a keyword until
// `struct name` reverts it for the rest of the translation unit, with a
// warning; a non-revertible one is a keyword everywhere; followed by `(`
// both are the trait. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 21:16 error "expected member name or ';' after declaration specifiers"
//@ diag: 22:5 error "expected unqualified-id"
//@ diag: 23:24 error "expected member name or ';' after declaration specifiers"
//@ diag: 25:27 warning "keyword '__is_void' will be made available as an identifier for the remainder of the translation unit"
//@ diag: 32:30 warning "keyword '__is_signed' will be made available as an identifier for the remainder of the translation unit"
//@ diag: 36:16 error "expected member name or ';' after declaration specifiers"
//@ diag: 37:15 error "expected member name or ';' after declaration specifiers"

// Rejected: never-reverted names, and revertible ones before any `struct`.
// Accepted: the trait calls, the reverted names as identifiers, and the
// names that are not keywords.

struct A { int __is_convertible; };
int __has_trivial_copy;
struct B { long double __is_pod; };

template <class T> struct __is_void { static const bool value = false; };
static_assert(!__is_void<int>::value);
struct C { int __is_void; };
static_assert(__is_void(void));
static_assert(!__is_convertible(int*, long));
int __is_nullptr;

struct N { static const bool __is_signed = true; };
static_assert(__is_signed(int) && N::__is_signed);
int __remove_cv;

struct D { int __is_trivially_destructible; };
class E { int __is_layout_compatible; };
