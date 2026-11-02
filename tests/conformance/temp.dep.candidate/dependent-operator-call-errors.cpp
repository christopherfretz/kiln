// [temp.dep.general]/2, [temp.dep.candidate], [basic.lookup.argdep]/1: an
// unqualified call `operator@(args)` whose ordinary lookup finds nothing is
// resolved by argument-dependent lookup alone, at the call or, when an
// argument is type-dependent, at instantiation (only ADL then: a declaration
// visible after the template's definition is not a candidate). Finding
// nothing is `use of undeclared 'operator@'` (clang's DiagnoseEmptyLookup
// wording for an operator- or conversion-function-id). Verified against
// clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:19 error "use of undeclared 'operator<=>'"
//@ diag: 35:16 error "use of undeclared 'operator+'"
//@ diag: 37:21 error "use of undeclared 'operator-'"
//@ diag: 38:10 note "in instantiation of function template specialization 'h<B>' requested here"
//@ diag: 40:21 error "use of undeclared 'operator-'"
//@ diag: 41:10 note "in instantiation of function template specialization 'k<int>' requested here"
//@ diag: 42:11 error "use of undeclared 'operator%'"
//@ diag: 43:18 error "use of undeclared 'operator int'"
//@ diag: 44:20 error "use of undeclared 'operator/'"
namespace N {
struct A { int v; };
constexpr int operator+(A x, A y) { return 100 + x.v + y.v; }
constexpr int operator*(A, long) { return 1; }
struct H { friend constexpr bool operator==(H, H) { return true; } };
}
template<typename T> constexpr int add(T a, T b) { return operator+(a, b); }
template<typename T> constexpr int mul(T a) { return operator*(a, 0); }
constexpr int operator*(N::A, int) { return 2; }
static_assert(add(N::A{1}, N::A{2}) == 103);
static_assert(mul(N::A{}) == 1);
static_assert(operator==(N::H{}, N::H{}));

struct B {};
int g1() { return operator<=>(1, 2); }
void g2(B b) { operator+(b, b); }
template<typename T>
int h(T t) { return operator-(t, t); }
int i1 = h(B{});
template<typename T>
int k(T t) { return operator-(t); }
int i2 = k(1);
auto p = &operator%;
int q() { return operator int(3); }
struct C { int x = operator/(1, 2); };
