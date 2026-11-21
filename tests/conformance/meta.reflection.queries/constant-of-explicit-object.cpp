// [meta.reflection.queries] constant_of (P2996R13): with R the splice of
// r, constant_of of a function is reflect_function([: R :]), the function
// itself. An explicit-object member function ([dcl.fct]/6) is not an
// implicit object member function, so [expr.prim.id.general]/4 does not
// restrict its splice: [: R :] is an lvalue of its function type
// ([expr.prim.splice]/2) and &[: R :] an ordinary pointer to function
// ([expr.unary.op]/3). That holds for an explicit-object conversion
// function and operator, a by-value explicit object parameter and a
// specialization of an explicit-object member function template, as for
// a static member function (WP-1782; kiln used to fail them as "not a
// valid argument to a splice-expression"). GCC 16.1 agrees.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
using namespace std::meta;

struct S {
  int v;
  constexpr int f(this const S& s) { return s.v + 1; }
  constexpr int by_value(this S s, int k) { return s.v + k; }
  template <class T> constexpr int t(this T&& s) { return s.v * 2; }
  constexpr operator int(this const S& s) { return s.v * 10; }
  constexpr bool operator==(this const S& a, const S& b) { return a.v == b.v; }
  static constexpr int g() { return 3; }
};

static_assert(constant_of(^^S::f) == ^^S::f);
static_assert(constant_of(^^S::by_value) == ^^S::by_value);
static_assert(constant_of(^^S::operator int) == ^^S::operator int);
static_assert(constant_of(^^S::operator==) == ^^S::operator==);
static_assert(constant_of(^^S::g) == ^^S::g);
static_assert(constant_of(constant_of(^^S::f)) == ^^S::f);
static_assert(is_function(constant_of(^^S::f)));
static_assert(type_of(constant_of(^^S::f)) == ^^int(const S&));
static_assert(type_of(constant_of(^^S::by_value)) == ^^int(S, int));

constexpr info ts = substitute(^^S::t, {^^const S&});
static_assert(constant_of(ts) == ts);
static_assert(constant_of(^^S::t<S&>) == ^^S::t<S&>);
static_assert(type_of(constant_of(ts)) == ^^int(const S&));

// The constant is the function: extract gives a pointer to it, and the
// splice of the reflection is an lvalue of function type whose address is
// an ordinary pointer to function. (A call `[: r :](s)` stays ill-formed,
// as `S::f(s)` is: [over.call.func]/3 gives it a contrived object.)
static_assert(extract<int (*)(const S&)>(constant_of(^^S::f))(S{4}) == 5);
static_assert(extract<int (*)(S, int)>(constant_of(^^S::by_value))(S{4}, 3) == 7);
static_assert(extract<int (*)(const S&)>(constant_of(ts))(S{4}) == 8);
static_assert(extract<int (*)(const S&)>(constant_of(^^S::operator int))(S{4}) == 40);
static_assert(extract<bool (*)(const S&, const S&)>(constant_of(^^S::operator==))(S{1}, S{1}));
static_assert(&[:constant_of(^^S::f):] == &S::f);
constexpr int (*pf)(const S&) = &[:constant_of(^^S::f):];
static_assert(pf(S{9}) == 10);

int main() {}
