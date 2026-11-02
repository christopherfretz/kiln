// [temp.arg.template]/3-4 (P0522, clang 19's default relaxed matching): a template template
// parameter's non-type parameter whose type names an earlier parameter of the enclosing template
// is matched with the earlier arguments substituted; the argument's parameters are not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:9 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 21:16 note "template non-type parameter has a different type 'int *' in template argument"
//@ diag: 20:31 note "previous non-type template parameter with type 'int' is here"
//@ diag: 31:11 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 22:14 note "template non-type parameter has a different type 'int' in template argument"
//@ diag: 20:31 note "previous non-type template parameter with type 'int *' is here"
//@ diag: 37:9 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 35:24 note "template non-type parameter has a different type 'char *' in template argument"
//@ diag: 33:40 note "previous non-type template parameter with type 'int' is here"
//@ diag: 44:9 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 42:26 note "template non-type parameter has a different type 'int *' in template argument"
//@ diag: 40:41 note "previous non-type template parameter with type 'int' is here"
//@ requires: clang<22
// clang 22 emits 19:16 note "template non-type parameter has a different type 'int *' in template a...; WP-355 residual
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
