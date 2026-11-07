// [temp.dep.expr]/3, [expr.type.conv]/2: `int(t + 0)` has type `int`
// whatever `t` is, so it is value-dependent but not type-dependent, as a
// `static_cast` is. A direct-initialization from it is checked at the
// definition with an explicit constructor a candidate
// ([over.match.ctor]/1), and a call naming it binds to the overloads
// visible at the definition ([temp.nondep]/1). LLVM's
// `RecordPrefix DummyPrefix(uint16_t(Record.getKind()))`
// (SimpleTypeSerializer.cpp). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 7 1 1 3
#include <cstdio>

typedef unsigned short u16;
struct P {
  P() = default;
  explicit P(u16 k) : k(k) {}
  u16 k = 0;
};
int pick(long) { return 1; }
template <class T> int ser(T &r) {
  P d(u16(r.kind()));
  P e((u16(r.kind() + 0)));
  return d.k == e.k ? d.k : -1;
}
template <class T> int bind(T t) { return pick(int(t + 0)); }
template <class T> int bind_list(T t) { return pick(int{t}); }
template <class T> constexpr int size_of(T t) { return sizeof(int(t)) - sizeof(int) + 3; }
int pick(int) { return 2; }
struct R {
  int kind() { return 7; }
};
int main() {
  R r;
  std::printf("%d %d %d %d\n", ser(r), bind(0), bind_list(0), size_of(0));
}
