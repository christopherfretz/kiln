// Per-specialization entries (spec/ast.md §18.9.3, §18.9.4): a dependent
// unqualified call whose lookup finds exactly one function, not viable
// for the specialization's arguments ([over.match.viable]/4), is an
// error in an instantiation ([temp.res.general]/6), not a substitution
// failure: `body<n::U>` records `swap` with no entity (`(unresolved)`)
// and the associated set of `n::U`, as a call with two candidates and
// none viable does; `body<n::S>` records the function.
//@ expect: compile-fail
//@ written-refs: inst-adl-single-error.refs
//@ diag: 16:38 error "no matching function for call to 'swap'"
namespace n {
struct S {};
struct U {};
void swap(S &, S &) {}
} // namespace n
template <class T> void body(T &a) { swap(a, a); }

void use(n::S &s, n::U &u) {
  body(s);
  body(u);
}
