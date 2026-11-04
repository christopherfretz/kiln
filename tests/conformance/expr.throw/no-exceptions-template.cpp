// [expr.throw], [temp.res.general], [temp.inst]: with -fno-exceptions a
// throw-expression inside a template definition is diagnosed when the
// enclosing entity is instantiated, at the throw, with the instantiation
// note; a template that is never instantiated is accepted (GCC's
// dependent THROW_EXPR, so code that builds with GCC builds with kiln).
// Covers a function template (dependent and non-dependent operand), a
// member of a class template, a generic lambda and a non-template
// function (still an error at its definition). A location is reported
// once however many specializations use it. clang 19 diagnoses every
// template throw at its definition, so clang does not run this test.
// spec/sema.md "Exceptions disabled", WP-607.
//@ expect: compile-fail
//@ strict-diags
//@ args: -fno-exceptions
//@ skip(clang): clang diagnoses a template's throw at its definition; kiln follows GCC's deferral to the instantiation
//@ diag: 27:36 error "cannot use 'throw' with exceptions disabled"
//@ diag: 40:3 note "in instantiation of function template specialization 'dep<int>' requested here"
//@ diag: 28:36 error "cannot use 'throw' with exceptions disabled"
//@ diag: 42:3 note "in instantiation of function template specialization 'nondep<int>' requested here"
//@ diag: 31:14 error "cannot use 'throw' with exceptions disabled"
//@ diag: 43:12 note "in instantiation of member function 'S<int>::m' requested here"
//@ diag: 32:14 error "cannot use 'throw' with exceptions disabled"
//@ diag: 44:12 note "in instantiation of member function 'S<int>::n' requested here"
//@ diag: 35:29 error "cannot use 'throw' with exceptions disabled"
//@ diag: 45:10 note "in instantiation of function template specialization"
//@ diag: 38:16 error "cannot use 'throw' with exceptions disabled"
template <class T> void dep(T t) { throw t; }
template <class T> void nondep() { throw 1; }
template <class T> void never(T t) { throw t; }
template <class T> struct S {
  void m() { throw 1; }
  void n() { throw T(); }
  void unused() { throw 2; }
};
auto generic = [](auto x) { throw x; };
template <class T> void never_nondep() { throw; }

void plain() { throw 3; }
void use() {
  dep(1);
  dep(2L);
  nondep<int>();
  S<int>().m();
  S<int>().n();
  generic(1);
}
