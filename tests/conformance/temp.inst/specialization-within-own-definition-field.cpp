// [temp.inst]/2, [class.mem.general]/8: a member of type `A<long>`, a
// nested class's member named through `C<short>::`, and a partial
// specialization naming another of its own specializations, all inside
// the definition being parsed, need a specialization whose pattern is not
// complete yet: "implicit instantiation of template ... within its own
// definition", with no note (clang `DiagnoseUninstantiableTemplate`);
// quiet once the member of incomplete type made the pattern invalid.
// Valid uses: specialization-within-own-definition-valid.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:11 error "implicit instantiation of template 'A<long>' within its own definition"
//@ diag: 23:23 error "implicit instantiation of template 'C<short>' within its own definition"
//@ diag: 29:13 error "implicit instantiation of template 'P<int *>' within its own definition"
template <class R> struct ptr { using string_t = R; };
template <class S> struct A {
  using jp = ptr<S>;
  A<long> m;
  struct B { typename A<short>::jp::string_t q; };
};
A<int> a;
template <class S> struct C {
  using jp = ptr<S>;
  struct B { typename C<short>::jp::string_t q; };
};
C<int> c;
template <class T> struct P;
template <class T> struct P<T *> {
  using jp = ptr<T>;
  using u = P<int *>::jp;
};
P<char *> p;
