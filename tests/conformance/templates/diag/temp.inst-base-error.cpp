// [class.derived.general]/2, [class.mi]/3: substituted base-specifiers
// must name distinct classes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:43 error "base class 'B' specified more than once as a direct base class"
//@ diag: 11:9 note "in instantiation of template class 'D<B, B>' requested here"
//@ diag: 12:31 error "base specifier must name a class"
//@ diag: 13:8 note "in instantiation of template class 'E<int>' requested here"
struct B {};
template <class T, class U> struct D : T, U {};
D<B, B> d;
template <class T> struct E : T {};
E<int> e;
