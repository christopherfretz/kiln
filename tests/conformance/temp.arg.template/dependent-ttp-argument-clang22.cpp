// [temp.arg.template]/3: a template template parameter given as the argument
// of another is matched against it when the template is defined (clang's
// `CheckTemplateArgumentList` on a dependent template-id). clang 22
// spelling (P0522 matching); dependent-ttp-argument.cpp has clang 19's.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 24:53 error "too many template arguments for template template parameter 'TT'"
//@ diag: 24:53 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 21:41 note "previous template template parameter is here"
//@ diag: 24:34 note "template is declared here"
//@ diag: 25:59 error "too few template arguments for template template parameter 'TT'"
//@ diag: 25:59 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 20:34 note "previous template template parameter is here"
//@ diag: 25:41 note "template is declared here"
//@ diag: 22:24 error "template argument for template type parameter must be a type"
//@ diag: 26:53 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 22:32 note "previous template template parameter is here"
//@ diag: 26:26 note "template parameter is declared here"
template <template <class> class TT> struct B {};
template <template <class, class> class TT> struct B2 {};
template <template <int> class TT> struct BI {};

template <template <class> class TT> struct S1 { B2<TT> bad; B<TT> ok; };
template <template <class, class> class TT> struct S2 { B<TT> bad; B2<TT> ok; };
template <template <class> class TT> struct S3 { BI<TT> bad; };
template <template <class...> class TT> struct S4 { B<TT> ok1; B2<TT> ok2; };
template <template <class, class = int> class TT> struct S5 { B<TT> ok1; B2<TT> ok2; };
