// Run-time behavior warnings (-Wdivision-by-zero, -Wshift-count-overflow,
// -Wnull-dereference; clang's DiagRuntimeBehavior) are dropped where the
// innermost evaluation context is not potentially evaluated: manifestly
// constant-evaluated expressions ([expr.const]/20: static_assert, an
// enumerator, a template argument, a case value, a constexpr variable's
// initializer at namespace scope), whose failure the constant evaluator
// reports instead; discarded if-constexpr branches ([stmt.if]/2); and
// immediate function contexts ([expr.const]/16: a consteval body, the
// compound statement of if consteval). A variable's initializer is
// potentially evaluated again, so it still warns.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 45:21 note "division by zero"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:17 note "division by zero"
//@ diag: 47:15 error "expression is not an integral constant expression"
//@ diag: 47:17 note "division by zero"
//@ diag: 49:3 error "non-type template argument is not a constant expression"
//@ diag: 49:5 note "division by zero"
//@ diag: 50:3 error "non-type template argument is not a constant expression"
//@ diag: 50:3 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 51:15 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 51:21 note "shift count 40 >= width of type 'int' (32 bits)"
//@ diag: 52:12 warning "shift count >= width of type"
//@ diag: 60:17 error "constexpr variable 'w' must be initialized by a constant expression"
//@ diag: 60:23 note "division by zero"
//@ diag: 61:21 error "case value is not a constant expression"
//@ diag: 61:24 note "division by zero"
//@ diag: 56:36 warning "division by zero is undefined"
//@ diag: 56:49 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 56:49 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 57:43 warning "division by zero is undefined"
//@ diag: 60:23 warning "division by zero is undefined"
//@ diag: 62:13 warning "division by zero is undefined"
//@ diag: 64:33 warning "division by zero is undefined"
//@ diag: 65:30 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 65:30 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ diag: 67:36 error "in-class initializer for static data member is not a constant expression"
//@ diag: 67:52 warning "division by zero is undefined"
//@ diag: 69:57 warning "indirection of non-volatile null pointer will be deleted, not trap"
//@ diag: 69:57 note "consider using __builtin_trap() or qualifying pointer with 'volatile'"
//@ requires: clang<22
// clang 22 emits error: non-type template argument is not a constant expression; WP-355 residual
constexpr int a = 1 / 0;
static_assert(2 % 0 == 0);
enum E { e1 = 4 / 0 };
template <int N> struct T {};
T<5 / 0> t;
T<*(int *)0> t2;
constexpr int s = 1 << 40;
int ns = 1 << 33;
void f() {
  if constexpr (false) { (void)(3 / 0); (void)(1 + *(int *)0); (void)(1 << 33); }
  if constexpr (true) {} else { (void)(4 % 0); }
  if constexpr (false) { int y = 5 / 0; int r = *(int *)0; }
  if consteval { (void)(6 / 0); int z = 7 / 0; }
  if !consteval {} else { (void)(8 / 0); }
  static_assert(sizeof(int) || *(int *)0);
  constexpr int w = 9 / 0;
  switch (1) { case 10 % 0: break; }
  (void)(11 / 0);
}
consteval int c1() { int x = 12 / 0; return x + 13 / 0; }
consteval int c2() { int y = *(int *)0; return y + *(int *)0; }
constexpr int c3() { if consteval { return 14 / 0; } return 0; }
struct S { static const int m = 15 / 0; int n = 16 / 0; };
auto l = [] consteval { return *(int *)0; };
auto l2 = [] { if consteval { return 1; } else { return *(int *)0; } };
