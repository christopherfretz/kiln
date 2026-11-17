// [expr.typeid], [dcl.link]; Itanium C++ ABI §2.9.2: a
// `__cxxabiv1::__fundamental_type_info` declared inside `extern "C++"`
// defines no fundamental `type_info` objects, as clang 22 (its test asks
// for the namespace's context to be the translation unit itself, WP-1620).
// Compared with clang's IR exactly.
//@ expect: ir
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std

extern "C++" {
namespace __cxxabiv1 {
struct __fundamental_type_info : std::type_info { ~__fundamental_type_info(); };
__fundamental_type_info::~__fundamental_type_info() {}
} // namespace __cxxabiv1
}
const std::type_info *ti() { return &typeid(int); }
