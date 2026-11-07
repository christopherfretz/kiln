// [temp.param]/1, [temp.arg.template]/1: the default argument of a
// concept-tt-parameter names a concept and that of a variable-tt-parameter a
// variable template, and a placeholder constrained by a concept template
// parameter still cannot declare a non-static data member. clang 22's
// wording (spec/sema-templates.md §2.1.1). WP-746.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 16:40 error "template argument does not refer to a concept, or template template parameter"
//@ diag: 15:34 note "template argument refers to a variable template 'sz', here"
//@ diag: 17:37 error "template argument does not refer to a variable template, or template template parameter"
//@ diag: 14:28 note "template argument refers to a concept 'Int', here"
//@ diag: 18:54 error "'auto' not allowed in non-static struct member"
template <class T> concept Int = __is_same(T, int);
template <class T> constexpr int sz = sizeof(T);
template <template <class> concept C = sz> struct A7 {};
template <template <class> auto V = Int> struct A8 {};
template <template <class> concept C> struct A12 { C auto x = 1; };
