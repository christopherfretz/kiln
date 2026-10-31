// [temp.deduct.call]/3: const T&& is not a forwarding reference; an lvalue does not bind.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:12 error "no matching function for call to 'f'"
//@ diag: 6:25 note "candidate function [with T = int] not viable: expects an rvalue for 1st argument"
template <class T> void f(const T&&);
int i = 0;
void g() { f(i); }
