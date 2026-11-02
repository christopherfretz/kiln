// [class.copy.ctor]/14, [dcl.attr.nouniqueaddr]/2, [intro.object]/9: an
// implicitly-defined copy constructor copies every non-static data member.
// clang merges a run of trivially copyable members into one memcpy, and an
// empty member (e.g. an empty [[no_unique_address]] predicate placed at
// offset 0 over a non-empty member, as in libc++ `filter_view`) copies none
// of its bytes; the run must still cover the non-empty member before it.
// Regression for libc++ range.filter/adaptor.pass.cpp. Matches clang 19.
//@ expect: run
struct R { int* p; };
struct Pred {};
struct Cache {
  long a = 1;
  Cache() = default;
  Cache(Cache const&) : a(0) {}
};
struct FV {
  [[no_unique_address]] R base;
  [[no_unique_address]] Pred pred;
  [[no_unique_address]] Cache c;
};
struct T1 { int x; Pred p; Cache c; };
struct T3 { [[no_unique_address]] Pred p; int x; Cache c; };
struct T4 { [[no_unique_address]] Pred p; [[no_unique_address]] Pred q; Cache c; };
int main() {
  int b[2] = {1, 2};
  FV v{{b}, {}, {}};
  FV w = v;
  if (w.base.p != b || w.c.a != 0) return 1;
  T1 t1{7, {}, {}};
  T1 u1 = t1;
  if (u1.x != 7) return 2;
  T3 t3{{}, 9, {}};
  T3 u3 = t3;
  if (u3.x != 9) return 3;
  T4 t4;
  T4 u4 = t4;
  if (u4.c.a != 0) return 4;
  return 0;
}
