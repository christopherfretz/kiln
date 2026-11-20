// [expr.typeid]/1 on AArch64 (pinned, no header): the type_info of __bf16,
// the SVE ACLE types and pointers to them is not in the C++ runtime (clang
// TypeInfoIsInStandardLibrary), so it is emitted here as a linkonce_odr
// abi::__fundamental_type_info (or __pointer_type_info), where kiln
// referenced an external symbol. Compared with clang's IR exactly.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std

const std::type_info *infos[] = {
    &typeid(__bf16),         &typeid(__bf16 *),
    &typeid(__SVInt32_t),    &typeid(__SVBool_t),
    &typeid(__SVCount_t),    &typeid(__SVFloat64_t *),
    &typeid(__clang_svint8x2_t), &typeid(__clang_svboolx4_t),
    &typeid(__SVBfloat16_t), &typeid(const __SVUint8_t *),
};

const std::type_info &svint8() { return typeid(__SVInt8_t); }
