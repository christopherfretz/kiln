// [meta.reflection.traits]/5-6 P2996R13, [meta.trans.cv], [meta.trans.ref],
// [meta.trans.sign], [meta.trans.arr], [meta.trans.ptr], [meta.trans.other]:
// each transformation is the builtin trait engine's __X (add_const etc.:
// the engine's cv-qualification, which leaves references and functions
// alone) and returns a reflection of the resulting type, never of an alias
// (p5: "the type denoted by"; spec/reflection-traits.md §4). An alias
// argument is the type it denotes (p4).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ xfail(gcc): compares with clang's __make_unsigned/__remove_cv builtins, which GCC 16.1 lacks
#include "kiln_meta_traits.h"
using namespace std::meta;

enum class E8 : signed char { a };
enum class EU : unsigned long long { a };
using Ptr = int*;
using CInt = const int;

static_assert(remove_const(^^const volatile int) == ^^volatile int);
static_assert(remove_volatile(^^const volatile int) == ^^const int);
static_assert(remove_cv(^^const volatile int) == ^^int);
static_assert(remove_cv(^^const int&) == ^^const int&);
static_assert(add_const(^^int) == ^^const int);
static_assert(add_const(^^int&) == ^^int&);
static_assert(add_volatile(^^int) == ^^volatile int);
static_assert(add_cv(^^int) == ^^const volatile int);
static_assert(add_const(^^int()) == ^^int());

static_assert(remove_reference(^^int&&) == ^^int);
static_assert(remove_reference(^^int) == ^^int);
static_assert(add_lvalue_reference(^^int&&) == ^^int&);
static_assert(add_lvalue_reference(^^void) == ^^void);
static_assert(add_rvalue_reference(^^int&) == ^^int&);
static_assert(add_rvalue_reference(^^int) == ^^int&&);

static_assert(make_signed(^^unsigned) == ^^int);
static_assert(make_signed(^^const unsigned char) == ^^const signed char);
static_assert(make_unsigned(^^int) == ^^unsigned);
static_assert(make_unsigned(^^char) == ^^unsigned char);
static_assert(make_signed(^^E8) == ^^signed char);
static_assert(make_unsigned(^^E8) == ^^unsigned char);
static_assert(make_unsigned(^^EU) == ^^__make_unsigned(EU));

static_assert(remove_extent(^^int[2][3]) == ^^int[3]);
static_assert(remove_extent(^^int) == ^^int);
static_assert(remove_all_extents(^^int[2][3]) == ^^int);
static_assert(remove_all_extents(^^int[]) == ^^int);

static_assert(remove_pointer(^^int* const) == ^^int);
static_assert(remove_pointer(^^int) == ^^int);
static_assert(add_pointer(^^int&) == ^^int*);
static_assert(add_pointer(^^void) == ^^void*);
static_assert(add_pointer(^^int() const) == ^^int() const);

static_assert(remove_cvref(^^const int&) == ^^int);
static_assert(remove_cvref(^^int[2]) == ^^int[2]);
static_assert(decay(^^const int&) == ^^int);
static_assert(decay(^^int[2]) == ^^int*);
static_assert(decay(^^int(long)) == ^^int (*)(long));
static_assert(underlying_type(^^E8) == ^^signed char);
static_assert(underlying_type(^^EU) == ^^unsigned long long);

// Aliases are looked through and never produced.
static_assert(remove_pointer(^^Ptr) == ^^int);
static_assert(add_const(^^Ptr) == ^^int* const);
static_assert(remove_const(^^CInt) == ^^int);
static_assert(remove_cv(^^CInt) == ^^__remove_cv(CInt));
