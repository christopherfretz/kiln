// [dcl.constexpr]/1-3, [basic.start.main]/3: where consteval may appear,
// and a redeclaration must agree on it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:1 error "consteval can only be used in function declarations"
//@ diag: 20:3 error "destructor cannot be declared consteval"
//@ diag: 21:3 error "non-static data member cannot be constexpr; did you intend to make it static?"
//@ diag: 21:3 error "consteval can only be used in function declarations"
//@ diag: 22:13 error "cannot combine with previous 'consteval' declaration specifier"
//@ diag: 24:1 error "'main' is not allowed to be declared consteval"
//@ diag: 26:15 error "consteval declaration of 'h' follows constexpr declaration"
//@ diag: 25:15 note "previous declaration is here"
//@ diag: 28:5 error "non-constexpr declaration of 'k' follows consteval declaration"
//@ diag: 27:15 note "previous declaration is here"
//@ diag: 29:8 error "function parameter cannot be consteval"
consteval int sq(int x) { return x * x; }
consteval int v = 3;
struct S {
  consteval S(int) {}
  consteval ~S() {}
  consteval int f = 1;
  consteval constexpr int g();
};
consteval int main() { return 0; }
constexpr int h();
consteval int h() { return 1; }
consteval int k();
int k() { return 2; }
void p(consteval int);
