// [temp.names]/4: `>=` is not split inside a template argument list.
// In `w<k>=3` the argument is the expression `k >= 3`, and the list then
// lacks its closing `>`.
//@ expect: compile-fail
//@ diag: 7:23 error "expected '>'"
template <int N> int w = 0;
void f(int k) { w<k>=3; }
