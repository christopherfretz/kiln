// [temp.deduct.general]/8 (WP-3697, WP-3283): when a function template
// specialization fails silently inside a SFINAE trap and is later
// re-deduced after a new declaration (completing `S`), if it fails again
// its own new first error is reported rather than the stale earlier one.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:18 error "no matching function for call to 'g'"
//@ diag: 10:25 note "candidate template ignored: substitution failure [with T = S]: no member named 'size' in 'S'"
struct S;
template <class T> auto g(T* t) -> decltype(t->size());
long g(...);
long a(S* p) { return g(p); }
struct S { int other(); };
void use(S* p) { g<S>(p); }
