// [temp.deduct.call]/4.2, [conv.fctptr]: deduction of P = R(*)(const C*,
// C**, B...) from a pointer to a noexcept function succeeds through the
// function pointer conversion, including the trailing pack. libstdc++
// bits/basic_string.h:4433 (__gnu_cxx::__stoa(&std::wcstol, ...)); <string>
// and 36 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
template<typename R, typename Ret = R, typename C, typename... B>
Ret stoa(R (*conv)(const C *, C **, B...), const C *s, B... b) {
  C *e;
  return Ret(conv(s, &e, b...));
}
long wcstol_(const wchar_t *__restrict, wchar_t **__restrict, int) noexcept;
double wcstod_(const wchar_t *__restrict, wchar_t **__restrict) noexcept;
long h(const wchar_t *, wchar_t **, int);
long g(const wchar_t *s) { return stoa(&wcstol_, s, 10); }
int g2(const wchar_t *s) { return stoa<long, int>(&wcstol_, s, 10); }
double g3(const wchar_t *s) { return stoa(&wcstod_, s); }
long g4(const wchar_t *s) { return stoa(&h, s, 10); }
