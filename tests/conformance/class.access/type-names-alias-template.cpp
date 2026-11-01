// [class.access]/4, [temp.alias]/2: a type name in an alias template is access-checked
// at every use, reported at the alias's 'using' (clang substitutes with trivial
// TypeLocs); a namespace-scope alias checks from the use site, a member alias
// from its class (spec/sema-templates.md §4.10).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:19 error "'type' is a private member of 'A'"
//@ diag: 21:15 note "implicitly declared private here"
//@ diag: 26:19 error "'type' is a private member of 'A'"
//@ diag: 21:15 note "implicitly declared private here"
//@ diag: 29:19 error "'N' is a private member of 'A'"
//@ diag: 22:10 note "implicitly declared private here"
//@ diag: 26:19 error "'type' is a private member of 'A'"
//@ diag: 31:30 note "in instantiation of template type alias 'X' requested here"
//@ diag: 32:6 note "in instantiation of template class 'U<A>' requested here"
//@ diag: 21:15 note "implicitly declared private here"
//@ diag: 26:19 error "'type' is a private member of 'A'"
//@ diag: 39:11 note "in instantiation of template type alias 'X' requested here"
//@ diag: 21:15 note "implicitly declared private here"
class A {
  typedef int type;
  struct N { typedef int V; };
  friend struct F;
  friend int g();
};
template<class T> using X = typename T::type;
X<A> x1;
X<A> x2;
template<class T> using Y = typename T::N::V;
Y<A> y;
template<class T> struct U { X<T> m; };
U<A> u;
struct F {
  X<A> f;
  template<class T> using Z = typename T::type;
};
F::Z<A> z;
int g() { F::Z<A> v = 0; return v; }
int h() { X<A> v = 0; return v; }
template<class T> X<T> k(int);
template<class T> char k(...);
static_assert(sizeof(k<A>(0)) == 1);
