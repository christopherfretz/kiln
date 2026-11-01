// [dcl.attr.fallthrough] (WP-234): before C++17 the -Wimplicit-fallthrough
// note prefers a macro spelled '[[clang::fallthrough]]' over one spelled
// '[[fallthrough]]' (clang getFallthroughAttrSpelling's PreferClangAttr).
//@ std: c++14
//@ args: -Wimplicit-fallthrough -DCMD_FT=[[fallthrough]]
//@ strict-diags
//@ diag: 41:3 warning "unannotated fall-through between switch labels"
//@ diag: 41:3 note "insert 'CMD_FT;' to silence this warning"
//@ diag: 41:3 note "insert 'break;' to avoid fall-through"
//@ diag: 49:3 warning "unannotated fall-through between switch labels"
//@ diag: 49:3 note "insert 'HEADER_FT;' to silence this warning"
//@ diag: 49:3 note "insert 'break;' to avoid fall-through"
//@ diag: 58:3 warning "unannotated fall-through between switch labels"
//@ diag: 58:3 note "insert 'FT2;' to silence this warning"
//@ diag: 58:3 note "insert 'break;' to avoid fall-through"
//@ diag: 67:3 warning "unannotated fall-through between switch labels"
//@ diag: 67:3 note "insert 'CFT;' to silence this warning"
//@ diag: 67:3 note "insert 'break;' to avoid fall-through"
//@ diag: 77:5 warning "unannotated fall-through between switch labels"
//@ diag: 77:5 note "insert 'CFT;' to silence this warning"
//@ diag: 77:5 note "insert 'break;' to avoid fall-through"
//@ diag: 83:3 warning "unannotated fall-through between switch labels"
//@ diag: 83:3 note "insert 'CFT;' to silence this warning"
//@ diag: 83:3 note "insert 'break;' to avoid fall-through"
//@ diag: 94:3 warning "unannotated fall-through between switch labels"
//@ diag: 94:3 note "insert 'LATE;' to silence this warning"
//@ diag: 94:3 note "insert 'break;' to avoid fall-through"
//@ diag: 103:3 warning "unannotated fall-through between switch labels"
//@ diag: 103:3 note "insert 'CMD_FT;' to silence this warning"
//@ diag: 103:3 note "insert 'break;' to avoid fall-through"
//@ diag: 111:3 warning "unannotated fall-through between switch labels"
//@ diag: 111:3 note "insert 'LATE;' to silence this warning"
//@ diag: 111:3 note "insert 'break;' to avoid fall-through"
//@ diag: 118:3 warning "unannotated fall-through between switch labels"
//@ diag: 124:10 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 118:3 note "insert 'LATE;' to silence this warning"
//@ diag: 118:3 note "insert 'break;' to avoid fall-through"
int cmd(int n) {
  switch (n) {
  case 0: n++;
  case 1: return 1;
  }
  return 0;
}
#include "Inputs/fallthrough-macro.h"
int a(int n) {
  switch (n) {
  case 0: n++;
  case 1: return 1;
  }
  return 0;
}
#define FT1 [[fallthrough]]
#define FT2 [[ fallthrough ]]
int b(int n) {
  switch (n) {
  case 0: n++;
  case 1: return 1;
  }
  return 0;
}
#undef FT2
#define CFT [[clang::fallthrough]]
int c(int n) {
  switch (n) {
  case 0: n++;
  case 1: return 1;
  }
  return 0;
}
#undef FT1
#undef HEADER_FT
int d(int n) {
  auto l = [](int m) {
    switch (m) {
    case 0: m++;
    case 1: return 1;
    }
    return 0;
  };
  switch (n) {
  case 0: n++;
  case 1: return l(n);
  }
  return 0;
}
#undef CFT
#define FN() [[fallthrough]]
#define LIT [[deprecated("x")]]
int e(int n) {
  switch (n) {
  case 0: n++;
#define LATE [[fallthrough]]
  case 1: return 1;
  }
  return 0;
}
#pragma push_macro("LATE")
#undef LATE
int f(int n) {
  switch (n) {
  case 0: n++;
  case 1: return 1;
  }
  return 0;
}
#pragma pop_macro("LATE")
int g(int n) {
  switch (n) {
  case 0: n++;
  case 1: return 1;
  }
  return 0;
}
template <class T> int h(T n) {
  switch (n) {
  case 0: n++;
  case 1: return 1;
  }
  return 0;
}
#undef LATE
#define TL [[fallthrough]]
int hi = h(1);
