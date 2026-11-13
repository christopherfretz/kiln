// [temp.deduct.type]/20: if `P` has a form that contains `<i>` and the type
// of `i` differs from the type of the corresponding template parameter of
// the template named by the enclosing simple-template-id, deduction fails
// (the standard's example: `template<short s> void f(A<s>)` against
// `A<1>`). The note is clang's `ConvertDeducedTemplateArgument` one.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:9 error "no matching function for call to 'f'"
//@ diag: 19:22 note "candidate template ignored: substitution failure: deduced non-type template argument does not have the same type as the corresponding template parameter ('unsigned long' vs 'int')"
//@ diag: 22:9 error "no matching function for call to 'g'"
//@ diag: 21:22 note "candidate template ignored: substitution failure: deduced non-type template argument does not have the same type as the corresponding template parameter ('char' vs 'int')"
//@ diag: 24:9 error "no matching function for call to 'h'"
//@ diag: 23:23 note "candidate template ignored: substitution failure: deduced non-type template argument does not have the same type as the corresponding template parameter ('int' vs 'long')"
template <unsigned long N> struct A {};
template <auto N> struct V {};
template <class T, T N> struct C {};
template <unsigned long N> int ok(const int (&)[N]) { return 0; }
template <int i> int f(A<i>) { return 1; }
int x = f(A<3>{});
template <int i> int g(V<i>) { return 1; }
int y = g(V<'c'>{});
template <long i> int h(V<i>) { return 1; }
int z = h(V<3>{});
template <int i> int k(C<int, i>) { return 1; }
template <int i> int m(V<i>) { return 1; }
int a[2];
int w = k(C<int, 3>{}) + m(V<3>{}) + ok(a);
