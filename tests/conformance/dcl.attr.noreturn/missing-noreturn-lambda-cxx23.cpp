// [dcl.attr.noreturn]/1: from C++23 clang 22 suggests noreturn for a
// lambda's call operator whose body is a single throw-expression, at the
// lambda's introducer (spec/sema.md §7.5.3; below C++23 it is inferred
// silently: missing-noreturn-lambda-cxx20.cpp).
//@ expect: compile-ok
//@ strict-diags
//@ args: -std=c++23 -Wall -Wmissing-noreturn
//@ requires: clang>=22
//@ diag: 13:10 warning "function 'operator()' could be declared with attribute 'noreturn'"
//@ diag: 16:16 warning "function 'operator()' could be declared with attribute 'noreturn'"
//@ diag: 17:12 warning "function 'operator()' could be declared with attribute 'noreturn'"
template <class F> int take(F f) { return f(1, 2) ? 1 : 0; }
auto a = [](int, int) -> bool { throw 0; };
int f() { a(1, 2); }
int main() {
  int r = take([](int, int) -> bool { throw 0; });
  auto g = [](auto x) { throw x; };
  (void)g;
  return r;
}
