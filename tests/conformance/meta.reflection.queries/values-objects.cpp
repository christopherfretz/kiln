// [meta.reflection.queries] P2996R13: object_of, constant_of, is_value
// and is_object, with the IS examples. object_of of a variable is the
// object it designates (an object reflection: equal for every way of
// reaching the object, not a variable, of the variable's type, static
// storage duration, no linkage, no parent); object_of of an object is
// itself. constant_of of a variable usable in constant expressions or of
// an enumerator is a value reflection of the constant's value and type;
// constant_of of an object reflection is the object's value (other
// types: constant-of-values.cpp). Answers checked against GCC 16.1
// (-freflection).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

namespace ex_object_of {
// [meta.reflection.queries], object_of's example.
int x;
int& y = x;

static_assert(^^x != ^^y);
static_assert(object_of(^^x) == object_of(^^y));
static_assert(object_of(^^x) == object_of(^^x));
}

namespace ex_constant_of {
// [meta.reflection.queries], constant_of's example.
constexpr int x = 0;
constexpr int y = 0;

static_assert(^^x != ^^y);
static_assert(constant_of(^^x) == constant_of(^^y));
static_assert(constant_of(^^x) == reflect_constant(0));

struct S { int m; };
constexpr S s {42};
static_assert(is_object(constant_of(^^s)) && is_object(reflect_object(s)));
static_assert(constant_of(^^s) != reflect_object(s));
static_assert(constant_of(^^s) == constant_of(reflect_object(s)));
}

constexpr int ci = 4;
const long cl = 7;
enum E2 { a2 = 1, b2 = a2 + 1 };
enum class EC : char { c = 'c' };
constexpr bool cb = true;
int gv;

static_assert(type_of(constant_of(^^ci)) == (^^int) && type_of(constant_of(^^cl)) == ^^long);
static_assert(type_of(constant_of(^^a2)) == (^^E2) && type_of(constant_of(^^EC::c)) == ^^EC);
static_assert(constant_of(^^b2) != constant_of(^^a2) && constant_of(^^cb) == constant_of(^^cb));
static_assert(constant_of(^^ci) != constant_of(^^cl));  // same value, other type
static_assert(is_value(constant_of(^^ci)) && !is_object(constant_of(^^ci)));
static_assert(!is_variable(constant_of(^^ci)) && !has_parent(constant_of(^^ci)));
static_assert(!is_const(constant_of(^^ci)) && dealias(constant_of(^^ci)) == constant_of(^^ci));

static_assert(is_object(object_of(^^ci)) && !is_value(object_of(^^ci)));
static_assert(type_of(object_of(^^ci)) == (^^const int) && is_const(object_of(^^ci)));
static_assert(object_of(object_of(^^ci)) == object_of(^^ci));
static_assert(object_of(^^gv) != object_of(^^ci) && object_of(^^gv) != ^^gv);
static_assert(!is_variable(object_of(^^gv)) && !has_parent(object_of(^^gv)));
static_assert(has_static_storage_duration(object_of(^^gv)) && !has_linkage(object_of(^^gv)));
static_assert(!is_const(object_of(^^gv)));
static_assert(constant_of(object_of(^^ci)) == constant_of(^^ci));
