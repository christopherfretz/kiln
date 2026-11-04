// [temp.explicit]/4, [temp.deduct.decl], [temp.func.order], [temp.mem]/6:
// an explicit instantiation by signature that deduces no member function
// template (constructor, member, conversion function template — whose
// failed candidates lookup never finds, so none is noted) or no free one
// is "does not refer to a function template", with a note at each
// candidate's most recent declaration; one that two templates match with
// neither more specialized is "partial ordering for explicit
// instantiation ... is ambiguous", the candidates in declaration order
// (clang's `err_explicit_instantiation_ambiguous`). WP-580.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:13 error "explicit instantiation of 'A' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 34:23 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 35:23 note "candidate template ignored: could not match 'int' against 'long'"
//@ diag: 39:17 error "explicit instantiation of 'f' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 36:27 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 40:13 error "explicit instantiation of 'operator int' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 48:13 error "partial ordering for explicit instantiation of 'B' is ambiguous"
//@ diag: 46:32 note "explicit instantiation candidate function 'B::B<int *, int>' template here [with T = int *, U = int]"
//@ diag: 47:32 note "explicit instantiation candidate function 'B::B<int, int *>' template here [with T = int, U = int *]"
//@ diag: 53:22 error "partial ordering for explicit instantiation of 'g' is ambiguous"
//@ diag: 52:34 note "explicit instantiation candidate function 'g<int *, int>' template here [with T = int *, U = int]"
//@ diag: 51:34 note "explicit instantiation candidate function 'g<int, int *>' template here [with T = int, U = int *]"
//@ diag: 56:15 error "explicit instantiation of 'h' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 55:25 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 62:20 error "explicit instantiation of 'C' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 60:22 note "candidate template ignored: could not match 'T *' against 'char'"
struct A {
  template <class T> A(T *);
  template <class T> A(T, int = 0);
  template <class T> int f(T *);
  template <class T> operator T *();
};
template <class T> A::A(T *) {}
template <class T> A::A(T, int) {}
template <class T> int A::f(T *) { return 0; }
template <class T> A::operator T *() { return nullptr; }
template A::A(int, long);
template int A::f(int);
template A::operator int();

struct B {
  template <class T, class U = int> B(T, U *);
  template <class T, class U> B(T *, U);
};
template <class T, class U> B::B(T, U *) {}
template <class T, class U> B::B(T *, U) {}
template B::B(int *, int *);

template <class T, class U> void g(T, U *);
template <class T, class U> void g(T *, U);
template <class T, class U> void g(T, U *) {}
extern template void g(int *, int *);
template <class T> void h(T *);
template <class T> void h(T *) {}
template void h(int);

struct C {
  C(int);
  template <class T> C(T *);
};
extern template C::C(char);
