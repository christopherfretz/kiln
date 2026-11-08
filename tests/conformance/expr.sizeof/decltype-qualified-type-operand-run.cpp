// [expr.sizeof]/1, [dcl.ambig.res]/2, [dcl.type.decltype]: in
// `sizeof(decltype(e)::name)` the qualifier is resolved before the operand
// is classified, so a member type makes the operand a type-id whatever
// follows the `)` (`+`, `)`, `;`), and a static data member an expression.
// The same holds for alignof and for a parenthesized cast or expression.
// Bitcoin Core's disconnected_transactions test sums
// `sizeof(decltype(temp_map)::key_type) + sizeof(decltype(temp_map)::value_type)`
// (WP-811).
//@ expect: run
//@ stdout: 8 4 8 3 4 7 ok
extern "C" int printf(const char*, ...);
struct Map { using key_type = long long; using mapped_type = int; static const int v = 3; };
Map m;
template <class T> unsigned long dep(T t) {
  return sizeof(typename decltype(t)::key_type) + (decltype(t)::v);
}
int main() {
  unsigned long a = sizeof(decltype(m)::key_type);
  unsigned long b = sizeof(decltype(m)::mapped_type);
  unsigned long c = alignof(decltype(m)::key_type);
  int d = (decltype(m)::v);
  unsigned long e = sizeof(decltype(m)::v);
  int f = (decltype(m)::mapped_type)7;
  bool g = sizeof(decltype(m)::key_type) + sizeof(decltype(m)::mapped_type) == 12 && dep(m) == 11;
  printf("%lu %lu %lu %d %lu %d %s\n", a, b, c, d, e, f, g ? "ok" : "bad");
}
