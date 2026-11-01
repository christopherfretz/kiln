// [temp.constr.decl]/3: the associated constraints are the type-constraints
// of the template-head, then its requires-clause, then the constraints of
// invented parameters, then the trailing requires-clause; satisfaction
// stops at the first unsatisfied conjunct ([temp.constr.op]/3), so a
// requires-clause whose substitution would hard-error is not checked when a
// type-constraint fails (libc++ pair's pair-like constructor). Matches
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
