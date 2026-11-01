// [namespace.udecl]/8, [temp.res.general]/4: a block-scope using-declaration
// with a dependent nested-name-specifier is checked at instantiation. One
// naming an enumerator of a class template specialization declares it (its
// uses find the enumerator); one naming a class member is ill-formed (use a
// reference instead, noted at the 'using'), and the uses of the failed
// declaration are not diagnosed again.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:12 error "using declaration cannot refer to class member"
//@ diag: 29:9 note "in instantiation of function template specialization 'f<A>' requested here"
//@ diag: 25:3 note "use a reference instead"
//@ diag: 25:12 error "using declaration cannot refer to class member"
//@ diag: 30:9 note "in instantiation of function template specialization 'f<B>' requested here"
namespace N { template<class T> struct E { enum { a, b }; }; }
template<class T> constexpr int g() {
  using N::E<T>::b;
  int x = b;
  return b + x;
}
static_assert(g<int>() == 2);

struct A { static int v; };
struct B {};
template<class T> int f() {
  using T::v;
  int a = v;
  return v + a;
}
int x = f<A>();
int y = f<B>();
