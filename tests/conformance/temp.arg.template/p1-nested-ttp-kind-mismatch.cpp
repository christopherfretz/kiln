// [temp.arg.template]/1, [temp.over.link]/6 (P2841R7): when a template
// template argument is matched against a template template parameter whose
// own head has a nested template template parameter, the nested parameter
// kinds (type, variable or concept template template parameter) must agree.
// clang 22 reports the argument's nested parameter as not referring to a
// template of the expected kind, then the head-mismatch notes
// (spec/sema-templates.md §2.1.1). WP-771.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:45 error "template argument does not refer to a class or alias template, or template template parameter"
//@ diag: 37:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 30:53 note "previous template template parameter is here"
//@ diag: 30:45 note "template argument refers to a concept '', here"
//@ diag: 30:45 error "template argument does not refer to a variable template, or template template parameter"
//@ diag: 38:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 30:53 note "previous template template parameter is here"
//@ diag: 30:45 note "template argument refers to a concept '', here"
//@ diag: 34:42 error "template argument does not refer to a concept, or template template parameter"
//@ diag: 40:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 34:50 note "previous template template parameter is here"
//@ diag: 34:42 note "template argument refers to a variable template '', here"
//@ diag: 35:43 error "template argument does not refer to a concept, or template template parameter"
//@ diag: 42:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 35:51 note "previous template template parameter is here"
//@ diag: 35:43 note "template argument refers to a class template '', here"
template <class T> concept Int = __is_same(T, int);
template <class T> constexpr bool vt = true;
template <class T> struct ct {};
template <template <template <class> concept> class K> struct A {};
template <template <class> concept C> struct XC {};
template <template <class> auto V> struct XV {};
template <template <class> class T> struct XT {};
template <template <template <class> auto> class K> struct B {};
template <template <template <class> class> class K> struct D {};
A<XC> a1;
A<XT> a2;
A<XV> a3;
B<XV> b1;
B<XC> b2;
D<XT> d1;
D<XC> d2;
