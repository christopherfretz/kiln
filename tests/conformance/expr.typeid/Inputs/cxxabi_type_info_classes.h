// The C++ runtime's `type_info` classes, declared (WP-1620).
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std
struct B0 { virtual ~B0(); };
struct D0 : B0 { ~D0(); };
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
} // namespace __cxxabiv1
