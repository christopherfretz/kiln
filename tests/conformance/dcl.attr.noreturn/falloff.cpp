// [dcl.attr.noreturn]/2: a noreturn function that can reach its end
// (clang -Winvalid-noreturn, ahead of -Wreturn-type); a throw or a
// handler that rethrows does not return.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:57 warning "function declared 'noreturn' should not return"
//@ diag: 11:25 warning "function declared 'noreturn' should not return"
//@ diag: 13:45 warning "function declared 'noreturn' should not return"
[[noreturn]] void h() { throw 1; }
[[noreturn]] void i() { try { throw 1; } catch (...) {} }
[[noreturn]] void j() { }
[[noreturn]] void k() { try { throw 1; } catch (...) { throw; } }
[[noreturn]] int l(int a) { if (a) throw 1; }
template <class T> [[noreturn]] void u() { }
template <class T> int v() { }
