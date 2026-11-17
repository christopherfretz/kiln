// [expr.typeid]; Itanium C++ ABI §2.9.2: only
// `::__cxxabiv1::__fundamental_type_info` defines the fundamental
// `type_info` objects (WP-1620). A class of that name in another
// namespace, in an inline namespace of `__cxxabiv1` or in a `__cxxabiv1`
// nested in another namespace does not (clang's test: the namespace's
// context is the translation unit itself), and `typeid(int)` stays a
// reference to the runtime's object. Compared with clang's IR exactly.
//@ expect: ir
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std

namespace other {
struct __fundamental_type_info : std::type_info { ~__fundamental_type_info(); };
__fundamental_type_info::~__fundamental_type_info() {}
} // namespace other
namespace __cxxabiv1 {
inline namespace v1 {
struct __fundamental_type_info : std::type_info { ~__fundamental_type_info(); };
__fundamental_type_info::~__fundamental_type_info() {}
} // namespace v1
} // namespace __cxxabiv1
namespace outer {
namespace __cxxabiv1 {
struct __fundamental_type_info : std::type_info { ~__fundamental_type_info(); };
__fundamental_type_info::~__fundamental_type_info() {}
} // namespace __cxxabiv1
} // namespace outer
const std::type_info *ti() { return &typeid(int); }
const std::type_info *tp() { return &typeid(const void *); }
