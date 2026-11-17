// [dcl.constexpr]/3 before C++23 (P2647R1 lifted it): a constexpr
// function may not define a variable of static or thread storage
// duration; clang accepts it as a C++23 extension per variable, and a
// lambda whose body holds one, or a label or `goto`, is not implicitly
// constexpr ([expr.prim.lambda.closure]/5), so calling it in a constant
// expression is an error.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++20
//@ strict-diags
//@ diag: 22:16 warning "definition of a static variable in a constexpr function is a C++23 extension [-Wc++23-extensions]"
//@ diag: 35:24 warning "definition of a thread_local variable in a constexpr constructor is a C++23 extension [-Wc++23-extensions]"
//@ diag: 41:15 error "constexpr variable 'jumps' must be initialized by a constant expression"
//@ diag: 41:23 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 41:23 note "declared here"
//@ diag: 47:15 error "constexpr variable 'holds_static' must be initialized by a constant expression"
//@ diag: 47:30 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 47:30 note "declared here"

constexpr int counter(bool b) {
  if (b) {
    static int calls = 0;
    return ++calls;
  }
  return 0;
}

// A block-scope extern declaration defines nothing.
constexpr bool declares() { extern thread_local int t; extern int u; return true; }
static_assert(declares(), "");

struct Widget {
  constexpr Widget(bool b) {
    if (b) {
      thread_local int seen = 0;
      ++seen;
    }
  }
};

constexpr int jumps = [] {
  goto done;
done:
  return 1;
}();

constexpr int holds_static = [] {
  if (false) {
    static int unused;
  }
  return 2;
}();

constexpr int plain = [] { return 3; }();
