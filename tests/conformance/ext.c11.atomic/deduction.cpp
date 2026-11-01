// Clang extension: clang 19 deduces no template argument through _Atomic(T)
// (couldn't infer); explicit template arguments substitute into it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:10 error "no matching function for call to 'f'"
//@ diag: 11:22 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 15:10 error "no matching function for call to 'f'"
//@ diag: 11:22 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 16:10 error "no matching function for call to 'g'"
//@ diag: 12:23 note "candidate template ignored: couldn't infer template argument 'T'"
template <class T> T f(_Atomic(T) *p);
template <class T> T *g(_Atomic(T *) *p);
_Atomic(int) a; _Atomic(char *) c;
auto x = f(&a);
auto y = f(&c);
auto z = g(&c);
auto w = f<int>(&a); auto v = g<char>(&c);
