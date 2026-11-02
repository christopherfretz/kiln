// [temp.deduct.call]/4.3: libstdc++'s std::variant assigns and emplaces
// through a helper taking `_Variant_storage<_Triv, _Types...>&`, while
// each layer of the variant's base chain names its base through an alias
// template (`_Variant_storage_alias<_Types...>`, `_Copy_ctor_alias<...>`);
// deduction is against the canonical base class, not the alias spelling.
// Matches clang 19.
//@ expect: run
//@ std: c++23
//@ stdout: 0 1 2 1 x y 1 1.5
#include <variant>
#include <cstdio>

struct name {
  const char* s;
  name(const char* s) : s(s) {}
  name(const name& o) : s(o.s) {}
  name& operator=(const name& o) { s = o.s; return *this; }
  ~name() {}
};

int main() {
  std::variant<int, name> v;
  v = 1;
  std::printf("%zu %d ", v.index(), std::get<0>(v));
  v.emplace<0>(2);
  std::printf("%d ", std::get<int>(v));
  v = name("x");
  std::printf("%zu %s ", v.index(), std::get<1>(v).s);
  v.emplace<name>("y");
  std::printf("%s ", std::get<name>(v).s);
  std::variant<int, double> w(3.5);
  w = 4;
  w.emplace<1>(1.5);
  std::printf("%zu %g\n", w.index(), std::get<double>(w));
}
