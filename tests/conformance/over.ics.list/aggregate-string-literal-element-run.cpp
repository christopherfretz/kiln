// [over.ics.list]/8, [dcl.init.string]: a braced list converts to an
// aggregate whose first element is a character array when its string
// literal initializes that array: a literal of a compatible character type
// that fits. Overload resolution picks the aggregate that accepts it.
//@ expect: run
//@ std: c++14
//@ stdout: 1 2 1
#include <cstdio>

struct A {
  char str[4] = "foo";
  char rows[2][4] = {"bar", "baz"};
};
struct B {
  char16_t str[10];
};

int pick(A) { return 1; }
int pick(B) { return 2; }

int main() { std::printf("%d %d %d\n", pick({"foo"}), pick({u"foo"}), pick({"ab"})); }
