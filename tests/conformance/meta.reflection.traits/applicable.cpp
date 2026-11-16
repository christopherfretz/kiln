// [meta.reflection.traits], [meta.syn] (P1317): is_applicable_type,
// is_nothrow_applicable_type and apply_result answer as
// is_invocable / is_nothrow_invocable / invoke_result of the function
// type and get<I>(declval<Tuple>())... when Tuple is tuple-like
// ([tuple.like]: array, complex, pair, tuple, ranges::subrange); for any
// other type is_applicable_type is false and apply_result has no type.
// The cv and value category of Tuple reach the elements.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include <array>
#include <ranges>
#include <tuple>
#include <utility>
#include "kiln_meta_traits.h"
using namespace std::meta;

int add(int, long);
long take_ref(int &);
void nt(int, int) noexcept;
struct Fn { char operator()(const int &, const int &) const; };
struct NotTuple { int a, b; };
int v;
int span2(int *, int *);

static_assert(is_applicable_type(^^decltype(add), ^^std::tuple<int, long>));
static_assert(is_applicable_type(^^decltype(add), ^^std::pair<short, int>));
static_assert(is_applicable_type(^^Fn, ^^std::array<int, 2>));
static_assert(!is_applicable_type(^^Fn, ^^std::array<int, 3>));
static_assert(!is_applicable_type(^^decltype(add), ^^NotTuple));
static_assert(!is_applicable_type(^^decltype(add), ^^int));
// get<0> of an rvalue tuple is an xvalue: no int& binds to it.
static_assert(!is_applicable_type(^^decltype(take_ref), ^^std::tuple<int>));
static_assert(is_applicable_type(^^decltype(take_ref), ^^std::tuple<int> &));
static_assert(!is_applicable_type(^^decltype(take_ref), ^^const std::tuple<int> &));
static_assert(is_applicable_type(^^decltype(take_ref), ^^std::tuple<int &>));
static_assert(is_nothrow_applicable_type(^^decltype(nt), ^^std::tuple<int, int>));
static_assert(!is_nothrow_applicable_type(^^decltype(add), ^^std::tuple<int, long>));
static_assert(!is_nothrow_applicable_type(^^decltype(nt), ^^NotTuple));
static_assert(apply_result(^^decltype(add), ^^std::tuple<int, long>) == ^^int);
static_assert(apply_result(^^Fn, ^^const std::array<int, 2> &) == ^^char);
static_assert(apply_result(^^decltype(take_ref), ^^std::tuple<int &> &&) == ^^long);
static_assert(is_applicable_type(^^decltype(span2), ^^std::ranges::subrange<int *>));
