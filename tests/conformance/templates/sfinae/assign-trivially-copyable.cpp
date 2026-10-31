// libc++'s __assign_trivially_copyable (constexpr_c_functions.h): an overload
// set selected by enable_if on __is_assignable / __is_constructible, called
// during constant evaluation through a char_traits-style constexpr memmove.
// Before __is_constructible, kiln found no viable overload.
//@ expect: compile-ok
//@ strict-diags
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> { using type = T; };
template <bool B> using en_t = typename en<B, int>::type;
template <class T, class U, en_t<__is_assignable(T &, const U &)> = 0>
constexpr T &assign(T &d, const U &s) { d = s; return d; }
template <class T, class U,
          en_t<!__is_assignable(T &, const U &) && __is_assignable(T &, U &&)> = 0>
constexpr T &assign(T &d, U &s) { d = static_cast<U &&>(s); return d; }
template <class T, class U,
          en_t<!__is_assignable(T &, const U &) && !__is_assignable(T &, U &&) &&
               __is_constructible(T, const U &)> = 0>
constexpr T &assign(T &d, const U &s) { d.~T(); new (&d) T(s); return d; }
void *operator new(decltype(sizeof 0), void *p) noexcept;
struct C { const int v; };
template <class T, class U> constexpr T *move(T *d, U *s, unsigned long n) {
  for (unsigned long i = 0; i != n; ++i) assign(d[i], s[i]);
  return d;
}
constexpr bool t() {
  char a[4] = {'a', 'b', 'c', 'd'};
  move(a, a + 1, 2);
  char16_t b[3] = {1, 2, 3};
  move(b, b + 1, 2);
  static_assert(!__is_assignable(C &, const C &) && __is_constructible(C, const C &));
  return a[0] == 'b' && a[1] == 'c' && a[2] == 'c' && b[0] == 2 && b[1] == 3;
}
static_assert(t());
int main() {}
