// [expr.const]/5 (WP-241): the "in call to" note of a failed constant
// evaluation prints each argument's value as clang's APValue::printPretty
// does: scalars as literals, a class or array in brace form with its bases
// first, a union by its active member, an enumerator by name, a string
// literal pointer as the literal, and an array cut off after 10 elements.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-division-by-zero
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:42 note "division by zero"
//@ diag: 67:15 note "in call to 'f(3, 97)'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:32 note "division by zero"
//@ diag: 68:15 note "in call to 'g({3, 98})'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:56 note "division by zero"
//@ diag: 69:15 note "in call to 'm(nullptr, nullptr)'"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:31 note "division by zero"
//@ diag: 70:15 note "in call to 'f1({{1}, 2, {97, 98}})'"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:31 note "division by zero"
//@ diag: 71:15 note "in call to 'f2({})'"
//@ diag: 72:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:31 note "division by zero"
//@ diag: 72:15 note "in call to 'f3({.i = 3})'"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:37 note "division by zero"
//@ diag: 73:15 note "in call to 'f4(1, 2)'"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:33 note "division by zero"
//@ diag: 74:15 note "in call to 'f5({{1, 2, 3}})'"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:67 note "division by zero"
//@ diag: 75:15 note "in call to 'f7(true, 5, -3)'"
//@ diag: 76:15 error "static assertion expression is not an integral constant expression"
//@ diag: 63:41 note "division by zero"
//@ diag: 76:15 note "in call to 'f8(&\"hi\"[0])'"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:36 note "division by zero"
//@ diag: 77:15 note "in call to 'f9({{4}, 5, {99, 100}}, 1)'"
//@ diag: 78:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:33 note "division by zero"
//@ diag: 78:15 note "in call to 'fa({{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, ...}})'"
struct S { int a; char b; };
struct B { int b; };
struct D : B { int d; char c[2]; };
struct E {};
union U { int i; float f; };
enum Col { Red, Green };
enum class Sc : int { A = 1, B };
struct Arr { int a[3]; };
struct A20 { int a[20]; };
constexpr int f(int x, char c) { return 1/(x-3); }
constexpr int g(S s) { return 1/(s.a-3); }
constexpr int m(decltype(nullptr) p, int* q) { return 1/0; }
constexpr int f1(D) { return 1/0; }
constexpr int f2(E) { return 1/0; }
constexpr int f3(U) { return 1/0; }
constexpr int f4(Col, Sc) { return 1/0; }
constexpr int f5(Arr) { return 1/0; }
constexpr int f7(bool, unsigned long long, signed char) { return 1/0; }
constexpr int f8(const char*) { return 1/0; }
constexpr int f9(D, int) { return 1/0; }
constexpr int fa(A20) { return 1/0; }
constexpr A20 mk() { A20 x{}; for (int i = 0; i < 20; ++i) x.a[i] = i; return x; }
static_assert(f(3, 'a'));
static_assert(g(S{3, 'b'}));
static_assert(m(nullptr, nullptr));
static_assert(f1(D{{1}, 2, {'a', 'b'}}));
static_assert(f2(E{}));
static_assert(f3(U{3}));
static_assert(f4(Green, Sc::B));
static_assert(f5(Arr{{1, 2, 3}}));
static_assert(f7(true, 5, -3));
static_assert(f8("hi"));
static_assert(f9({{4}, 5, {'c', 'd'}}, 1));
static_assert(fa(mk()));
