// [temp.inst], [class.temporary]/4: a variable template whose type is
// dependent but whose initializer is not ("q().d" for a class prvalue
// q()) keeps the materialized temporary's full-expression in each
// specialization: the temporary lives until the specialization's
// initialization (here a copy into T) is done, so reading its member is
// a constant expression. The full-expression wrapper of the pattern's
// initializer is rebuilt around the instantiated initializer rather than
// nested inside its conversion (spec/sema.md "Immediate invocations";
// WP-549). Covers a constexpr and a consteval q. Matches clang 19 and 22.
//@ expect: run
//@ strict-diags
struct D { int a = 0; bool bad = false; };
struct Q { int x; D d{}; };
constexpr Q q0() { return Q{1, {2}}; }
consteval Q q1() { return Q{3, {4}}; }
template <class T> constexpr T v0 = q0().d;
template <class T> constexpr T v1 = q1().d;
template <class T> constexpr T x1 = q1().x;
static_assert(v0<D>.a == 2);
static_assert(v1<D>.a == 4);
static_assert(x1<int> == 3);
static_assert(x1<long> == 3);
int main() { return v0<D>.a + v1<D>.a + x1<int> - 9; }
