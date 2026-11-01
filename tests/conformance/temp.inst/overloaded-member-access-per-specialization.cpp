// [temp.inst], [over.match.call]: a non-dependent overloaded member access
// (`k.cmp`) called with dependent arguments is resolved again in every
// specialization (clang's `TransformUnresolvedMemberExpr`). libc++'s
// `basic_string_view` comparisons hit this through `char_traits<C>::compare`.
//@ std: c++26
struct K {
  static constexpr int cmp(const char *, int n) { return n; }
  static constexpr int cmp(const wchar_t *, int n) { return -n; }
  constexpr int go(int x) const { return x; }
  constexpr int go(long x) const { return int(x) * 10; }
};

template <class C> constexpr int f(const C *p, int n) {
  K k;
  return k.cmp(p, n);
}
static_assert(f("a", 3) == 3);
static_assert(f(L"a", 3) == -3);

template <class T> constexpr int g(T t) { return K().go(t); }
static_assert(g(2) == 2);
static_assert(g(2L) == 20);
static_assert(g(3) == 3);
