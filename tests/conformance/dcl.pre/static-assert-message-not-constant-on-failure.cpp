// [dcl.pre]/12 (P2741R3): only when the condition of a static_assert-declaration
// is false must `M.size()` and `M.data()` be constant expressions. A message
// that is not is diagnosed once, with the evaluation notes, before the
// failed assertion itself (clang's wording and order). In a template the
// message is evaluated on the failing instantiation only, once. Assertions
// that hold never evaluate the message:
// static-assert-message-unevaluated-when-true.cpp (WP-570; spec/sema.md §4.6).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:22 error "the message in a static assertion must be produced by a constant expression"
//@ diag: 34:22 note "non-constexpr function 'size' cannot be used in a constant expression"
//@ diag: 24:17 note "declared here"
//@ diag: 34:15 error "static assertion failed"
//@ diag: 35:33 error "the message in a static assertion must be produced by a constant expression"
//@ diag: 35:33 note "null pointer"
//@ diag: 35:15 error "static assertion failed due to requirement 'sizeof(int) == 0'"
//@ diag: 35:27 note "expression evaluates to '4 == 0'"
//@ diag: 36:50 error "the message in a static assertion must be produced by a constant expression"
//@ diag: 37:15 note "in instantiation of function template specialization 'f<0>' requested here"
//@ diag: 36:50 note "non-constexpr function 'size' cannot be used in a constant expression"
//@ diag: 24:17 note "declared here"
//@ diag: 36:43 error "static assertion failed due to requirement '0 > 0'"
struct Rt {
  unsigned long size() const { return 1; }
  const char* data() const { return "x"; }
};
struct View {
  const char* p;
  unsigned long n;
  constexpr const char* data() const { return p; }
  constexpr unsigned long size() const { return n; }
};
constexpr View null_data() { return View{nullptr, 1}; }
static_assert(false, Rt{});
static_assert(sizeof(int) == 0, null_data());
template <int N> void f() { static_assert(N > 0, Rt{}); }
template void f<0>();
