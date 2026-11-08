// GNU `returns_twice` checks (WP-807; clang `ReturnsTwiceAttr`): a
// function subject (else ignored with a warning) and no arguments.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:28 warning "'returns_twice' attribute only applies to functions"
//@ diag: 12:24 warning "'returns_twice' attribute only applies to functions"
//@ diag: 13:27 error "'returns_twice' attribute takes no arguments"
//@ diag: 14:23 warning "'returns_twice' attribute only applies to functions"
//@ diag: 15:29 warning "'returns_twice' attribute only applies to functions"
//@ diag: 17:31 warning "'returns_twice' attribute only applies to functions"
int (*fp)() __attribute__((returns_twice));
int bad __attribute__((returns_twice));
int bad2() __attribute__((returns_twice(1)));
struct __attribute__((returns_twice)) C {};
void p(int q __attribute__((returns_twice)));
[[gnu::returns_twice]] int ok();
typedef int TD __attribute__((returns_twice));
