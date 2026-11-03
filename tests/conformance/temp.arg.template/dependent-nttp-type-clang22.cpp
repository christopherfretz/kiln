// [temp.arg.template]/3-4 (P0522, clang 22 matching: P's parameters as arguments to A): a template template
// parameter's non-type parameter whose type names an earlier parameter of the enclosing template
// is matched with the earlier arguments substituted; the argument's parameters are not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:31 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 36:9 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 26:39 note "previous template template parameter is here"
//@ diag: 27:16 note "template parameter is declared here"
//@ diag: 26:31 error "value of type 'int *' is not implicitly convertible to 'int'"
//@ diag: 37:11 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 26:39 note "previous template template parameter is here"
//@ diag: 28:14 note "template parameter is declared here"
//@ diag: 39:40 error "conversion from 'int' to 'char *' is not allowed in a converted constant expression"
//@ diag: 43:9 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 39:48 note "previous template template parameter is here"
//@ diag: 41:24 note "template parameter is declared here"
//@ diag: 48:26 error "value of type 'int *' is not implicitly convertible to 'int'"
//@ diag: 46:48 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 48:33 note "previous template template parameter is here"
//@ diag: 50:9 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 46:56 note "previous template template parameter is here"
//@ diag: 46:41 note "template parameter is declared here"
//@ requires: clang>=22
// clang 22 spelling of dependent-nttp-type.cpp (WP-394; spec/sema-templates.md §3.2.1).
template <class T, template <T> class P> struct DP {};
template <int *> struct IP;
template <int> struct II;
template <long> struct IL;
template <auto> struct IA;
template <class U, U> struct IU;
DP<int, II> ok1;
DP<int, IA> ok2;
DP<long, IL> ok3;
DP<int, IL> ok4;
DP<int, IP> bad1;
DP<int *, II> bad2;
DP<int *, IP> ok5;
template <class T, template <class U, T> class Q> struct DQ {};
template <class, int> struct CI;
template <class, char *> struct CC;
DQ<int, CI> ok6;
DQ<int, CC> bad3;
template <class V> struct Outer { DP<V, IP> m; };
Outer<int *> ok7;
template <class T, template <template <T> class> class R> struct DR {};
template <template <int> class> struct TI;
template <template <int *> class> struct TP;
DR<int, TI> ok8;
DR<int, TP> bad5;
int main() {}
