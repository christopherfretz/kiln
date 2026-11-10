// [dcl.type.auto.deduct]/2, [temp.deduct]/7: a function template's
// placeholder return type whose type-constraint calls functions of its
// template parameters (`C<spec_of(common(R1, R2))> auto`) has them
// substituted with the deduced arguments; the substituted call is then
// converted to the concept's parameter before the type deduced from the
// return statement is checked ([temp.arg.nontype]/2). Substituting the
// constraint's arguments (WP-1235) first left the call unconverted
// ("substitution into constraint expression resulted in a non-constant
// expression").
//@ std: c++20
//@ expect: run
//@ stdout: sum 3 3
//@ stdout: one 4
extern "C" int printf(const char *, ...);

struct spec { int v; };
template <auto R, class Rep> struct quantity {
  static constexpr auto reference = R;
  Rep value;
};
template <class Q, auto S> concept of = Q::reference.v == S.v;
template <class T, class... Ts> consteval spec common(T t, Ts...) { return t; }
consteval spec spec_of(spec s) { return s; }

template <auto R1, class Rep1, auto R2, class Rep2>
  requires requires { common(R1, R2); }
constexpr of<spec_of(common(R1, R2))> auto sum(const quantity<R1, Rep1> &x,
                                               const quantity<R2, Rep2> &y) {
  constexpr auto ref = common(R1, R2);
  return quantity<ref, double>{x.value + y.value};
}

template <auto R> of<spec_of(R)> auto one(quantity<R, int> q) {
  return quantity<spec{4}, double>{double(q.value)};
}

int main() {
  const auto c = sum(quantity<spec{3}, double>{1}, quantity<spec{3}, int>{2});
  printf("sum %d %d\n", int(c.value), decltype(c)::reference.v);
  const auto d = one(quantity<spec{4}, int>{4});
  printf("one %d\n", int(d.value));
}
