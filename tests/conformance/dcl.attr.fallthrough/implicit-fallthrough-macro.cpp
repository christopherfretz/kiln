// [dcl.attr.fallthrough] (WP-234): the -Wimplicit-fallthrough note names
// the latest object-like macro in effect at the label whose replacement
// list is '[[fallthrough]]' (from C++17 preferred over one spelled
// '[[clang::fallthrough]]'), as clang's getFallthroughAttrSpelling does:
// macros from the command line and from headers count, whitespace in the
// replacement list does not, #undef / #pragma push_macro / pop_macro end
// or restore a definition, and a function-like macro does not qualify.
//@ args: -Wimplicit-fallthrough -DCMD_FT=[[fallthrough]]
//@ strict-diags
//@ diag: 44:3 warning "unannotated fall-through between switch labels"
//@ diag: 44:3 note "insert 'CMD_FT;' to silence this warning"
//@ diag: 44:3 note "insert 'break;' to avoid fall-through"
//@ diag: 52:3 warning "unannotated fall-through between switch labels"
//@ diag: 52:3 note "insert 'HEADER_FT;' to silence this warning"
//@ diag: 52:3 note "insert 'break;' to avoid fall-through"
//@ diag: 61:3 warning "unannotated fall-through between switch labels"
//@ diag: 61:3 note "insert 'FT2;' to silence this warning"
//@ diag: 61:3 note "insert 'break;' to avoid fall-through"
//@ diag: 70:3 warning "unannotated fall-through between switch labels"
//@ diag: 70:3 note "insert 'FT1;' to silence this warning"
//@ diag: 70:3 note "insert 'break;' to avoid fall-through"
//@ diag: 80:5 warning "unannotated fall-through between switch labels"
//@ diag: 80:5 note "insert 'CMD_FT;' to silence this warning"
//@ diag: 80:5 note "insert 'break;' to avoid fall-through"
//@ diag: 86:3 warning "unannotated fall-through between switch labels"
//@ diag: 86:3 note "insert 'CMD_FT;' to silence this warning"
//@ diag: 86:3 note "insert 'break;' to avoid fall-through"
//@ diag: 97:3 warning "unannotated fall-through between switch labels"
//@ diag: 97:3 note "insert 'LATE;' to silence this warning"
//@ diag: 97:3 note "insert 'break;' to avoid fall-through"
//@ diag: 106:3 warning "unannotated fall-through between switch labels"
//@ diag: 106:3 note "insert 'CMD_FT;' to silence this warning"
//@ diag: 106:3 note "insert 'break;' to avoid fall-through"
//@ diag: 114:3 warning "unannotated fall-through between switch labels"
//@ diag: 114:3 note "insert 'LATE;' to silence this warning"
//@ diag: 114:3 note "insert 'break;' to avoid fall-through"
//@ diag: 121:3 warning "unannotated fall-through between switch labels"
//@ diag: 127:10 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 121:3 note "insert 'LATE;' to silence this warning"
//@ diag: 121:3 note "insert 'break;' to avoid fall-through"
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
