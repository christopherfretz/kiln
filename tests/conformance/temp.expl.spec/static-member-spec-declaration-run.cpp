// [temp.expl.spec]/14: `template<> int cache<char>::count;` declares the
// explicit specialization without defining it; the later one with an
// initializer is its definition (no redefinition), as is
// `template<> X cache<char>::obj{};` after `template<> X cache<char>::obj;`
// (a class type with no default constructor is not default-initialized by
// the declaration). Uses before the definition see its value; the other
// specializations are still instantiated from the out-of-line definitions.
//@ expect: run
//@ stdout: 42 3 0 9 1 3
#include <cstdio>
struct X {
  int v;
  explicit X(int x) : v(x) {}
};
template <typename C> struct cache {
  static int count;
  static const C *zones[2];
  static X obj;
};
template <typename C> int cache<C>::count;
template <typename C> const C *cache<C>::zones[2];
template <typename C> X cache<C>::obj{9};
template <> int cache<char>::count;
template <> const char *cache<char>::zones[2];
template <> X cache<char>::obj;
int early() { return cache<char>::count; }
template <> int cache<char>::count = 42;
template <> int cache<char>::count;
template <> const char *cache<char>::zones[2] = {"a", "bcd"};
template <> X cache<char>::obj{1};
struct S {
  template <typename T> static int v;
};
template <> int S::v<char>;
template <> int S::v<char> = 3;
int main() {
  std::printf("%d %d %d %d %d %d\n", early(), (int)__builtin_strlen(cache<char>::zones[1]),
              cache<int>::count, cache<int>::obj.v, cache<char>::obj.v, S::v<char>);
}
