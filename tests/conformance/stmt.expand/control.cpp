// [stmt.expand]/2 and [stmt.jump]: `break` in an expansion statement's
// body leaves the whole statement and `continue` ends the current element
// (jumps to the end of its compound statement). A loop in the body keeps
// its own break/continue; `return` leaves the function. Checked at compile
// time and at run time.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ stdout: 4 6 2 1
extern "C" int printf(const char*, ...);

constexpr int skip_and_stop() {
  int s = 0;
  template for (constexpr int x : {1, 2, 3, 4}) {
    if (x == 2) continue;
    if (x == 4) break;
    s += x;
  }
  return s;
}
static_assert(skip_and_stop() == 4);

constexpr int inner_loop() {
  int s = 0;
  template for (auto x : {1, 2, 3}) {
    for (int i = 0;; ++i) {
      if (i == x) break;
      ++s;
    }
  }
  return s;
}
static_assert(inner_loop() == 6);

constexpr int early_return() {
  template for (auto x : {1, 2, 3}) {
    if (x == 2) return x;
  }
  return 0;
}
static_assert(early_return() == 2);

// `break` inside a switch in the body leaves the switch only.
constexpr int in_switch() {
  int s = 0;
  template for (auto x : {0, 1}) {
    switch (x) {
      case 0: break;
      default: s += 1; break;
    }
  }
  return s;
}
static_assert(in_switch() == 1);

// A `return` in the body of a function with a deduced return type deduces
// it from the expansions ([dcl.spec.auto.general]/12 applies to the
// enclosing template, not to the statement's own level).
constexpr auto deduced() {
  template for (constexpr auto x : {1, 2}) {
    if constexpr (x == 2) return x;
  }
  return 0;
}
static_assert(deduced() == 2);
auto deduced_void(int a) {
  template for (auto x : {1, 2}) {
    if (x == a) return;
  }
}
void (*deduced_void_ptr)(int) = deduced_void;

int main(int argc, char**) {
  int a = 0, b = 0;
  template for (auto x : {1, 2, 3, 4}) {
    if (x == argc + 1) continue;
    if (x == 4) break;
    a += x;
  }
  template for (auto x : {1, 2, 3}) {
    for (int i = 0;; ++i) {
      if (i == x) break;
      ++b;
    }
  }
  int r = 0;
  [&] {
    template for (auto x : {1, 2, 3}) {
      if (x == argc + 1) { r = x; return; }
    }
  }();
  printf("%d %d %d %d\n", a, b, r, argc);
  return 0;
}
