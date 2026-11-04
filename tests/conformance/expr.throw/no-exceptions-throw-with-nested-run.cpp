// [expr.throw], [except.nested]: with -fno-exceptions an instantiated
// `std::throw_with_nested` (a throw-expression in a system header) is
// accepted and lowered to the ordinary `__cxa_allocate_exception` /
// `__cxa_throw` sequence without landing pads, as clang emits it; the
// program compiles, links and runs (the call is guarded by a condition
// that never holds, since the throw would terminate). spec/sema.md
// "Exceptions disabled", WP-607.
//@ expect: run
//@ args: -fno-exceptions
//@ stdout: ok
#include <exception>

extern "C" int puts(const char *);

struct E {
  int v;
};

int main(int argc, char **) {
  if (argc > 1000)
    std::throw_with_nested(E{argc});
  puts("ok");
}
