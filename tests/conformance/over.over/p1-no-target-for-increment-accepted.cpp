// [over.over]/1, [expr.ref]/7.3.2: the accepted side of
// p1-no-target-for-increment-conditional.cpp: a call, a static member
// function, the address of a member, an overload set with a target type
// and a conditional whose operands are functions already resolved.
//@ expect: compile-ok
//@ args: -std=c++20 -Wno-unused-value
struct S { void f(); static void sf(); };
void g(int);
void g(double);
void k(int);
void h(bool b, S s) {
  for (;;s.f()) break;
  for (;;s.sf) break;
  for (;;g(1)) break;
  for (;;&S::f) break;
  void (*fp)(int) = g;
  fp = b ? k : fp;
  fp = b ? &k : static_cast<void (*)(int)>(g);
  (void)fp;
  (void)(b ? s.sf : S::sf);
}
