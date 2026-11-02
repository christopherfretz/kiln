// [namespace.def.general]/3: the identifier of a named namespace definition
// is looked up in the enclosing namespace and in every element of its
// inline namespace set; a namespace found there is extended. libc++'s
// tests specialize `std::ranges::enable_borrowed_range` inside
// `namespace std::ranges {`, which reopens `std::__1::ranges` (WP-309).
//@ expect: run
//@ strict-diags
namespace A {
inline namespace I {
namespace R {
int x = 1;
template <class T> constexpr bool e = false;
} // namespace R
} // namespace I
} // namespace A

struct X {};

namespace A {
namespace R {
int y = x + 1; // finds A::I::R::x unqualified
}
} // namespace A

namespace A::R {
int z = y + 1;
template <> constexpr bool e<::X> = true; // [lex.pptoken]/3: `<` `::`
} // namespace A::R

static_assert(A::I::R::e<X>);
static_assert(A::R::e<X> && !A::R::e<int>);

int main() { return A::I::R::y + A::I::R::z == 5 ? 0 : 1; }
