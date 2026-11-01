// [temp.constr.decl]/3: the associated constraints are the template
// parameters' type-constraints, then the requires-clause, then the
// trailing requires-clause; satisfaction stops at the first unsatisfied
// conjunct ([temp.constr.op]/3), so a requires-clause whose substitution
// would hard-error is not checked when a type-constraint fails (libc++
// pair's pair-like constructor). An abbreviated template's invented
// parameters count as template parameters, before the requires-clause
// (clang 19; the standard, /3.3, and GCC put them after it). Matches
// clang 19.
template <class T> struct ts {
  static constexpr int value = 1;
};
template <> struct ts<int[2]> {
  static constexpr int value = 2;
};
template <class T> concept two = ts<T>::value == 2;
template <int I, class T> struct elem {
  static_assert(I < 1, "out of range");
  using type = T;
};
template <int I, class T> typename elem<I, T>::type get(T);
template <class T> T&& declval();
struct P {
  template <two U>
    requires(sizeof(decltype(get<1>(declval<U>()))) > 0)
  P(U&&) {}
  P(int, int) {}
};
template <class T> concept C = requires(T t) { P(t); };
static_assert(!C<double>);

template <class T> concept small = sizeof(T) <= 4;
template <class T> concept has_x = requires { T::x; };
void h(small auto t)
  requires has_x<decltype(t)>;
template <class T> concept callable_h = requires(T t) { h(t); };
static_assert(!callable_h<int>);

// An invented parameter's constraint is checked before the template-head's
// requires-clause (clang 19; the standard and GCC check it after).
template <class T> concept never = false;
template <class T> struct boom {
  static_assert(sizeof(T) == 0, "boom");
  static constexpr bool v = true;
};
template <class T>
  requires boom<T>::v
void k(never auto x, T) {}
template <class T> concept can_k = requires(int i, T t) { k(i, t); };
static_assert(!can_k<int>);
