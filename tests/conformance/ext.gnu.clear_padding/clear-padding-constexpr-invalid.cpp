// __builtin_clear_padding in a constant expression may only name storage
// the evaluation may modify: an object outside the evaluation, a null
// or past-the-end pointer and freed storage make the call
// non-constant. Each function is also never constant (-Winvalid-constexpr,
// [dcl.constexpr]/6, WP-873), as clang reports for a plain assignment.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang has no __builtin_clear_padding (LLVM issue 64830)
//@ strict-diags
//@ diag: 32:16 error "constexpr function never produces a constant expression"
//@ diag: 33:3 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:3 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 36:15 note "in call to 'global()'"
//@ diag: 37:16 error "constexpr function never produces a constant expression"
//@ diag: 39:3 note "assignment to dereferenced null pointer is not allowed in a constant expression"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:3 note "assignment to dereferenced null pointer is not allowed in a constant expression"
//@ diag: 42:15 note "in call to 'null()'"
//@ diag: 43:16 error "constexpr function never produces a constant expression"
//@ diag: 45:3 note "assignment to dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:3 note "assignment to dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 48:15 note "in call to 'past()'"
//@ diag: 49:16 error "constexpr function never produces a constant expression"
//@ diag: 52:3 note "assignment to heap allocated object that has been deleted"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:3 note "assignment to heap allocated object that has been deleted"
//@ diag: 55:15 note "in call to 'freed()'"
struct S { char c; int i; };
S g;
constexpr bool global() {
  __builtin_clear_padding(&g);
  return true;
}
static_assert(global());
constexpr bool null() {
  S *p = nullptr;
  __builtin_clear_padding(p);
  return true;
}
static_assert(null());
constexpr bool past() {
  S s{};
  __builtin_clear_padding(&s + 1);
  return true;
}
static_assert(past());
constexpr bool freed() {
  S *p = new S{};
  delete p;
  __builtin_clear_padding(p);
  return true;
}
static_assert(freed());
