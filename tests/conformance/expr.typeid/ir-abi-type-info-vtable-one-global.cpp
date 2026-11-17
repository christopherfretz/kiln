// [expr.typeid]/1, [class.virtual]; Itanium C++ ABI §2.9.5, §5.2.3: a
// `type_info` object points into the vtable of its `__cxxabiv1` class,
// which RTTI emission names by symbol. In a unit that also defines that
// class (the C++ runtime's own, found building libc++abi with kiln), the
// reference and the class's vtable are one global under the class's own
// symbol, whichever comes first: never a second `_ZTV...E.N` beside an
// undefined `_ZTV...E` (WP-1620). Compared with clang's IR exactly.
//@ expect: ir
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std

struct B0 { virtual ~B0(); };
struct B1 { virtual ~B1(); };
struct Before : B0 { ~Before(); };          // RTTI refers to __si_class_type_info
Before::~Before() {}
struct VBefore : B0, B1 { ~VBefore(); };    // __vmi_class_type_info
VBefore::~VBefore() {}
enum E { e0 };
const std::type_info *before[] = {
    &typeid(E), &typeid(void (*)(int)), &typeid(int[3]), &typeid(B0 *),
    &typeid(int B0::*), &typeid(void(int)),
};

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
class __vmi_class_type_info : public __class_type_info {
public:
  ~__vmi_class_type_info() override;
};
class __enum_type_info : public __shim_type_info {
public:
  ~__enum_type_info() override;
};
class __function_type_info : public __shim_type_info {
public:
  ~__function_type_info() override;
};
class __array_type_info : public __shim_type_info {
public:
  ~__array_type_info() override;
};
class __pbase_type_info : public __shim_type_info {
public:
  ~__pbase_type_info() override;
};
class __pointer_type_info : public __pbase_type_info {
public:
  ~__pointer_type_info() override;
};
class __pointer_to_member_type_info : public __pbase_type_info {
public:
  ~__pointer_to_member_type_info() override;
};
__shim_type_info::~__shim_type_info() {}
__class_type_info::~__class_type_info() {}
__si_class_type_info::~__si_class_type_info() {}
__vmi_class_type_info::~__vmi_class_type_info() {}
__enum_type_info::~__enum_type_info() {}
__function_type_info::~__function_type_info() {}
__array_type_info::~__array_type_info() {}
__pbase_type_info::~__pbase_type_info() {}
__pointer_type_info::~__pointer_type_info() {}
__pointer_to_member_type_info::~__pointer_to_member_type_info() {}
} // namespace __cxxabiv1

struct After : B1 { ~After(); };
After::~After() {}
struct VAfter : B1, B0 { ~VAfter(); };
VAfter::~VAfter() {}
enum F { f0 };
const std::type_info *after[] = {
    &typeid(F), &typeid(long (*)()), &typeid(char[2]), &typeid(B1 *),
    &typeid(int B1::*), &typeid(void(long)),
};
