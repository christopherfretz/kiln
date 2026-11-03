// [temp.arg.template]/3: a template template parameter given as the argument
// of another is matched against it when the template is defined (clang's
// `CheckTemplateArgumentList` on a dependent template-id). clang 19
// spelling; dependent-ttp-argument-clang22.cpp has clang 22's.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 21:53 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 21:11 note "too few template parameters in template template argument"
//@ diag: 18:11 note "previous template template parameter is here"
//@ diag: 22:59 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 22:11 note "too many template parameters in template template argument"
//@ diag: 17:11 note "previous template template parameter is here"
//@ diag: 23:53 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 23:26 note "template parameter has a different kind in template argument"
//@ diag: 19:24 note "previous template template parameter is here"
template <template <class> class TT> struct B {};
template <template <class, class> class TT> struct B2 {};
template <template <int> class TT> struct BI {};

template <template <class> class TT> struct S1 { B2<TT> bad; B<TT> ok; };
template <template <class, class> class TT> struct S2 { B<TT> bad; B2<TT> ok; };
template <template <class> class TT> struct S3 { BI<TT> bad; };
template <template <class...> class TT> struct S4 { B<TT> ok1; B2<TT> ok2; };
template <template <class, class = int> class TT> struct S5 { B<TT> ok1; B2<TT> ok2; };
