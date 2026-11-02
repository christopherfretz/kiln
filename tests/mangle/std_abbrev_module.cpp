// Itanium ABI <substitution> (clang `mangleStandardSubstitution`): the
// abbreviations `Sa`, `Sb`, `Ss`, `Si`, `So` and `Sd` apply only to
// templates in `::std` that are not attached to a named module
// ([module.unit]/7). A `std` template declared in the purview of module
// `M` is spelled out under `St` and the module-name substitution
// (`StS_9allocatorIiE`), and a module-attached `char_traits<char>`
// argument also blocks `So`. `std` reopened inside `extern "C++"` declares
// global-module entities: no `W` prefix (`St13basic_ostream`).
// std_abbrev_module_global.cpp has the `extern "C++"` case keeping `So`.
export module M;
export namespace std {
template <class T> struct allocator {};
template <class T> struct char_traits {};
template <class C, class T = char_traits<C>, class A = allocator<C>>
struct basic_string {};
int f(int);
struct S {
  void m();
};
void S::m() {}
} // namespace std
extern "C++" {
namespace std {
template <class C, class T> struct basic_ostream {};
template <class C> struct traits2 {};
} // namespace std
}
export int f(int x) { return x; }
export void g1(std::allocator<int>) {}
export void g2(std::basic_string<char>) {}
export void g3(std::basic_string<int>) {}
export void g4(std::S) {}
export void g5(std::basic_ostream<char, std::char_traits<char>> &) {}
export void g6(std::basic_ostream<int, std::traits2<int>> &) {}
int std::f(int x) { return x; }
