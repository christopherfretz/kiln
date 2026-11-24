// Per-specialization entries (spec/ast.md §18.9.3, §18.9.4): a dependent
// unqualified call whose lookup finds exactly one function
// ([basic.lookup.argdep]/4) gets a row naming it only when the call's
// arguments convert to its parameters, that is, when the candidate is
// viable ([over.match.viable]/4). `n::swap(S &, S &)` is the one function
// argument-dependent lookup finds for `n::U` too, where it is not viable:
// the unsatisfied `Sw<n::U>` ([temp.constr.atomic]/3) and the failed
// `twice<n::U>` deduction ([temp.deduct.general]/8) record nothing, as
// a call with two candidates and none viable does. `Sw<n::S>`,
// `twice<n::S>` and `body<n::S>` record the candidate, with the
// associated set of their argument type.
//@ written-refs: inst-adl-single.refs
namespace n {
struct S {};
struct U {};
void swap(S &, S &) {}
} // namespace n

template <class T> concept Sw = requires(T &a) { swap(a, a); };

template <class T> auto twice(T &a) -> decltype(swap(a, a), 1) { return 1; }
int twice(...) { return 0; }

template <class T> void body(T &a) { swap(a, a); }

static_assert(Sw<n::S>);
static_assert(!Sw<n::U>);

int use(n::S &s, n::U &u) {
  body(s);
  return twice(s) + twice(u);
}
