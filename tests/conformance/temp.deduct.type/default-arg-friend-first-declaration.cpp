// [temp.deduct.type]/9, [temp.arg.general]/9, [temp.friend]/1: a class
// template-id `P` written with fewer arguments than its template has is
// completed with the defaults, substituted with `P`'s own arguments, before
// it is matched against `A`. The defaults and their parameters are the
// template's own (depth 0) even when its first declaration is a friend
// declaration inside a class template, whose parameters sit one level
// deeper. Reduced from libc++ 19 `std::format` -> `std::vformat_to` ->
// `std::__vformat_to(_OutIt, basic_string_view<_CharT>, ...)`, where
// `basic_string_view` is first declared as a friend of `__wrap_iter`
// (WP-286). Verified against clang 19.
//@ expect: run
//@ exit: 3
template <class C> struct traits {};
template <class I> class wrap {
  template <class C, class Tr> friend class sv;
};
template <class C, class Tr = traits<C>> class sv;
template <class C, class Tr> class sv {};
using string_view = sv<char>;

template <class O, class C> O vf(O o, sv<C>) { return o; }
template <class O, class C, class D> int two(O, sv<C>, sv<D>) { return sizeof(D); }

int main() {
  int r = vf(1, string_view{});
  r += two(0, sv<char>{}, sv<short>{});
  return r;
}
