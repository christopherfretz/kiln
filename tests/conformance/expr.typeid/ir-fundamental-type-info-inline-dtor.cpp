// Itanium C++ ABI §2.9.2, §5.2.3: `::__cxxabiv1::__fundamental_type_info`
// without a key function (an inline destructor) defines its vtable
// `linkonce_odr` where used; the fundamental `type_info` objects are
// still defined there, strong, as clang does (WP-1620); a unit that does
// not use the class defines none. Compared with clang's IR exactly
// (C++23: no C++26 erroneous-value initialization of the local).
//@ expect: ir
//@ std: c++23
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
  ~__fundamental_type_info() override {}
};
} // namespace __cxxabiv1
void use() { __cxxabiv1::__fundamental_type_info t; }
