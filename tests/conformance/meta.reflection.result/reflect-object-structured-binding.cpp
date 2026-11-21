// [meta.reflection.result] reflect_object (P2996R13) of a structured
// binding. The name of a binding is an lvalue that refers to an element
// or member of the decomposed variable e, or to the object a tuple-like
// binding's holding variable r_i is bound to ([dcl.struct.bind]/5-7);
// reflect_object(b) reflects that object when it is suitable as a
// constant template argument of type T& ([temp.arg.nontype]/2-3): a
// subobject of a variable of static storage duration, which e is at
// namespace scope (constexpr or not: no value is read), and r_i is a
// reference constant-initialized with one, so usable in constant
// expressions ([expr.const]). The template arguments X<a> are valid
// likewise (temp.arg.nontype/structured-binding-reference.cpp; clang 22
// accepts them). GCC 16.1 rejects each reflect_object here
// ("reflect_object failed", or "the value of '<structured bindings>' is
// not usable in a constant expression"); see docs/status/deviations.md
// (WP-1783).
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>
#include <tuple>
using namespace std::meta;

struct P { int x; int y; };
P gp{1, 2};
auto [a, b] = gp;
constexpr P cp{3, 4};
constexpr auto [ca, cb] = cp;
int arr[2] = {5, 6};
auto [e0, e1] = arr;
constexpr int carr[2] = {7, 8};
constexpr auto [c0, c1] = carr;
std::tuple<int, long> gt{9, 10};
auto [t0, t1] = gt;
constexpr std::tuple<int, long> ct{11, 12};
constexpr auto [k0, k1] = ct;

constexpr info ra = reflect_object(a);
constexpr info rcb = reflect_object(cb);
constexpr info re1 = reflect_object(e1);
constexpr info rc0 = reflect_object(c0);
constexpr info rt1 = reflect_object(t1);
constexpr info rk0 = reflect_object(k0);

static_assert(is_object(ra) && is_object(rcb) && is_object(re1));
static_assert(is_object(rc0) && is_object(rt1) && is_object(rk0));
static_assert(type_of(ra) == (^^int) && type_of(rcb) == (^^const int));
static_assert(type_of(rt1) == (^^long) && type_of(rk0) == (^^const int));
static_assert(&[:ra:] == &a && &[:rcb:] == &cb && &[:re1:] == &e1);
static_assert(&[:rc0:] == &c0 && &[:rt1:] == &t1 && &[:rk0:] == &k0);
// The value of a binding of a constexpr decomposition is a constant.
static_assert([:rcb:] == 4 && [:rc0:] == 7 && [:rk0:] == 11);
static_assert(constant_of(rcb) == reflect_constant(4));
// Equal designations are one reflection.
static_assert(reflect_object(cb) == reflect_object(cb) && ra != reflect_object(b));

int main() {}
