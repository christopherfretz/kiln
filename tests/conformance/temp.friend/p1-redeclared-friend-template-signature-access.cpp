// [temp.friend]/1 with [temp.deduct.general]/8: a function template
// befriended by a redeclaration (before or after its definition) has its
// signature substituted with the friend's access, whether its template
// arguments are explicit or deduced; befriending the caller grants
// nothing, as access is checked from the template's declaration.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 40:21 error "no matching function for call to 'g'"
//@ diag: 33:28 note "candidate template ignored: substitution failure [with T = C]: 'J' is a private member of 'C'"
//@ diag: 47:3 error "no matching function for call to 'g'"
//@ diag: 33:28 note "candidate template ignored: substitution failure [with T = C]: 'J' is a private member of 'C'"

template <int I> struct B {};

template <class T> B<T::I> f();
template <class T> B<T::I> h(T);
class A {
  static const int I = 42;
  template <class T> friend B<T::I> f();
  template <class T> friend B<T::I> h(T);
};

template <class T> class Outer {
  struct Inner {
    Inner(int) {}
  };
  template <class U> friend typename Outer<U>::Inner make();
};
template <class U> typename Outer<U>::Inner make() { return 1; }

template <class T> B<T::J> g();
class C {
  static const int J = 1;
  friend int main();
  friend struct M;
};
struct M {
  static void m() { g<C>(); }
};

int main() {
  f<A>();
  h(A());
  make<int>();
  g<C>();
}
