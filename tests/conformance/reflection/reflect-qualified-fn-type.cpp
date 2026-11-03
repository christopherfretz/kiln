// [dcl.fct]/6 P2996R13: a function type with a cv-qualifier-seq or
// ref-qualifier may be the type-id of a reflect-expression, named through
// a typedef-name or spelled out; it stays a distinct type.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

using FnC = int() const;
using FnR = void() &&;
template <class T> constexpr info canon = ^^T;
static_assert(canon<FnC> == ^^int() const);
static_assert(canon<FnR> == ^^void() &&);
static_assert(^^int() const != ^^int());
static_assert(^^FnC != ^^int() const);  // the alias is kept ([expr.reflect])
