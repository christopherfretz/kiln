// Itanium C++ ABI §2.9.2 on x86-64 (pinned, no header): the same list of
// fundamental `type_info` objects as on AArch64 (`__float128`, `long
// double`, `__int128`, `__fp16`, `char8_t`), defined by the key function
// of `::__cxxabiv1::__fundamental_type_info` (WP-1620). Compared with
// clang's IR exactly.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std

namespace __cxxabiv1 {
class __fundamental_type_info : public std::type_info {
public:
  ~__fundamental_type_info() override;
};
__fundamental_type_info::~__fundamental_type_info() = default;
} // namespace __cxxabiv1
