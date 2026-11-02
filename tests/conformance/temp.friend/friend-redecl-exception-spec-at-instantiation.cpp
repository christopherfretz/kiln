// [except.spec]/5, [temp.inst]/14, [temp.friend]/1: a friend of a class
// template redeclaring a namespace-scope function has its noexcept-specifier
// instantiated with the class, and it must agree with the earlier
// declaration's: `noexcept(false)` against `noexcept` is a mismatch, and a
// later friend is compared with the first declaration, not with an earlier
// friend that was itself mismatched (clang `CheckEquivalentExceptionSpec`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 38:8 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 19:6 note "previous declaration is here"
//@ diag: 26:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 20:6 note "previous declaration is here"
//@ diag: 35:15 error "exception specification in declaration does not match previous declaration"
//@ diag: 42:9 note "in instantiation of template class 'B<long>' requested here"
//@ diag: 32:15 note "previous declaration is here"

template <class T> struct A;
void f(A<int>&) noexcept;
void g(A<int>&);
void k(A<int>&) noexcept;
void m(A<long>&) noexcept;

template <class T> struct A {
  friend void f(A&) noexcept(sizeof(T) > 4);
  friend void g(A&) noexcept(sizeof(T) >= 4);
  friend void k(A&) noexcept(sizeof(T) >= 4);
  friend void m(A&) noexcept(sizeof(T) >= 4);
};

template <class T> struct H {
  friend void h(int) noexcept(sizeof(T) > 4);
};
template <class T> struct B {
  friend void h(int) noexcept(sizeof(T) > 4);
};

A<int> a;
A<long> b;
static_assert(noexcept(m(b)));
H<int> e;
B<long> c;
B<int> d;
