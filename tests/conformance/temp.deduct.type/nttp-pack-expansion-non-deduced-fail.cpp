// [temp.deduct.type]/5.3, [temp.deduct.call]/1: a template parameter used only in a
// non-deduced non-type argument (`Spec<conv<T>()>`) is still not deduced, and a bare
// `N...` pack is a deduced context, so a literal does not match `Spec<N...>`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:9 error "no matching function for call to 'bad'"
//@ diag: 12:39 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 16:9 error "no matching function for call to 'ded'"
//@ diag: 13:25 note "candidate template ignored: could not match 'const Spec<N...>' against 'const char[2]'"
template <class T> consteval int conv() { return sizeof(T); }
template <int... C> struct Spec { consteval Spec(const char*) {} };
template <class T, class... Args> int bad(const Spec<conv<T>()>&, const Args&...) { return 0; }
template <int... N> int ded(const Spec<N...>&) { return sizeof...(N); }

int a = bad("x", 1);
int c = ded("y");
