// Mangling differential: libc++'s `std::__1` spelling (WP-384). The
// templates live in the inline namespace `std::__1`, not directly in
// `::std`, so neither `Sa`/`Sb` nor `Ss`/`Si`/`So`/`Sd` applies; only the
// `St` prefix of entities declared in `::std` itself is abbreviated.
namespace std {
inline namespace __1 {
template <class T> struct allocator { allocator() {} void f() {} };
template <class C> struct char_traits {};
template <class C, class T = char_traits<C>, class A = allocator<C>> struct basic_string {
  basic_string() {}
  int size() const { return 0; }
};
template <class C, class T = char_traits<C>> struct basic_ostream { void g() {} };
typedef basic_string<char> string;
}
struct in_std { void h() {} };
void sf(string, allocator<int>) {}
}
template <template <class> class> struct TT {};
void f1(std::string, std::allocator<char>, std::basic_ostream<char>&) {}
void f3(TT<std::allocator>) {}
int use() {
  std::allocator<char> a; a.f();
  std::basic_ostream<char> o; o.g();
  std::in_std i; i.h();
  std::string s;
  return s.size();
}
