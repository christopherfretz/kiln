// [class.base.init]/13: -Wuninitialized through a glvalue conditional operator
// in a mem-initializer (the lvalue-to-rvalue conversion sits on top of the
// `?:`, so the check walks the condition and both arms as value uses). The
// sema check used to read the `?:` node with the binary-operand accessor and
// aborted. Matches clang 19.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wuninitialized
//@ diag: 14:37 warning "field 'd' is uninitialized when used here"
//@ diag: 15:23 warning "field 'e' is uninitialized when used here"
struct P { int x; };
struct S {
  int a, b, c, d, e, h; P p; P q; P& r;
  S(bool k, P& o) : a(1), b(k ? a : d), d(2),        // glvalue ?: read: both arms are uses
                    c(e ? a : b), e(3),               // the condition is a use
                    h(k ? 1 : 2),                     // prvalue ?: with no field use
                    p(k ? o : P{1}), q((k ? o : p)), r(k ? o : p) {}
};
int main() { P o{1}; S s(true, o); return s.b + s.c + s.h + s.q.x + s.r.x; }
