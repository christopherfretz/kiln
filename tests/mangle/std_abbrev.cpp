// Mangling differential (crates/kiln_codegen/tests/mangle_vs_clang.rs):
// the Itanium standard abbreviations ([abi:mangling.compression]) — `Sa`
// ::std::allocator, `Sb` ::std::basic_string, `Ss`
// ::std::basic_string<char, ::std::char_traits<char>,
// ::std::allocator<char>>, `Si`/`So`/`Sd` ::std::basic_{i,o,io}stream<char,
// ::std::char_traits<char>> — which are never substitution candidates
// themselves; only entities declared directly in ::std qualify (a
// basic_string in an inline namespace of std, as libstdc++'s
// std::__cxx11, or any other nested namespace, is spelled out). An inline namespace's abi_tag is not
// mangled after its name (`St7__cxx11`, no `B5cxx11`).
namespace std {
template <class T> struct char_traits {};
template <class T> struct allocator {
  allocator();
  allocator(const allocator &);
  ~allocator();
  T *allocate(unsigned long);
  static int count;
};
template <class C, class Tr = char_traits<C>, class A = allocator<C>> struct basic_string {
  basic_string();
  basic_string(const basic_string &);
  ~basic_string();
  void assign(const basic_string &, A);
};
template <class C, class Tr = char_traits<C>> struct basic_istream { void get(); };
template <class C, class Tr = char_traits<C>> struct basic_ostream { void put(C); };
template <class C, class Tr = char_traits<C>> struct basic_iostream { void sync(); };
template <class T> struct vector {};
namespace __nested {
template <class C, class Tr = char_traits<C>, class A = allocator<C>> struct basic_string {
  basic_string(const basic_string &);
  void swap(basic_string &);
};
}
inline namespace __attribute__((__abi_tag__("cxx11"))) __cxx11 {
struct tagged { void f(); };
template <class T> struct tagged_tmpl { void g(); };
}
} // namespace std

namespace other {
template <class T> struct allocator { void f(); };
template <class C, class Tr = std::char_traits<C>> struct basic_ostream { void put(); };
}

template <template <class> class TT> struct holder { void h(); };

void f1(std::allocator<char>, std::allocator<int>) {}
void f2(std::basic_string<char>, const std::basic_string<char> &) {}
void f3(std::basic_string<wchar_t>, std::basic_string<char, std::char_traits<char>, std::allocator<int>>) {}
void f4(std::basic_istream<char> &, std::basic_ostream<char> &, std::basic_iostream<char> &) {}
void f5(std::basic_ostream<wchar_t> &, std::basic_istream<char, std::char_traits<int>> &) {}
void f6(std::vector<std::allocator<char>>, std::allocator<std::vector<int>>) {}
void f7(std::__nested::basic_string<char>, const std::__nested::basic_string<char> &, std::allocator<char>) {}
void f8(other::allocator<char>, other::basic_ostream<char> &) {}
void f9(holder<std::allocator>, holder<std::vector>) {}
template <class T> void tf(T, std::allocator<T>) {}
template void tf<char>(char, std::allocator<char>);

void use() {
  std::allocator<char> a;
  std::allocator<char> b(a);
  (void)a.allocate(1);
  (void)std::allocator<char>::count;
  std::basic_string<char> s;
  std::basic_string<char> t(s);
  s.assign(t, a);
  std::basic_string<wchar_t> w;
  std::basic_istream<char> *i = nullptr;
  i->get();
  std::basic_ostream<char> *o = nullptr;
  o->put('c');
  std::basic_iostream<char> *io = nullptr;
  io->sync();
  std::__nested::basic_string<char> *n = nullptr;
  std::__nested::basic_string<char> m(*n);
  m.swap(*n);
  std::tagged().f();
  std::tagged_tmpl<std::allocator<char>>().g();
  other::allocator<char>().f();
  holder<std::allocator>().h();
}
