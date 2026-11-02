// [temp.expl.spec]/2, [temp.dep.general]/2: a class-scope explicit
// specialization `template <> void g(int);` in a dependent class whose
// lookup finds no member function template of the name is "no candidate
// function template was found for dependent member function template
// specialization" at the declarator-id, with "candidate ignored: not a
// function template" at each non-template found (clang's
// `CheckDependentFunctionTemplateSpecialization`). In a non-dependent class
// it is the ordinary "no function template matches". A matching candidate
// is accepted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:20 error "no candidate function template was found for dependent member function template specialization"
//@ diag: 22:20 error "no candidate function template was found for dependent member function template specialization"
//@ diag: 21:8 note "candidate ignored: not a function template"
//@ diag: 27:20 error "no function template matches function template specialization 'g'"
//@ diag: 31:22 error "no candidate function template was found for dependent member function template specialization"
template <class T> struct A {
  template <class U> void f(U);
  template <> void f(int);
  template <> void g(int);
  void h(int);
  template <> void h(int);
  template <class U> static int s(U);
  template <> int s<char>(char);
};
struct B {
  template <> void g(int);
};
template <class T> struct O {
  template <class V> struct I {
    template <> void k(V);
  };
};
A<int> a;
