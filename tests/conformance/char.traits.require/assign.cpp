// [char.traits.require]: X::assign(c, d) assigns d to c. The Oven
// evaluates char_traits<C>::assign natively where it is a plain
// assignment (spec/oven.md "Library fast paths") and runs the library's
// code elsewhere (storage not yet holding an object, a union member),
// with the library's result.
//@ expect: compile-ok
#include <memory>
#include <string>
template <class C> constexpr bool one(C a, C b) {
  C buf[2] = {a, a};
  std::char_traits<C>::assign(buf[1], b);
  return buf[0] == a && buf[1] == b;
}
static_assert(one<char>('a', 'z'));
static_assert(one<wchar_t>(L'a', L'z'));
static_assert(one<char8_t>(u8'a', u8'z'));
static_assert(one<char16_t>(u'a', u'z'));
static_assert(one<char32_t>(U'a', U'\U0010FFFF'));
constexpr bool string() {
  std::string s = "hello";
  s[0] = 'j';
  s.append(3, '!');
  s.insert(s.begin(), '<');
  return s == "<jello!!!";
}
static_assert(string());
constexpr bool fresh() {
  std::allocator<char> al;
  char *p = al.allocate(2);
  std::char_traits<char>::assign(p[0], 'x');
  std::char_traits<char>::assign(p[1], p[0]);
  bool ok = p[0] == 'x' && p[1] == 'x';
  al.deallocate(p, 2);
  return ok;
}
static_assert(fresh());
union U {
  int i;
  char c;
};
constexpr bool member() {
  U u{1};
  std::char_traits<char>::assign(u.c, 'q');
  return u.c == 'q';
}
static_assert(member());
