// [temp.expl.spec]/14: an explicit specialization of a static data member
// (or of a static data member template) is a definition only if it has an
// initializer; otherwise it is a declaration. `cache<char>::zones` and
// `cache<char>::count` (and `S::v<char>`) are declared, not defined: their
// uses name `external global` symbols, while the initialized
// `cache<int>::count` and `S::v<int>` are strong definitions. (libstdc++'s
// `__timepunct_cache<char>::_S_timezones` is declared so in every unit that
// includes `<locale>`.)
//@ expect: ir
//@ requires: clang>=22
template <typename C> struct cache {
  static const C *zones[14];
  static int count;
};
template <typename C> const C *cache<C>::zones[14];
template <typename C> int cache<C>::count;
template <> const char *cache<char>::zones[14];
template <> int cache<char>::count;
template <> int cache<int>::count = 5;
struct S {
  template <typename T> static int v;
};
template <> int S::v<char>;
template <> int S::v<int> = 7;
int f() {
  return cache<char>::count + (cache<char>::zones[0] != 0) + cache<int>::count + S::v<char> +
         S::v<int>;
}
