// [meta.reflection.layout] (P2996R13): offset_of needs a non-static data
// member or a direct base; size_of and alignment_of reject bit-fields,
// reference variables, functions, namespaces and incomplete types;
// bit_size_of rejects what size_of does except a bit-field. Each failure
// throws std::meta::exception.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:15 note "reflection unsuitable for offset_of"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:15 note "reflection unsuitable for offset_of"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:15 note "reflection unsuitable for offset_of"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:15 note "reflection unsuitable for offset_of"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:15 note "reflection not suitable for size_of"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:15 note "reflection not suitable for alignment_of"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 63:15 note "reflection with incomplete type in size_of"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:15 note "reflection with incomplete type in alignment_of"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:15 note "reflection with incomplete type in bit_size_of"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:15 note "reflection not suitable for size_of"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 67:15 note "reflection not suitable for size_of"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:15 note "reflection not suitable for alignment_of"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 69:15 note "reflection not suitable for bit_size_of"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 70:15 note "reflection not suitable for size_of"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 71:15 note "reflection with incomplete type in size_of"
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;
struct S {
  int m;
  int bf : 3;
  static int st;
  void f();
};
struct Inc;
int g;
int& gr = g;
namespace N {}
static_assert(offset_of(^^S::st).bytes == 0);
static_assert(offset_of(^^S::f).bytes == 0);
static_assert(offset_of(^^g).bytes == 0);
static_assert(offset_of(^^S).bytes == 0);
static_assert(size_of(^^S::bf) == 0);
static_assert(alignment_of(^^S::bf) == 0);
static_assert(size_of(^^Inc) == 0);
static_assert(alignment_of(^^Inc) == 0);
static_assert(bit_size_of(^^Inc) == 0);
static_assert(size_of(^^gr) == 0);
static_assert(size_of(^^S::f) == 0);
static_assert(alignment_of(^^N) == 0);
static_assert(bit_size_of(^^N) == 0);
static_assert(size_of(info()) == 0);
static_assert(size_of(^^void) == 0);
