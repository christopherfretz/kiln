// [temp.constr.atomic]/3, [temp.res.general]: a concept's atomic
// constraints are substituted in the context of the concept's definition,
// not where satisfaction is checked: the unqualified `swap` finds the
// deleted `R::swap` (plus `N::swap` by argument-dependent lookup, an
// ambiguity), even when `R::C<N::Q>` is checked inside namespace `N`
// (libc++ `__swappable_with` checked from `ranges::swap`).
// Verified against clang 19.
//@ expect: run
namespace N {
template <class T> void swap(T&, T&) {}
struct Q {};
} // namespace N
namespace R {
template <class T> void swap(T&, T&) = delete;
template <class T> concept C = requires(T& a) { swap(a, a); };
struct S {};
} // namespace R
namespace N {
static_assert(!R::C<Q>);
static_assert(!R::C<R::S>);
static_assert(!R::C<int>);
} // namespace N
int main() { return 0; }
