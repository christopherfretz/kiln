// [meta.reflection.access.context] (P2996R13): via(cls) needs a null reflection or a complete
// class type (a type alias of one is fine); current_function() needs a function scope and
// current_class() a class scope or a member function's. Each failure throws std::meta::exception.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:15 note "in call to 'access_context::current().via(^^int)'"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:15 note "in call to 'access_context::current().via(^^Inc)'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:15 note "in call to 'access_context::current().via(^^N)'"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:15 note "current scope does not represent a function"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:15 note "current scope does not represent a class nor a member function"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:35 note "current scope does not represent a class nor a member function"
//@ diag: 37:15 note "in call to 'free_fn()'"
//@ meta-header: skip access_context::via is GCC's library code, so the notes differ
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;
struct Inc;
namespace N {}
static_assert(access_context::current().via(^^int).scope() == ^^::);
static_assert(access_context::current().via(^^Inc).scope() == ^^::);
static_assert(access_context::current().via(^^N).scope() == ^^::);
static_assert(current_function() == info());
static_assert(current_class() == info());
consteval info free_fn() { return current_class(); }
static_assert(free_fn() == info());
