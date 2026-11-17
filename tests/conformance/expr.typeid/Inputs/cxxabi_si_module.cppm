// A module interface that defines the C++ runtime's `__si_class_type_info`
// (attached to the global module) after RTTI that refers to its vtable
// (WP-1620).
export module cxxabi_si;
extern "C++" {
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std
}
export struct B0 { virtual ~B0(); };
export struct D0 : B0 { ~D0(); };
B0::~B0() {}
D0::~D0() {}
export const std::type_info *early() { return &typeid(D0); }
extern "C++" {
namespace __cxxabiv1 {
class __shim_type_info : public std::type_info {
public:
  ~__shim_type_info() override;
};
class __class_type_info : public __shim_type_info {
public:
  ~__class_type_info() override;
};
class __si_class_type_info : public __class_type_info {
public:
  ~__si_class_type_info() override;
  const __class_type_info *base;
};
__shim_type_info::~__shim_type_info() {}
__class_type_info::~__class_type_info() {}
__si_class_type_info::~__si_class_type_info() {}
} // namespace __cxxabiv1
}
export const void *si_address_point() {
  __cxxabiv1::__si_class_type_info si;
  return *reinterpret_cast<void *const *>(&si);
}
