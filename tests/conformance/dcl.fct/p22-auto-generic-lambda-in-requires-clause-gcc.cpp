// [dcl.fct]/22, [temp.constr.atomic]: a generic lambda called in the
// trailing requires-clause (or a default argument) of an abbreviated
// function template; the constraint is satisfied for `int`, so the call
// is valid (found running GCC's C++ testsuite).
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19/22 find no viable call operator of a generic lambda in the declarator of an abbreviated function template (clang 22: nor a capture-free one in a default argument)
//@ stdout: 7 2.5 3.5
extern "C" int printf(const char*, ...);

int two(auto p) requires requires { { [](auto q) { return q; }(p) }; } { return p + 4; }

double three(auto p, int k = [](auto x) { return x; }(0)) { return p + k; }
double four(auto p, int k = [](int x) { return x; }(1)) { return p + k; }

int main() { printf("%d %g %g\n", two(3), three(2.5), four(2.5)); }
