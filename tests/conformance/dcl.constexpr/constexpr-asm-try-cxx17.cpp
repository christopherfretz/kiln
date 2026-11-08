// [dcl.constexpr]/3 (C++17): an asm-declaration or a try-block in the body
// of a constexpr function or constructor, and a function-try-block, became
// valid in C++20; clang accepts them in C++17 as C++20 extensions, once per
// function (WP-890, WP-891; spec/sema.md). Verified against clang 19 and 22.
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:5 warning "use of this statement in a constexpr function is a C++20 extension"
//@ diag: 19:3 warning "use of this statement in a constexpr function is a C++20 extension"
//@ diag: 24:25 warning "function try block in constexpr function is a C++20 extension"
//@ diag: 32:7 warning "use of this statement in a constexpr constructor is a C++20 extension"

constexpr int f1(int x) {
  if (x < 0)
    asm("");
  return x;
}
constexpr int f2(int x) {
  try {
  } catch (...) {
  }
  return x;
}
constexpr int f3(int x) try {
  return x;
} catch (...) {
  return 0;
}
struct S {
  constexpr S(int x) {
    if (x < 0)
      asm("");
  }
};
static_assert(f1(1) == 1, "");
static_assert(f2(2) == 2, "");
static_assert(f3(3) == 3, "");
constexpr S s(1);
