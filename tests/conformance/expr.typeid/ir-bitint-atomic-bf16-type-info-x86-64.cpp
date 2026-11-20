// [expr.typeid]/1, [except.throw], [except.handle] on x86-64 (pinned, no
// header): the type_info of a _BitInt, an _Atomic type, __bf16 and
// pointers to them is not in the C++ runtime (clang
// TypeInfoIsInStandardLibrary, BuildTypeInfo), so it is emitted here as a
// linkonce_odr abi::__fundamental_type_info (or __pointer_type_info), for
// typeid, a throw and a handler alike. kiln failed on the first two and
// referenced __bf16's as an external symbol. Compared with clang's IR
// exactly.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std

const std::type_info *infos[] = {
    &typeid(_BitInt(7)),         &typeid(unsigned _BitInt(65)),
    &typeid(_BitInt(7) *),       &typeid(_Atomic(int)),
    &typeid(_Atomic(int) *),     &typeid(const _Atomic(int) *),
    &typeid(__bf16),             &typeid(const __bf16 *),
};

void thrower(int i) {
  if (i)
    throw (_BitInt(9))1;
  try {
    throw (__bf16)1.0f;
  } catch (_Atomic(long) *) {
  } catch (__bf16) {
  }
}
