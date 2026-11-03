// [meta.reflection.access.context] (P2996R13): access_context::current()
// designates the innermost function, class or namespace scope enclosing
// the point where it is evaluated (a default argument: the call), with no
// designating class; unprivileged() is the global namespace scope and
// unchecked() the null scope, both without a designating class; via(cls)
// keeps the scope and designates `cls` (a class type or null).
// current_function(), current_class() and current_namespace() name the
// scopes current() starts from.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

static_assert(access_context::current().scope() == ^^::);
static_assert(access_context::current().designating_class() == info());
static_assert(access_context::unprivileged().scope() == ^^::);
static_assert(access_context::unprivileged().designating_class() == info());
static_assert(access_context::unchecked().scope() == info());
static_assert(access_context::unchecked().designating_class() == info());
static_assert(current_namespace() == ^^::);

namespace N {
static_assert(access_context::current().scope() == ^^N);
static_assert(current_namespace() == ^^N);
consteval info here() { return access_context::current().scope(); }
static_assert(here() == ^^here);
consteval info fn() { return current_function(); }
static_assert(fn() == ^^fn);
consteval info ns() { return current_namespace(); }
static_assert(ns() == ^^N);
}  // namespace N

struct C {
  static consteval info scope() { return access_context::current().scope(); }
  static consteval info cls() { return current_class(); }
  static_assert(access_context::current().scope() == ^^C);
};
static_assert(C::scope() == ^^C::scope);
static_assert(C::cls() == ^^C);

// A default argument is evaluated at the call.
consteval info caller(access_context a = access_context::current()) { return a.scope(); }
static_assert(caller() == ^^::);
consteval info calls_caller() { return caller(); }
static_assert(calls_caller() == ^^calls_caller);

// via
struct B {};
static_assert(access_context::current().via(^^B).designating_class() == ^^B);
static_assert(access_context::current().via(^^B).scope() == ^^::);
static_assert(access_context::unchecked().via(^^B).scope() == info());
static_assert(access_context::current().via(^^B).via(info()).designating_class() == info());
// A union is a class type ([class.pre]/1); GCC 16.1 rejects it as a
// designating class.
union Un {};
#ifdef __KILN__
static_assert(access_context::current().via(^^Un).designating_class() == ^^Un);
#endif
using BA = B;
static_assert(access_context::current().via(^^BA).designating_class() != info());

int main() {}
