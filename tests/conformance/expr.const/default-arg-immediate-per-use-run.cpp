// CWG 2631, [expr.const]/16, [dcl.fct.default]/5: an immediate invocation
// in a default argument is evaluated at each use, in the context of that
// use: `len(__builtin_FUNCTION())` sees the caller's name. A
// source-location builtin written directly in the invocation keeps its own
// line (clang 19 and 22); `std::source_location::current()` in it is the
// use's whole location. Nested default arguments are evaluated in the
// outermost use's context. Reduced from real code.
//@ expect: run
//@ stdout: log 1 | logs 8 | line 20 | sline 42 | nest 1
//@ stdout: log 2 | logs 9 | line 20 | sline 47 | nest 2 | outer 2 2
//@ stdout: tlog 3 3 | inner 3 | user 29 30
//@ stdout: pick l
#include <source_location>
extern "C" int printf(const char *, ...);
consteval unsigned len(const char *s) { unsigned n = 0; while (s[n]) ++n; return n; }
consteval unsigned line(unsigned l) { return l; }
consteval unsigned sline(std::source_location s) { return s.line(); }
unsigned log(unsigned n = len(__builtin_FUNCTION())) { return n; }
unsigned logs(unsigned n = len(std::source_location::current().function_name())) { return n; }
unsigned logl(unsigned n = line(__builtin_LINE())) { return n; }
unsigned logsl(unsigned n = sline(std::source_location::current())) { return n; }
unsigned inner(unsigned n = len(__builtin_FUNCTION())) { return n; }
unsigned nest(unsigned n = inner()) { return n; }
unsigned outer(unsigned a = log(), unsigned b = len(__builtin_FUNCTION())) {
  printf(" | outer %u %u", a, b);
  return a;
}
template <class T> unsigned tlog(T, unsigned n = len(__builtin_FUNCTION())) { return n; }
template <class T> unsigned tpretty(T, unsigned n = len(std::source_location::current().function_name())) { return n; }
template <class T> unsigned user() { return tpretty(T{}); }
template <class T> struct W {
  static unsigned m(unsigned n = inner()) { return n; }
};
// The pretty name at the use is "void legacy::f()": its sixth byte is
// 'l'. In the declaration's own context (no function) the name is empty
// and the read is out of bounds, so the declaration is not diagnosed.
consteval char pick(const char *s) { return s[5]; }
void say(char c = pick(std::source_location::current().function_name())) { printf("pick %c\n", c); }
namespace legacy { void f() { say(); } }
void f() {
  printf("log %u | logs %u | line %u", log(), logs(), logl());
  printf(" | sline %u", logsl());
  printf(" | nest %u\n", nest());
}
void gh() {
  printf("log %u | logs %u | line %u", log(), logs(), logl());
  printf(" | sline %u", logsl());
  printf(" | nest %u", nest());
  outer();
  printf("\n");
}
void ijk() {
  printf("tlog %u %u", tlog(1), tlog(1.0));
  printf(" | inner %u", W<int>::m());
  unsigned u1 = user<int>();
  unsigned u2 = user<char>();
  printf(" | user %u %u\n", u1, u2);
}
int main() {
  f();
  gh();
  ijk();
  legacy::f();
}
