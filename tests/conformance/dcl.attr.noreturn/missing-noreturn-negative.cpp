// [dcl.attr.noreturn]/1: no -Wmissing-noreturn when the body may fall off
// or return (a handler that falls off, a loop that may end), for a function
// already noreturn, a non-void function, a virtual function, a lambda, and
// an instantiation (clang CheckFallThroughDiagnostics::MakeForFunction;
// spec/sema.md §7.5.3).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall -Wmissing-noreturn
[[noreturn]] void die();
void ext();
void b1() {}
void b2(int n) { if (n) die(); }
void b3() try { die(); } catch (...) {}
void b4(int n) { while (n) {} }
void b5() { for (;;) { if (ext(), true) return; } }
[[noreturn]] void b6() { die(); }
int b7() { die(); }
struct V {
  virtual void v() { die(); }
};
struct W : V {
  void v() override { die(); }
};
template <class T> void t1() { die(); }
void use1() { t1<int>(); }
template <class T> struct C {
  void m() { die(); }
};
template struct C<int>;
auto l = [] { die(); };
void use2() { l(); }
void b8() { auto k = [] { die(); }; k(); }
