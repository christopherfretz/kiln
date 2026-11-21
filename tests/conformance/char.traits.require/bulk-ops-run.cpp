// [char.traits.require], [char.traits.specializations]: compare, length
// and find over each character type, in constant evaluation (the Oven's
// bulk operations, spec/oven.md "Library fast paths") and at run time.
// char_traits<char>::lt compares as unsigned char, so '\xff' sorts after
// 'a' ([char.traits.specializations.char]/1); a union member read after
// its string is reassigned exercises the one-array run check.
//@ expect: run
//@ stdout: ok -1 1 0 3 2 1
#include <cstdio>
#include <string>
#include <string_view>

template <class C> constexpr int cmp(const C *a, const C *b, std::size_t n) {
  return std::char_traits<C>::compare(a, b, n);
}

template <class C> constexpr long where(const C *s, std::size_t n, C c) {
  const C *p = std::char_traits<C>::find(s, n, c);
  return p ? p - s : -1;
}

constexpr bool all() {
  const char hi[] = {'\xff', 0};
  if (cmp<char>(hi, "a", 1) <= 0 || cmp<char>("a", hi, 1) >= 0)
    return false;
  if (cmp<char>("abcd", "abce", 4) != -1 || cmp<char>("abce", "abcd", 4) != 1 ||
      cmp<char>("abcd", "abce", 3) != 0 || cmp<char>("x", "y", 0) != 0)
    return false;
  if (cmp<wchar_t>(L"ab", L"ac", 2) != -1 || cmp<char16_t>(u"b", u"a", 1) != 1 ||
      cmp<char32_t>(U"\U0001F600", U"\U0001F600", 1) != 0 ||
      cmp<char8_t>(u8"\xff", u8"a", 1) != 1)
    return false;
  if (std::char_traits<char>::length("hello") != 5 || std::char_traits<char>::length("") != 0 ||
      std::char_traits<wchar_t>::length(L"abc") != 3 || std::char_traits<char32_t>::length(U"ab") != 2)
    return false;
  if (where<char>("hello", 5, 'l') != 2 || where<char>("hello", 2, 'l') != -1 ||
      where<char>("hello", 0, 'h') != -1 || where<char16_t>(u"xyz", 3, u'z') != 2)
    return false;
  std::string s = "short";
  std::string_view v1 = s;
  bool eq1 = v1 == "short";
  s = "a much longer string that leaves the local buffer";
  std::string_view v2 = s;
  return eq1 && v2.find("leaves") == 26 && v2.compare("a much") > 0;
}
static_assert(all());

int main(int argc, char **) {
  // Not constant: the library's run-time path (argc keeps it so).
  const char *a = argc > 5 ? "zz" : "abcd";
  std::printf("%s %d %d %d %zu %ld %ld\n", all() ? "ok" : "bad", cmp<char>(a, "abce", 4),
              cmp<char>("abce", a, 4), cmp<char>(a, "abcd", 4), std::char_traits<char>::length("abc"),
              where<char>(a, 4, 'c'), where<char>(a, 4, 'b'));
}
