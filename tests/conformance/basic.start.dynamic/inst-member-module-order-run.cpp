// [basic.start.dynamic]/1: the initialization of a static data member or
// variable template specialization that is an instantiated specialization
// is unordered, so nothing orders it against this unit's ordered
// initializers; what is required is that it is complete before `main`
// (no initialization is deferred here, [basic.start.dynamic]/5) and
// happens once. `seen`, this unit's ordered initializer, may run before
// or after it and is not printed. Module `instm`'s only dynamic
// initializers are such members; this unit calls `_ZGIW5instm` all the
// same, as clang 22 does (WP-1669).
//@ expect: run
//@ std: c++20
//@ module: instm=Inputs/inst-member.cppm
//@ stdout: calls 1111 7 8 9 10
import instm;
extern "C" int printf(const char *, ...);

int seen = init_calls();

int main() {
  int at_entry = init_calls();
  printf("calls %d %d %d %d %d\n", at_entry, E<int>::y, F<int>::y, G<int>::y, vt<int>);
  return seen < 0;
}
