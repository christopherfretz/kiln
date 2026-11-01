// [temp.deduct.call]/1: a parameter whose only template parameters are
// packs expanded in its template argument list (`Tup<int, Us...>`) still
// takes part in deduction (CWG1391 skips only a P with no template
// parameter in a deduced context), so a mismatched argument fails
// deduction rather than the conversion. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:12 error "no matching function for call to 'f'"
//@ diag: 15:29 note "candidate template ignored: could not match 'Tup<int, Us...>' against 'std::nullptr_t'"
//@ diag: 18:24 error "no matching function for call to 'g'"
//@ diag: 16:29 note "candidate template ignored: could not match 'Tup<Us...> *' against 'std::nullptr_t'"
//@ diag: 18:36 error "no matching function for call to 'h'"
//@ diag: 17:29 note "candidate template ignored: could not match 'Tup<Us *...>' against 'int'"
template <class... T> struct Tup {};
template <class... Us> void f(Tup<int, Us...>);
template <class... Us> void g(Tup<Us...> *);
template <class... Us> void h(const Tup<Us *...> &);
void t() { f(nullptr); g(nullptr); h(0); }
