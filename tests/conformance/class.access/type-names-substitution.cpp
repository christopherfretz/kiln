// [class.access]/4, [temp.deduct.general]/8: T::type is access-checked after
// substitution; in a SFINAE context an inaccessible one is a deduction failure.
// Both are reported at the name as written (spec/sema-templates.md §5.8).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:42 error "'type' is a private member of 'A'"
//@ diag: 24:6 note "in instantiation of template class 'W<A>' requested here"
//@ diag: 13:15 note "implicitly declared private here"
//@ diag: 25:42 error "'N' is a private member of 'A'"
//@ diag: 26:6 note "in instantiation of template class 'G<A>' requested here"
//@ diag: 14:10 note "implicitly declared private here"
class A {
  typedef int type;
  struct N { typedef int V; };
  friend struct F;
};
struct P { typedef long type; };
template<class T> typename T::type f(int) { return 0; }
template<class T> char f(...) { return 0; }
static_assert(sizeof(f<A>(0)) == 1);
static_assert(sizeof(f<P>(0)) == sizeof(long));
template<class T> struct W { typename T::type v; };
W<P> wp;
W<A> wa;
template<class T> struct G { typename T::N::V v; };
G<A> ga;
struct F {
  template<class T> static int g() { typename T::type x = 0; return x; }
};
int z = F::g<A>();
