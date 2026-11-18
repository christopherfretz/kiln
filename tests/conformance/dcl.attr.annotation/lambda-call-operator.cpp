// [expr.prim.lambda.closure]/7, [dcl.attr.annotation]/1: an
// attribute-specifier-seq before a lambda-declarator appertains to the
// closure's function call operator (or operator template), a function
// whose host scope and target scope are the closure class, so an
// annotation there is the call operator's. One after the declarator's
// parameter list or noexcept-specifier appertains to its type and stays
// an error (dcl.attr.annotation/non-declaration.cpp). Found running GCC's
// C++ testsuite; GCC 16.1 agrees.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
#include <meta>
using namespace std::meta;

auto a = [] [[=1, =2]] () { return 0; };
constexpr info op = ^^decltype(a)::operator();
static_assert(annotations_of(op).size() == 2);
static_assert([:constant_of(annotations_of(op)[1]):] == 2);
static_assert(annotations_of_with_type(op, ^^int).size() == 2);

auto none = [] () { return 0; };
static_assert(annotations_of(^^decltype(none)::operator()).size() == 0);

// A generic lambda: each specialization of the operator template has the
// annotations.
auto g = [] [[=3]] (auto x) { return x; };
static_assert(annotations_of(^^decltype(g)::operator()<int>).size() == 1);
auto t = []<class T> [[=4]] (T x) { return x; };
static_assert([:constant_of(annotations_of(^^decltype(t)::operator()<long>)[0]):] == 4);

// In a template, substituted with the lambda.
template <class T> consteval unsigned long size_note() {
  auto l = [] [[=sizeof(T)]] () {};
  return [:constant_of(annotations_of(^^decltype(l)::operator())[0]):];
}
static_assert(size_note<char>() == 1);
static_assert(size_note<int[4]>() == 4 * sizeof(int));

void block_scope() {
  auto l = [] [[=5]] () {};
  static_assert(annotations_of(^^decltype(l)::operator()).size() == 1);
}
