// [dcl.attr.noreturn]/1: the attribute appertains to the function, so a
// definition without it still may not return; a call of a noreturn member
// function (virtual or not) ends control flow for -Wreturn-type.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:13 warning "function declared 'noreturn' should not return"
//@ diag: 16:34 warning "non-void function does not return a value in all control paths"
[[noreturn]] void die();
void die() { throw 1; }
[[noreturn]] void bad();
void bad() {}
struct S { [[noreturn]] void m(); [[noreturn]] virtual void v(); };
int f1(int x) { if (x) return 1; die(); }
int f2(S &s) { s.m(); }
int f3(S *s) { s->v(); }
int f4(int x) { if (x) return 1; }
