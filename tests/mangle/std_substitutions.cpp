// Mangling differential: Itanium standard substitutions ([abi.mangling]
// 5.1.5 "Compression", clang `mangleStandardSubstitution`; WP-384). `Sa`
// and `Sb` name the templates `::std::allocator` / `::std::basic_string`;
// `Ss`/`Si`/`So`/`Sd` the specializations over `char` with
// `std::char_traits<char>` (and `std::allocator<char>`); none enters the
// substitution table. A mock `std` declared directly in `::std` (the
// libstdc++ `_GLIBCXX_USE_CXX11_ABI=0` / pre-C++11 spelling).
namespace std {
template <class T> struct allocator { allocator() {} void f() {} };
template <class C> struct char_traits {};
template <class C, class T = char_traits<C>, class A = allocator<C>> struct basic_string {
  basic_string() {}
  int size() const { return 0; }
};
template <class C, class T = char_traits<C>> struct basic_istream { void g() {} };
template <class C, class T = char_traits<C>> struct basic_ostream { void g() {} };
template <class C, class T = char_traits<C>> struct basic_iostream { void g() {} };
typedef basic_string<char> string;
void sf(string, basic_string<wchar_t>, allocator<int>) {}
}
template <template <class> class> struct TT {};
void f1(std::string, std::string, std::allocator<char>, std::basic_istream<char>&, std::basic_ostream<char>&, std::basic_iostream<char>&) {}
void f2(std::basic_string<char, std::char_traits<char>, std::allocator<int>>, std::basic_istream<wchar_t>*) {}
void f3(TT<std::allocator>) {}
template <class T> void f4(std::allocator<T>, T) {}
int use() {
  std::allocator<char> a; a.f();
  std::basic_string<char> s; std::basic_istream<char> i; i.g(); std::basic_ostream<char> o; o.g(); std::basic_iostream<char> io; io.g();
  f4(std::allocator<int>(), 1);
  return s.size();
}
