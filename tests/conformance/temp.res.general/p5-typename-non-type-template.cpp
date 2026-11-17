// [temp.res.general]/5: a typename-specifier names a type: one naming a
// function or variable template specialization is ill-formed (clang's
// recovery adds further errors, so the diagnostics are not strict).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ diag: 11:24 error "typename specifier refers to a non-type template"
struct A {
  template<int> void foo(int i);
  template<int> static int v;
};
void g() { typename A::foo<0> *p; }
