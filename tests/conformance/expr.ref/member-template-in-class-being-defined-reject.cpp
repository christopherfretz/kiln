// [expr.ref]/4: the rejected side of
// member-template-in-class-being-defined.cpp. A function template that is
// not a member of the class has no `this` ([expr.prim.this]/2): substituted
// before the class's closing '}', its member access into the class is
// ill-formed, as in clang.
//@ args: -std=c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:23 error "no matching function for call to 'g'"
//@ diag: 16:25 note "candidate template ignored: substitution failure [with U = A]: member access into incomplete type 'const A'"
//@ diag: 26:23 error "no matching function for call to 'g'"
//@ diag: 28:8 note "in instantiation of template class 'B<int>' requested here"
//@ diag: 16:25 note "candidate template ignored: substitution failure [with U = B<int>]: member access into incomplete type 'const B<int>'"
template <class T> T&& declval();

template <class U> auto g(int) -> decltype(declval<U const&>().get_impl());

struct A {
  int get_impl() const;
  using t1 = decltype(g<A>(0));
};

template <class T>
struct B {
  int get_impl() const;
  using t1 = decltype(g<B>(0));
};
B<int> b;
