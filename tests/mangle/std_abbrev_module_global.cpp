// Itanium ABI <substitution>, [module.unit]/7: `std` templates declared in
// `extern "C++"` inside a module purview are attached to the global module,
// so their standard abbreviations still apply (`So`), unlike the
// module-attached ones in std_abbrev_module.cpp.
export module M;
extern "C++" {
namespace std {
template <class C> struct char_traits {};
template <class C, class T> struct basic_ostream {};
template <class T> struct allocator {};
} // namespace std
}
export void g1(std::basic_ostream<char, std::char_traits<char>> &) {}
export void g2(std::allocator<int>) {}
