// Mangling differential: libstdc++'s `std::__cxx11` spelling (WP-384).
// The inline namespace's `abi_tag("cxx11")` tags the entities declared in
// it, not the namespace (`_ZNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEC2Ev`),
// so `Ss` does not apply; a function or variable whose return or variable
// type mentions a tagged type and whose name/parameters do not carries
// the missing tags (`_ZNKSt6locale4nameB5cxx11Ev`, `_Z2gsB5cxx11`; clang
// `AbiTagState`, `makeFunctionReturnTypeTags`). Local entities restart
// from the tags the enclosing encoding emitted.
namespace std {
template <class T> struct allocator {};
template <class C> struct char_traits {};
inline namespace __cxx11 __attribute__((abi_tag("cxx11"))) {
template <class C, class T = char_traits<C>, class A = allocator<C>> struct basic_string {
  basic_string() {}
  basic_string(const char*) {}
  int size() const { return 0; }
};
typedef basic_string<char> string;
}
struct locale { string name() const { return string(); } static string sname(); };
string locale::sname() { return string(); }
}
struct __attribute__((abi_tag("x"))) Tagged { int v; };
Tagged mk() { return Tagged(); }
struct M { Tagged get() { return Tagged(); } Tagged* ptr() { return nullptr; } Tagged arg(Tagged t) { return t; } };
template <class T> struct W { T t; };
W<Tagged> wrap() { return W<Tagged>(); }
template <class T> T id(T t) { return t; }
template <class T> W<T> wid(int) { return W<T>(); }
std::string gs;
W<Tagged> gw;
Tagged gt;
namespace N { std::string ns; Tagged nt; }
std::string str() { return "x"; }
void take(std::string) {}
inline std::string& local() { static std::string s; return s; }
inline Tagged loc2() { struct L { Tagged t() { return Tagged(); } }; return L().t(); }
struct __attribute__((abi_tag("y"))) Y { Tagged m() { return Tagged(); } Y self() { return *this; } };
int use() {
  std::locale l; l.name(); std::locale::sname(); mk(); M m; m.get(); m.ptr(); m.arg(Tagged()); wrap(); id(Tagged()); wid<Tagged>(1);
  str(); take("a"); local(); loc2(); Y y; y.m(); y.self();
  return gs.size() + gw.t.v + gt.v + N::ns.size() + N::nt.v;
}
