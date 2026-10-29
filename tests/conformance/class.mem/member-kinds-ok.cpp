// [class.mem.general], [class.bit], [dcl.attr.nouniqueaddr], [dcl.stc]/9: member kinds that need no sema stub
//@ expect: compile-ok
enum E { e0, e1 };
struct Empty {};
struct S final {
  enum G : int { g0 } g : 3;
  E e : 2;
  [[no_unique_address]] Empty empty;
  mutable int m;
  int : 0;
  unsigned bits : 3, more : 4;
  struct In { int v; } in;
  ;
};
struct final { int y; };
int use(S &s) { return s.m + s.in.v + s.bits; }
