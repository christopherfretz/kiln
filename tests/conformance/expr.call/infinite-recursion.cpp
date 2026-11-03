// [expr.call]: -Winfinite-recursion (in -Wall) at the start of a function
// body every live path of whose CFG calls the function before reaching the
// exit (clang checkRecursiveFunction; spec/sema.md §7.5.3): returns and
// branches that all recurse, a switch, ?:, do-while, goto, a
// function-try-block (its handlers are unreachable without EH edges),
// constexpr/consteval functions, members called through this or
// non-virtually, an operator, an explicit object parameter, members of an
// explicitly instantiated class template, a local class.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall
//@ diag: 35:15 warning "all paths through this function will call itself"
//@ diag: 36:11 warning "all paths through this function will call itself"
//@ diag: 37:16 warning "all paths through this function will call itself"
//@ diag: 38:15 warning "all paths through this function will call itself"
//@ diag: 39:15 warning "all paths through this function will call itself"
//@ diag: 40:16 warning "all paths through this function will call itself"
//@ diag: 41:11 warning "all paths through this function will call itself"
//@ diag: 42:20 warning "all paths through this function will call itself"
//@ diag: 43:11 warning "all paths through this function will call itself"
//@ diag: 44:26 warning "all paths through this function will call itself"
//@ diag: 45:26 warning "all paths through this function will call itself"
//@ diag: 47:12 warning "all paths through this function will call itself"
//@ diag: 48:12 warning "all paths through this function will call itself"
//@ diag: 49:19 warning "all paths through this function will call itself"
//@ diag: 50:20 warning "all paths through this function will call itself"
//@ diag: 51:37 warning "all paths through this function will call itself"
//@ diag: 52:24 warning "all paths through this function will call itself"
//@ diag: 55:12 warning "all paths through this function will call itself"
//@ diag: 59:17 note "in instantiation of member function 'C<int>::q' requested here"
//@ diag: 58:35 warning "all paths through this function will call itself"
//@ diag: 59:17 note "in instantiation of member function 'C<int>::u' requested here"
//@ diag: 60:34 warning "all paths through this function will call itself"
[[noreturn]] void die();
int f1(int n) { return f1(n - 1); }
void f2() { f2(); }
void f3(int n) { if (n) f3(n); else f3(n + 1); }
int f4(int n) { switch (n) { case 0: return f4(1); default: return f4(2); } }
int f5(int n) { return n ? f5(1) : f5(2); }
void f6(int n) { do { f6(n); } while (n); }
void f7() { goto L; L: f7(); }
void f8() noexcept { f8(); }
void f9() try { f9(); } catch (...) { }
constexpr int f10(int n) { return f10(n); }
consteval int f11(int n) { return f11(n); }
struct S {
  void m() { m(); }
  void t() { this->t(); }
  static void s() { S::s(); }
  virtual void v() { v(); }
  bool operator==(const S &o) const { return *this == o; }
  void x(this S &self) { self.x(); }
};
template <class T> struct C {
  void q() { q(); }
  void u();
};
template <class T> void C<T>::u() { this->u(); }
template struct C<int>;
void f12() { struct L { void b() { b(); } }; }
