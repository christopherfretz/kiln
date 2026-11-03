// Argument checks of the clang alignment builtins (clang SemaChecking
// BuiltinAlignment): __builtin_align_up / __builtin_align_down /
// __builtin_is_aligned take an integer (not bool or an enumeration) or a
// non-function pointer (an array decays) and an integer alignment that,
// when constant, is a power of 2 in [1, 2^(N-1)] for an N-bit source;
// aligning to 1 is warned (-Wtautological-compare). The result has the
// source's type (bool for __builtin_is_aligned).
// __builtin_frame_address takes a constant level in [0, 65535], warned
// when nonzero (-Wframe-address, in -Wall); __builtin_debugtrap takes no
// argument. Same at clang 19 and 22.
//@ args: -Wall
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:25 error "requested alignment must be 1 or greater"
//@ diag: 45:25 error "requested alignment is not a power of 2"
//@ diag: 46:25 warning "aligning a value to 1 byte is a no-op"
//@ diag: 46:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 47:27 warning "the result of checking whether a value is aligned to 1 byte is always true"
//@ diag: 47:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 48:25 error "requested alignment must be 1 or greater"
//@ diag: 49:38 error "requested alignment must be 128 or smaller"
//@ diag: 50:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 51:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 52:22 error "operand of type 'bool' where arithmetic or pointer type is required"
//@ diag: 53:22 error "operand of type 'E' where arithmetic or pointer type is required"
//@ diag: 54:22 error "operand of type 'void (*)()' where arithmetic or pointer type is required"
//@ diag: 55:22 error "operand of type 'S' where arithmetic or pointer type is required"
//@ diag: 56:22 error "operand of type 'float' where arithmetic or pointer type is required"
//@ diag: 57:25 error "used type 'float' where integer is required"
//@ diag: 58:25 error "used type 'bool' where integer is required"
//@ diag: 59:25 error "used type 'E' where integer is required"
//@ diag: 60:23 error "too few arguments to function call, expected 2, have 1"
//@ diag: 61:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 62:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 63:30 error "too many arguments to function call, expected 2, have 3"
//@ diag: 64:3 warning "calling '__builtin_frame_address' with a nonzero argument is unsafe"
//@ diag: 66:3 error "argument value 65536 is outside the valid range [0, 65535]"
//@ diag: 67:3 error "argument value 4294967295 is outside the valid range [0, 65535]"
//@ diag: 68:3 error "argument to '__builtin_frame_address' must be a constant integer"
//@ diag: 69:23 error "too many arguments to function call, expected 0, have 1"
struct S {};
enum E { e1 };
void f(int i, unsigned u, bool b, E e, int *p, void (*fp)(), S s, float fl, long long l, int (&arr)[4]) {
  __builtin_align_up(i, 0);
  __builtin_align_up(i, 3);
  __builtin_align_up(i, 1);
  __builtin_is_aligned(i, 1);
  __builtin_align_up(i, -1);
  __builtin_align_up((signed char)i, 256);
  __builtin_align_up((signed char)i, 128);
  __builtin_align_down(l, 1ull << 63);
  __builtin_align_up(b, 4);
  __builtin_align_up(e, 4);
  __builtin_align_up(fp, 4);
  __builtin_align_up(s, 4);
  __builtin_align_up(fl, 4);
  __builtin_align_up(i, fl);
  __builtin_align_up(i, b);
  __builtin_align_up(i, e);
  __builtin_align_up(i);
  __builtin_is_aligned(arr, 4);
  __builtin_is_aligned(i, u);
  __builtin_align_down(p, 4, 5);
  __builtin_frame_address(1);
  __builtin_frame_address(0);
  __builtin_frame_address(65536);
  __builtin_frame_address(-1);
  __builtin_frame_address(i);
  __builtin_debugtrap(1);
}
static_assert(__is_same(decltype(__builtin_align_up((short)1, 4)), short));
static_assert(__is_same(decltype(__builtin_is_aligned(1u, 4)), bool));
static_assert(__is_same(decltype(__builtin_align_down((int *)nullptr, 4)), int *));
