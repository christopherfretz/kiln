// [dcl.fct]/6, [expr.reflect]/6 P2996R13: a cv- or ref-qualified function
// type ("abominable" type) may be the operand of `^^`, named through an
// alias or spelled as a type-id: the reflect-expression declares nothing,
// so the [dcl.fct]/6 restriction on declarations does not apply (GCC 16.1
// agrees). Declaring a function of such a type is still an error
// (reflect-qualified-function-type-neg.cpp).
//@ expect: compile-ok
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
using FnC = int() const;
using FnR = void() &&;
using FnVL = void() volatile &;
constexpr auto a = ^^FnC;
constexpr auto b = ^^FnR;
constexpr auto c = ^^int() const;
constexpr auto d = ^^FnVL;
static_assert(a != ^^int());
static_assert(b != ^^void());
static_assert(d != b);
template <decltype(^^int) R> struct Tag {};
Tag<^^int() const &> t;
int main() {}
