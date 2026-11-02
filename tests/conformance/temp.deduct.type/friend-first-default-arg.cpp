// [temp.param]/12, [temp.arg.general]/9, [temp.deduct.type]/9: a dependent
// `P` = `bsv<C>` is completed with the default `tr<C>` before it is deduced
// against `A`. When the first declaration of the class template is a friend
// redeclaration inside a class template (deeper parameters), the default
// still comes from the namespace-scope declaration, substituted at its own
// depth: `C` is not mistaken for the function's first parameter. libc++
// `__wrap_iter`'s `template <class _CharT, class _Traits> friend class
// basic_string_view;` before `<__fwd/string_view.h>` (`std::vformat_to`
// calling `__vformat_to(out, basic_string_view<_CharT>, ...)`). WP-296.
//@ expect: compile-ok

template <class I> struct W { template <class C, class T> friend class bsv; };
template <class C> struct tr {};
template <class C, class T = tr<C>> class bsv;
using sv = bsv<char>;
template <class C, class T> class bsv { public: constexpr bsv(const C*) {} };

template <class O, class C> constexpr O zz(O o, bsv<C>) { return o; }
template <class O> constexpr O yy(O o, sv f) { return zz(o, f); }
static_assert(yy(3L, "x") == 3L);

template <class O, class C, class F> constexpr int ww(O, bsv<C>, F f) { return f; }
static_assert(ww(nullptr, sv("x"), 7) == 7);

int main() {}
