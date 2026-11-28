// [namespace.udecl]/14, [temp.deduct.general]/8, [except.spec]/7: inheriting
// constructors from `std::pair<K, V>` inside an enclosing class definition
// must not turn the SFINAE check of `M::Data`'s unparsed default member
// initializers into a hard error (reduced from real code, regression since
// 1f2951a7b, WP-3685).
//@ expect: run
//@ std: c++17
#include <utility>
template <class K, class V> struct DP : std::pair<K, V> { using std::pair<K, V>::pair; };
template <class K, class V> struct Map { char s[sizeof(DP<K, V>)]; };
struct M {
  struct Data { bool H = false; unsigned ID = 0; };
  Map<const int *, Data> Info;
};
int main() {
  M m;
  M::Data d;
  return (d.H || d.ID != 0) ? 1 : 0;
}
