// [temp.func.order]/3, [temp.deduct.partial], [temp.deduct.type]/20: for
// function templates ordered by `V<E>` (with `template <auto> struct V`),
// `template <int E> f(V<E>)` is more specialized than `template <auto E>
// f(V<E>)`: deducing `int E` from the synthesized `V<E'>`, `E'` of the
// unique type invented for `auto`, fails because the type of `E'` (that of
// `V`'s corresponding parameter) is not `int` (/20, second sentence), while
// `auto E` deduces from a synthesized `int` value. The same holds for
// `decltype(auto)`. This is the rule clang 19 and 22 apply to class template
// partial specializations (temp.spec.partial.order/
// typed-nttp-more-specialized-than-auto-run.cpp) but not to function
// templates, which they call ambiguous; GCC 16.1 agrees with kiln. A call
// with `V<'c'>` deduces only the `auto` one (all three agree).
//@ std: c++20
//@ expect: run
//@ oracle: none
//@ xfail(clang): clang 19 and 22 report the calls f(V<5>{}) and g(V<5>{}) ambiguous
//@ stdout: 2 1 2 1
template <auto E> struct V {};
template <auto E> int f(V<E>) { return 1; }
template <int E> int f(V<E>) { return 2; }
template <decltype(auto) E> int g(V<E>) { return 1; }
template <int E> int g(V<E>) { return 2; }

extern "C" int printf(const char *, ...);
int main() { printf("%d %d %d %d\n", f(V<5>{}), f(V<'c'>{}), g(V<5>{}), g(V<'c'>{})); }
