// [expr.prim.id.qual]/1: a qualified-id names its entity through the
// nested-name-specifier as written; diagnostics print the expression with
// that specifier (`N::x`, `::z`, `A<int>::y`) and point at its first token,
// also after the qualifier was substituted in an instantiation
// ([temp.inst], clang's `TransformDeclRefExpr`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion failed due to requirement 'N::x == 2'"
//@ diag: 20:20 note "expression evaluates to '1 == 2'"
//@ diag: 21:15 error "static assertion failed due to requirement '::z == 3'"
//@ diag: 21:19 note "expression evaluates to '2 == 3'"
//@ diag: 22:15 error "static assertion failed due to requirement 'A<int>::y == 4'"
//@ diag: 22:25 note "expression evaluates to '3 == 4'"
//@ diag: 25:17 error "static assertion failed due to requirement 'A<int>::y + N::x == 5'"
//@ diag: 27:8 note "in instantiation of template class 'B<int>' requested here"
//@ diag: 25:32 note "expression evaluates to '4 == 5'"
namespace N { constexpr int x = 1; }
constexpr int z = 2;
template <class T> struct A { static constexpr int y = 3; };
static_assert(N::x == 2);
static_assert(::z == 3);
static_assert(A<int>::y == 4);
template <class T>
struct B {
  static_assert(A<T>::y + N::x == 5);
};
B<int> b;
