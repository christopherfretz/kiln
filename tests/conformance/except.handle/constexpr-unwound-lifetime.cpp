// [except.ctor]/1, [basic.life]/4, [expr.const]/10.8 (C++26, P3068R6):
// unwinding ends the lifetime of the automatic objects of every block and
// frame it leaves, exactly as a normal exit does. A pointer to one of them
// dangles in the handler (reading through it is not a constant
// expression), and an object that is unwound must still be within its
// lifetime when its destructor runs.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 32:12 note "read of variable whose lifetime has ended"
//@ diag: 36:19 note "in call to 'read_unwound_frame()'"
//@ diag: 22:7 note "declared here"
//@ diag: 48:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 45:12 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 48:19 note "in call to 'read_unwound_block()'"
//@ diag: 71:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 56:5 note "destroying object 'x' whose lifetime has already ended"
//@ diag: 65:5 note "in call to 'end_then_throw(true)'"
//@ diag: 71:19 note "in call to 'destroy_ended()'"
constexpr void escape(int **out) {
  int local = 1;
  *out = &local;
  throw 0;
}

constexpr int read_unwound_frame() {
  int *p = nullptr;
  try {
    escape(&p);
  } catch (int) {
    return *p;
  }
  return 0;
}
constexpr int a = read_unwound_frame();

constexpr int read_unwound_block() {
  int *p = nullptr;
  try {
    int x = 2;
    p = &x;
    throw 0;
  } catch (int) {
    return *p;
  }
}
constexpr int b = read_unwound_block();

struct T {
  int v = 3;
};

// T is trivially destructible: only the end of x's scope checks it.
constexpr void end_then_throw(bool fail) {
  T x;
  if (fail) {
    x.~T();
    throw 0;
  }
}

constexpr int destroy_ended() {
  try {
    end_then_throw(true);
  } catch (int) {
    return 1;
  }
  return 0;
}
constexpr int c = destroy_ended();

// A normal exit of the same function destroys a live object.
constexpr int normal() {
  end_then_throw(false);
  return 1;
}
static_assert(normal() == 1);

int main() {}
