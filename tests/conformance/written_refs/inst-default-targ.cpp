// Per-specialization entries (spec/ast.md §18.9.2, §18.9.3): a function
// template's default template argument is substituted by the deduction
// that uses it ([temp.deduct.general]/5), before the specialization it
// is part of exists; its names get rows for that specialization once
// the deduction forms it (`call<n::S, 4>`). A default that depends on
// an earlier deduced parameter (`sized`), one naming a function found by
// argument-dependent lookup (`probed`, `how=adl`) and a member function
// template's (`Host::make`) do the same. An explicit argument uses no
// default: `call<n::U, 1>` adds no line. Rows belong to the
// specializations that are formed: of the two `pick` candidates for
// `n::S`, the second's default fails (`T::missing`), a substitution
// failure ([temp.deduct.general]/8) that records nothing; for `n::W`
// both deductions succeed and form `pick<n::W, 4>` and
// `pick<n::W, 4, 4>` ([temp.over]/1), and the second is chosen for its
// exact match of `0` ([over.match.best.general]/2.1): the first keeps
// its rows though it is not called.
//@ written-refs: inst-default-targ.refs
namespace n {
struct S {
  static int s() { return 1; }
};
struct U {
  static long s() { return 2; }
};
struct W {
  static int s() { return 3; }
  static int missing() { return 4; }
};
int probe(S) { return 0; }
} // namespace n

template <class T, int = sizeof(T::s())> int call() { return 0; }
template <class T, int N = sizeof(T::s())> int sized(T) { return N; }
template <class T, class R = decltype(probe(T{}))> R probed(T) { return 0; }

struct Host {
  template <class T, int = sizeof(T::s())> static int make() { return 0; }
};

template <class T, int = sizeof(T::s())> int pick(T, long) { return 1; }
template <class T, int = sizeof(T::s()), int = sizeof(T::missing())> int pick(T, int) { return 2; }

int use() {
  return call<n::S>() + call<n::U, 1>() + sized(n::U{}) + probed(n::S{}) + Host::make<n::S>() +
         pick(n::S{}, 0) + pick(n::W{}, 0);
}
