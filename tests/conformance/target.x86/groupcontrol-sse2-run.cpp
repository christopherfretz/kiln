// [driver] The SSE2 group-control shape of BDE's
// bdlc_flathashtable_groupcontrol.h, which includes <immintrin.h> and
// <emmintrin.h> under `__SSE2__`: `_mm_loadu_si128` of 16 control bytes,
// `_mm_movemask_epi8` of `_mm_cmpeq_epi8` against `_mm_set1_epi8` to find
// the slots holding a hash's low bits, and of the bytes themselves for the
// empty / erased ones. `__builtin_ia32_pmovmskb128` lowers to
// `llvm.x86.sse2.pmovmskb.128` (spec/landed/codegen/WP-630.md). WP-630.
//@ requires: target-x86_64
//@ expect: run
//@ std: c++23
//@ stdout: match 5: 0x0811 match 0: 0x0000 available: 0xc0c0 inuse: 0x3f3f neverfull: 1
#include <immintrin.h>
#include <emmintrin.h>
#include <cstdint>
#include <cstdio>

class GroupControl {
  __m128i d_value;

  std::uint32_t matchRaw(std::uint8_t value) const {
    return _mm_movemask_epi8(_mm_cmpeq_epi8(_mm_set1_epi8(static_cast<char>(value)), d_value));
  }

public:
  static const std::uint8_t k_EMPTY = 0x80;
  static const std::uint8_t k_ERASED = 0xC0;
  explicit GroupControl(const std::uint8_t *data)
      : d_value(_mm_loadu_si128(static_cast<const __m128i *>(static_cast<const void *>(data)))) {}
  std::uint32_t available() const { return _mm_movemask_epi8(d_value); }
  std::uint32_t inUse() const { return (~available()) & 0xFFFF; }
  std::uint32_t match(std::uint8_t value) const { return matchRaw(value); }
  bool neverFull() const { return 0 != matchRaw(k_EMPTY); }
};

int main() {
  // One byte past the group: an unaligned load.
  std::uint8_t bytes[17] = {0};
  std::uint8_t *g = bytes + 1;
  const std::uint8_t ctl[16] = {5, 1, 2, 3, 5, 9, 0x80, 0xC0, 7, 7, 7, 5, 4, 3, 0x80, 0xC0};
  for (int i = 0; i < 16; ++i)
    g[i] = ctl[i];
  GroupControl c(g);
  std::printf("match 5: 0x%04x match 0: 0x%04x available: 0x%04x inuse: 0x%04x neverfull: %d\n",
              c.match(5), c.match(0), c.available(), c.inUse(), c.neverFull());
}
