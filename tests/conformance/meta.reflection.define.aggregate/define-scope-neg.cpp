// [meta.reflection.define.aggregate]/7.3 (P2996R13; GCC 16.1's rule,
// spec/reflection.md §6.2): no function or class scope may enclose
// exactly one of the consteval block and the class's target scope. A
// lambda's body is a function scope.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 33:3 error "consteval block is not a constant expression"
//@ diag: 33:15 note "'f' intervenes between the scope of 'G' and the 'consteval' block"
//@ diag: 33:3 note "in call to"
//@ diag: 33:3 note "'consteval' block defined here"
//@ diag: 40:3 error "consteval block is not a constant expression"
//@ diag: 40:15 note "'Host' intervenes between the scope of 'In' and the 'consteval' block"
//@ diag: 40:3 note "in call to"
//@ diag: 40:3 note "'consteval' block defined here"
//@ diag: 45:5 error "consteval block is not a constant expression"
//@ diag: 45:17 note "a lambda intervenes between the scope of 'L' and the 'consteval' block"
//@ diag: 45:5 note "in call to"
//@ diag: 45:5 note "'consteval' block defined here"
//@ diag: 52:1 error "consteval block is not a constant expression"
//@ diag: 52:13 note "'H2' intervenes between the scope of 'M' and the 'consteval' block"
//@ diag: 52:1 note "in call to"
//@ diag: 52:1 note "'consteval' block defined here"
//@ diag: 55:3 error "consteval block is not a constant expression"
//@ diag: 55:15 note "'Wrap' intervenes between the scope of 'Ok' and the 'consteval' block"
//@ diag: 55:3 note "in call to"
//@ diag: 55:3 note "'consteval' block defined here"
#include "kiln_meta.h"
using namespace std::meta;
struct G;
consteval int f() {
  consteval { define_aggregate(^^G, {}); }
  return 0;
}
struct Out {
  struct In;
};
struct Host {
  consteval { define_aggregate(^^Out::In, {}); }
};
consteval int h() {
  struct L;
  auto lam = [] {
    consteval { define_aggregate(^^L, {}); }
  };
  return 0;
}
struct H2 {
  struct M;
};
consteval { define_aggregate(^^H2::M, {}); }
struct Ok;
struct Wrap {
  consteval { define_aggregate(^^Ok, {}); }
};
