// [expr.reflect]/7 (P2996R13), [class.access]/4: the name in `^^C::m` is
// access-checked like any other use of it: a private or protected data
// member, member type, member template or alias named outside the class
// (and its friends) is ill-formed, also when `^^T::m` is instantiated.
// A splice is not access-checked ([expr.prim.splice]); `^^` is the check.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 37:25 error "'priv' is a private member of 'C'"
//@ diag: 29:7 note "implicitly declared private here"
//@ diag: 38:25 error "'sp' is a private member of 'C'"
//@ diag: 30:14 note "implicitly declared private here"
//@ diag: 39:25 error "'N' is a private member of 'C'"
//@ diag: 31:10 note "implicitly declared private here"
//@ diag: 40:25 error "'TN' is a private member of 'C'"
//@ diag: 32:27 note "implicitly declared private here"
//@ diag: 41:25 error "'A' is a private member of 'C'"
//@ diag: 33:9 note "implicitly declared private here"
//@ diag: 42:25 error "'prot' is a protected member of 'C'"
//@ diag: 35:7 note "declared protected here"
//@ diag: 43:55 error "'priv' is a private member of 'C'"
//@ diag: 44:20 note "in instantiation of function template specialization 'dep<C>' requested here"
//@ diag: 29:7 note "implicitly declared private here"
#include "kiln_meta_queries.h"
class C {
  int priv;
  static int sp;
  struct N {};
  template <class> struct TN {};
  using A = int;
protected:
  int prot;
};
constexpr auto a = ^^C::priv;
constexpr auto b = ^^C::sp;
constexpr auto c = ^^C::N;
constexpr auto d = ^^C::TN;
constexpr auto e = ^^C::A;
constexpr auto g = ^^C::prot;
template <class T> consteval auto dep() { return ^^T::priv; }
constexpr auto h = dep<C>();
