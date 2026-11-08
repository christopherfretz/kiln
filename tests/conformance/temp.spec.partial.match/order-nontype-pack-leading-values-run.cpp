// [temp.spec.partial.order]/1, [temp.deduct.partial]: a partial
// specialization that fixes more leading values of a non-type pack is more
// specialized: `P<'0', D...>` deduces `D` = `<'x', D'...>` from the
// synthesized arguments of `P<'0', 'x', D'...>`, and the reverse fails. This
// is libstdc++'s `_Parse_int` dispatch, reached by `0x10s` from <chrono>
// (WP-811).
//@ expect: run
//@ stdout: 16 8 10 2 3 5 7
extern "C" int printf(const char*, ...);
template<char... D> struct P { static const int v = 10; };
template<char... D> struct P<'0', 'x', D...> { static const int v = 16; };
template<char... D> struct P<'0', D...> { static const int v = 8; };
template<int... I> struct Q { static const int v = 1; };
template<int... I> struct Q<0, I...> { static const int v = 2; };
template<int... I> struct Q<0, 1, I...> { static const int v = 3; };
template<int A, int... I> struct R { static const int v = 4; };
template<int... I> struct R<0, I...> { static const int v = 5; };
template<int... I> struct R<0, 1, I...> { static const int v = 7; };
template<char... C> struct Via { static const int v = P<C...>::v; };
int main() {
  printf("%d %d %d %d %d %d %d\n", Via<'0', 'x', '4', '9'>::v, P<'0', '1'>::v,
         P<'1'>::v, Q<0, 2>::v, Q<0, 1, 2>::v, R<0>::v, R<0, 1>::v);
}
