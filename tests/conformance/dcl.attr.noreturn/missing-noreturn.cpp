// [dcl.attr.noreturn]/1: -Wmissing-noreturn at the start of the body of a
// function returning void (a constructor, a destructor) whose CFG has no
// live edge into the exit that falls off or returns (clang
// CheckFallThroughForBody NeverFallThroughOrReturn; spec/sema.md §7.5.3):
// a noreturn call, an infinite loop, a throw that is not the body's only
// statement, a function-try-block whose handler rethrows, members, an
// explicit specialization (named with its arguments), a local class.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall -Wmissing-noreturn
//@ diag: 27:11 warning "function 'a1' could be declared with attribute 'noreturn'"
//@ diag: 28:11 warning "function 'a2' could be declared with attribute 'noreturn'"
//@ diag: 29:11 warning "function 'a3' could be declared with attribute 'noreturn'"
//@ diag: 30:11 warning "function 'a4' could be declared with attribute 'noreturn'"
//@ diag: 31:11 warning "function 'a5' could be declared with attribute 'noreturn'"
//@ diag: 32:11 warning "function 'a6' could be declared with attribute 'noreturn'"
//@ diag: 33:11 warning "function 'a7' could be declared with attribute 'noreturn'"
//@ diag: 34:16 warning "function 'a8' could be declared with attribute 'noreturn'"
//@ diag: 36:7 warning "function 'S' could be declared with attribute 'noreturn'"
//@ diag: 37:8 warning "function '~S' could be declared with attribute 'noreturn'"
//@ diag: 38:12 warning "function 'm' could be declared with attribute 'noreturn'"
//@ diag: 39:19 warning "function 's' could be declared with attribute 'noreturn'"
//@ diag: 43:28 warning "function 't<char>' could be declared with attribute 'noreturn'"
//@ diag: 44:33 warning "function 'b' could be declared with attribute 'noreturn'"
[[noreturn]] void die();
void ext();
void a1() { die(); }
void a2() { for (;;) ext(); }
void a3() { while (true) {} }
void a4() { int x = 0; (void)x; throw 1; }
void a5() { { throw 1; } }
void a6() { ((throw 1)); }
void a7() try { throw 1; } catch (...) { throw; }
void a8(int n) { if (n) die(); else throw n; }
struct S {
  S() { die(); }
  ~S() { die(); }
  void m() { die(); }
  static void s() { die(); }
};
void S2();
template <class T> void t();
template <> void t<char>() { die(); }
void a9() { struct L { void b() { die(); } }; }
