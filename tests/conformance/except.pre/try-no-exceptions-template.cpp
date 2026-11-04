// [except.pre], [temp.inst]: with -fno-exceptions a try-block inside a
// template definition is diagnosed when the enclosing entity is
// instantiated, with the instantiation note, and a never-instantiated one
// is accepted (GCC's rule); a non-template function's is still an error
// at its definition. One report per location. clang 19 diagnoses every
// template try at its definition, so clang does not run this test.
// spec/sema.md "Exceptions disabled", WP-607.
//@ expect: compile-fail
//@ strict-diags
//@ args: -fno-exceptions
//@ skip(clang): clang diagnoses a template's try at its definition; kiln follows GCC's deferral to the instantiation
//@ diag: 19:32 error "cannot use 'try' with exceptions disabled"
//@ diag: 29:3 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 22:14 error "cannot use 'try' with exceptions disabled"
//@ diag: 31:12 note "in instantiation of member function 'S<int>::m' requested here"
//@ diag: 25:27 error "cannot use 'try' with exceptions disabled"
//@ diag: 32:10 note "in instantiation of function template specialization"
//@ diag: 27:16 error "cannot use 'try' with exceptions disabled"
template <class T> void f(T) { try {} catch (T) {} }
template <class T> void never(T) { try {} catch (...) {} }
template <class T> struct S {
  void m() { try {} catch (...) {} }
  void unused() { try {} catch (...) {} }
};
auto generic = [](auto) { try {} catch (...) {} };

void plain() { try {} catch (...) {} }
void use() {
  f(1);
  f(2L);
  S<int>().m();
  generic(1);
}
