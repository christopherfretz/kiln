// [dcl.fct]/13, [dcl.typedef]/1, [cstdarg.syn], [class.derived.general]/2,
// [temp.inst]/3: functions declared through a typedef, an alias template,
// decltype or __typeof, a va_list handed on, and a base and a member
// class of an implicit instantiation. With `--pch-diff`, the header's
// snapshot carries them (WP-1369).
//@ expect: run
//@ stdout: 8 9 12 10 10
#include "Inputs/typedef_fns.h"
extern "C" int printf(const char *, ...);

int tf::twice(int v) { return v * 2; }
int tf::thrice(int v) noexcept { return v * 3; }
int tf::Q::dec(int a) { return a - 1; }
int tf::base_fn(long v) { return int(v); }
int tf::dt_fn(long v) { return int(v) + 1; }
int tf::to_fn(long v) { return int(v) + 2; }
int tf::vsum(int n, va_list ap) {
  int s = 0;
  for (int i = 0; i < n; ++i)
    s += va_arg(ap, int);
  return s;
}
static_assert(noexcept(tf::thrice(1)) && !noexcept(tf::twice(1)));
static_assert(noexcept(tf::Box<int>::quad(1)));

int main() {
  tf::Q q;
  tf::Box<long> b;
  printf("%d %d %d %d %d\n", tf::twice(), tf::thrice(3), tf::sum(3, 2, 4, 6),
         q.inc(4) + q.dec(6), tf::use_holder());
  return b.half(8) == 4 && tf::Box<int>::quad(2) == 8 &&
                 tf::dt_fn(1) + tf::to_fn(1) + tf::base_fn(1) == 6
             ? 0
             : 1;
}
