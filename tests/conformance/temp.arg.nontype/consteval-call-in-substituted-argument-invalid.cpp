// [temp.arg.nontype]/1, [expr.const]: the rejected twin of
// consteval-call-in-substituted-argument.cpp: a substituted template
// argument whose evaluation is not a constant expression (an
// out-of-range element) makes the explicit argument invalid. WP-1613.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:35 error "no matching function for call to 'fi'"
//@ diag: 14:9 note "in instantiation of function template specialization 'f<2>' requested here"
//@ diag: 12:22 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'K'"
#include <vector>
consteval std::vector<int> vec(int n) { return std::vector<int>(n, 7); }
template <int K> int fi() { return K; }
template <int N> int f() { return fi<vec(N)[N]>(); }
int x = f<2>();
