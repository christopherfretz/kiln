// [temp.arg.template]/1 (P2841R7): a template-argument for a type template
// template parameter names a class or alias template, for a concept template
// parameter a concept, for a variable template template parameter a variable
// template. A mismatch is diagnosed with clang 22's wording and a note at
// the argument's declaration (spec/sema-templates.md §2.1.1). WP-746.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 27:3 error "template argument does not refer to a concept, or template template parameter"
//@ diag: 22:34 note "template argument refers to a variable template 'sz', here"
//@ diag: 28:3 error "template argument does not refer to a concept, or template template parameter"
//@ diag: 23:27 note "template argument refers to a class template 'X', here"
//@ diag: 29:3 error "template argument does not refer to a variable template, or template template parameter"
//@ diag: 21:28 note "template argument refers to a concept 'Int', here"
//@ diag: 30:3 error "template argument does not refer to a variable template, or template template parameter"
//@ diag: 23:27 note "template argument refers to a class template 'X', here"
//@ diag: 31:4 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 21:28 note "template argument refers to a concept 'Int', here"
//@ diag: 32:4 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 22:34 note "template argument refers to a variable template 'sz', here"
template <class T> concept Int = __is_same(T, int);
template <class T> constexpr int sz = sizeof(T);
template <class T> struct X {};
template <template <class> concept C> struct A {};
template <template <class> auto V> struct B {};
template <template <class> class K> struct Cl {};
A<sz> a1;
A<X> a2;
B<Int> b1;
B<X> b2;
Cl<Int> c1;
Cl<sz> c2;
A<Int> ok1; B<sz> ok2; Cl<X> ok3;
