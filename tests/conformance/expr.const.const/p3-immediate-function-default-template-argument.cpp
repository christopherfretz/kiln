// [expr.const.imm]/1.2, [temp.arg.nontype]/1, [expr.const.const]/1, /3
// (P4101R1): a constant template argument is a converted constant
// expression, manifestly constant-evaluated, so a reference to an
// immediate function in it, also in a default template argument
// substituted at a call, is in an immediate function context; its value,
// a pointer to an immediate function, is consteval-only and its template
// parameter object an immediate object, which /3 allows. So substitution
// of `&escalates<T>` (a constexpr function template that escalates to an
// immediate function, [expr.const.imm]/4) and of `&cf` succeeds and the
// `int` overload is chosen. kiln and GCC 16.1 agree; clang 22 rejects the
// argument under the pre-P4101R1 rule (no constant expression points to
// an immediate function; docs/status/deviations.md).
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
//@ skip(clang): clang 22 does not implement P4101R1: a pointer to an immediate function is not a template argument
consteval void cf(int*) {}
template <class T> constexpr void escalates(T p) { cf(p); }

template <class T, void (*)(T) = &escalates<T>> constexpr int check(int) { return 1; }
template <class T> constexpr int check(long) { return 2; }
static_assert(check<int*>(0) == 1);

template <class T, void (*)(int*) = &cf> constexpr int direct(int) { return 1; }
template <class T> constexpr int direct(long) { return 2; }
static_assert(direct<int>(0) == 1);

template <void (*F)(int*)> struct X {
  static constexpr bool ok = F != nullptr;
};
static_assert(X<&cf>::ok && X<&escalates<int*>>::ok);
