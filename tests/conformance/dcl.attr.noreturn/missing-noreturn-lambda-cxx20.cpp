// [dcl.attr.noreturn]/1, [expr.prim.lambda.closure]/4: below C++23 clang 22
// infers noreturn for a lambda's call operator whose body is a single
// throw-expression without suggesting the attribute (clang 19 has no
// inferred form, and its graph form skips lambdas): no -Wmissing-noreturn
// at namespace or block scope, with an explicit return type, for a generic
// lambda or a lambda in a template (spec/sema.md §7.5.3; WP-679, range-v3
// chunk_by.cpp). The C++23 warning: missing-noreturn-lambda-cxx23.cpp.
//@ expect: compile-ok
//@ strict-diags
//@ args: -std=c++20 -Wall -Wmissing-noreturn
auto a = [](int, int) -> bool { throw 0; };
auto b = [] { throw 0; };
void use() { (void)a; (void)b; }
template <class F> int take(F f) { return f(1, 2) ? 1 : 0; }
int main() {
  int r = take([](int, int) -> bool { throw 0; });
  auto g = [](auto x) { throw x; };
  (void)g;
  return r;
}
template <class T> void tf() {
  auto l = [](T x) { throw x; };
  (void)l;
}
template void tf<int>();
