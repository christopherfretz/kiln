// [expr.typeid], [except.handle]; Itanium C++ ABI §2.9.2: the C++ runtime
// contains the `type_info` objects of every fundamental type X and of X*
// and const X*. They are defined, external with the class's visibility,
// where the vtable of `::__cxxabiv1::__fundamental_type_info` is defined
// (here by its key function, its destructor), as clang and GCC do (found
// building libc++abi with kiln, WP-1620). A `typeid(int)` before the
// destructor refers to that definition. Compared with clang's IR exactly.
//@ expect: ir
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std

const std::type_info *early() { return &typeid(int); }

namespace __cxxabiv1 {
class __shim_type_info : public std::type_info {
public:
  ~__shim_type_info() override;
};
class __fundamental_type_info : public __shim_type_info {
public:
  ~__fundamental_type_info() override;
};
__fundamental_type_info::~__fundamental_type_info() {}
} // namespace __cxxabiv1

const std::type_info *late() { return &typeid(const char *); }
