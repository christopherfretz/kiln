// [expr.throw], [except.pre], [temp.res.general]: with -fno-exceptions a
// throw-expression or try-block in a template that is never instantiated
// is accepted -- function templates, class template members, a generic
// lambda -- as GCC accepts them (its dependent THROW_EXPR; compared with
// GCC 16.1, spec/harness.md "GCC oracle"). clang 19 rejects them at their
// definition, so clang does not run this test. spec/sema.md "Exceptions
// disabled", WP-607.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fno-exceptions
//@ oracle: gcc
//@ skip(clang): clang diagnoses a template's throw and try at their definition; kiln follows GCC's deferral to the instantiation
template <class T> void dep(T t) { throw t; }
template <class T> void nondep() { throw 1; }
template <class T> void rethrow() { throw; }
template <class T> int tried(T t) {
  try {
    return t;
  } catch (...) {
    return 0;
  }
}
template <class T> struct S {
  void m() { throw T(); }
  void n() { try {} catch (const T &) {} }
};
auto generic = [](auto x) { if (x) throw x; try {} catch (...) {} };
S<int> instantiated_class_only;
