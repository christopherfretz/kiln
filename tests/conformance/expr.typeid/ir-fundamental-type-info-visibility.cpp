// Itanium C++ ABI §2.9.2, [expr.typeid]: the fundamental `type_info`
// objects take the visibility of `::__cxxabiv1::__fundamental_type_info`:
// default for a class marked so under -fvisibility=hidden (the runtime's
// `_LIBCXXABI_TYPE_VIS`), as clang (WP-1620). Compared with clang's IR
// exactly.
//@ expect: ir
//@ args: -fvisibility=hidden
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std

namespace __cxxabiv1 {
class __attribute__((visibility("default"))) __fundamental_type_info : public std::type_info {
public:
  ~__fundamental_type_info() override;
};
__fundamental_type_info::~__fundamental_type_info() {}
} // namespace __cxxabiv1
