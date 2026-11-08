// [temp.over.link]/5, [temp.constr.order], P2996 [temp.type]: a type
// splice `typename [: R :]` in a concept-id's parameter mapping is
// equivalent to the same splice in another template, so the overload
// adding a conjunct is more constrained. GCC 16.1 reports the call as
// ambiguous, against the wording (it does match the same type splice in
// redeclarations; dependent-splice-equivalent-run.cpp). WP-884.
//@ expect: compile-ok
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ skip(gcc): GCC 16.1 does not treat equal type splices in a concept-id's mapping as equivalent
//@ strict-diags
using info = decltype(^^int);
template <class T> concept C = sizeof(T) > 0;
template <info R> requires C<typename [:R:]> constexpr int f(int) { return 1; }
template <info R> requires C<typename [:R:]> && true constexpr int f(int) { return 2; }
static_assert(f<^^int>(0) == 2);
