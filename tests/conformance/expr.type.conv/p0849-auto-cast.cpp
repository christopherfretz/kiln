// [expr.type.conv]/1-2 (C++23, P0849R8): `auto(x)` and `auto{x}` deduce
// the type as for `auto v(x);` (the decayed type of `x`) and make a prvalue
// copy (a class copy runs its copy constructor), in a template too: a
// type-dependent operand is deduced at instantiation.
//@ expect: run
//@ std: c++23
//@ strict-diags
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

struct S {
  int v;
  constexpr S(int x) : v(x) {}
  constexpr S(const S& o) : v(o.v + 1) {}
};

template <class T> constexpr auto paren(T& t) { return auto(t); }
template <class T> constexpr auto brace(const T& t) { return auto{t}; }
template <class T> using Copy = decltype(auto(T(1)));

int f(int x) { return x * 2; }

constexpr int k() {
  S s{1};
  return paren(s).v + brace(s).v + auto(s).v + auto{s}.v;
}
static_assert(k() == 8);

int main() {
  int a[3] = {4, 5, 6};
  const int c = 7;
  static_assert(same<decltype(auto(a)), int*>);
  static_assert(same<decltype(auto{c}), int>);
  static_assert(same<decltype(auto(f)), int (*)(int)>);
  static_assert(same<decltype(paren(a)), int*>);
  static_assert(same<decltype(brace("ab")), const char*>);
  static_assert(same<Copy<S>, S>);
  static_assert(same<Copy<const long>, long>);
  // The copy is a distinct object: changing it leaves `a` alone.
  int* p = auto(a);
  ++*p;
  if (a[0] != 5 || *auto{p} != 5) return 1;
  if (auto(f)(3) != 6) return 2;
  S s{10};
  if (auto(s).v != 11 || paren(s).v != 11 || brace(s).v != 11) return 3;
  return 0;
}
