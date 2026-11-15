// [expr.typeid]/5, [basic.type.qualifier]/3 in a constant expression: the
// cv-qualifiers of an array's element type are top-level, so the
// `type_info` of `const int[2]` is that of `int[2]` (C++23 constexpr
// `type_info::operator==`), while a pointer's pointee keeps its own.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
#include <typeinfo>
struct S {};
static_assert(typeid(const int[2]) == typeid(int[2]));
static_assert(typeid(S volatile[3][4]) == typeid(S[3][4]));
static_assert(typeid(const volatile S[]) == typeid(S[]));
static_assert(typeid(const S *[2]) != typeid(S *[2]));
constexpr const S cs[2] = {};
static_assert(typeid(cs) == typeid(S[2]));
