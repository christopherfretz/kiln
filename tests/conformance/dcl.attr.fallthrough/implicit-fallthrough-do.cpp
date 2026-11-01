// [dcl.attr.fallthrough] (WP-234): a case label that starts a 'do' body
// is entered both from the statement before the 'do' and from the
// condition when that is reached and not constant false (clang's loop-back
// block), so it can warn even after a 'break'; a [[fallthrough]] before
// the 'do' directly precedes that label. Labels nested in branches and
// loops follow clang's CFG predecessors.
//@ args: -Wimplicit-fallthrough
//@ strict-diags
//@ diag: 96:5 warning "unannotated fall-through between switch labels"
//@ diag: 96:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 96:5 note "insert 'break;' to avoid fall-through"
//@ diag: 104:5 warning "unannotated fall-through between switch labels"
//@ diag: 104:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 104:5 note "insert 'break;' to avoid fall-through"
//@ diag: 114:7 warning "unannotated fall-through between switch labels"
//@ diag: 114:7 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 114:7 note "insert 'break;' to avoid fall-through"
//@ diag: 126:9 warning "unannotated fall-through between switch labels"
//@ diag: 126:9 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 126:9 note "insert 'break;' to avoid fall-through"
//@ diag: 135:5 warning "unannotated fall-through between switch labels"
//@ diag: 135:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 135:5 note "insert 'break;' to avoid fall-through"
//@ diag: 144:5 warning "unannotated fall-through between switch labels"
//@ diag: 144:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 144:5 note "insert 'break;' to avoid fall-through"
//@ diag: 152:5 warning "unannotated fall-through between switch labels"
//@ diag: 152:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 152:5 note "insert 'break;' to avoid fall-through"
//@ diag: 161:7 warning "unannotated fall-through between switch labels"
//@ diag: 161:7 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 161:7 note "insert 'break;' to avoid fall-through"
//@ diag: 169:5 warning "unannotated fall-through between switch labels"
//@ diag: 169:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 169:5 note "insert 'break;' to avoid fall-through"
//@ diag: 177:5 warning "unannotated fall-through between switch labels"
//@ diag: 177:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 177:5 note "insert 'break;' to avoid fall-through"
//@ diag: 188:9 warning "unannotated fall-through between switch labels"
//@ diag: 188:9 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 188:9 note "insert 'break;' to avoid fall-through"
//@ diag: 206:5 warning "unannotated fall-through between switch labels"
//@ diag: 206:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 206:5 note "insert 'break;' to avoid fall-through"
//@ diag: 223:5 warning "unannotated fall-through between switch labels"
//@ diag: 223:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 223:5 note "insert 'break;' to avoid fall-through"
//@ diag: 245:5 warning "unannotated fall-through between switch labels"
//@ diag: 245:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 245:5 note "insert 'break;' to avoid fall-through"
//@ diag: 265:5 warning "unannotated fall-through between switch labels"
//@ diag: 272:5 warning "unannotated fall-through between switch labels"
//@ diag: 272:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 272:5 note "insert 'break;' to avoid fall-through"
//@ diag: 281:7 warning "unannotated fall-through between switch labels"
//@ diag: 281:7 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 281:7 note "insert 'break;' to avoid fall-through"
//@ diag: 291:5 warning "unannotated fall-through between switch labels"
//@ diag: 291:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 291:5 note "insert 'break;' to avoid fall-through"
//@ diag: 301:5 warning "unannotated fall-through between switch labels"
//@ diag: 306:15 note "in instantiation of function template specialization 'd12<1>' requested here"
//@ diag: 301:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 301:5 note "insert 'break;' to avoid fall-through"
//@ diag: 310:14 warning "unannotated fall-through between switch labels"
//@ diag: 310:14 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 310:14 note "insert 'break;' to avoid fall-through"
//@ diag: 311:10 warning "unannotated fall-through between switch labels"
//@ diag: 311:10 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 311:10 note "insert 'break;' to avoid fall-through"
//@ diag: 317:17 warning "unannotated fall-through between switch labels"
//@ diag: 317:17 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 317:17 note "insert 'break;' to avoid fall-through"
//@ diag: 332:7 warning "unannotated fall-through between switch labels"
//@ diag: 332:7 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 332:7 note "insert 'break;' to avoid fall-through"
//@ diag: 341:18 warning "unannotated fall-through between switch labels"
//@ diag: 341:18 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 341:18 note "insert 'break;' to avoid fall-through"
//@ diag: 349:5 warning "unannotated fall-through between switch labels"
//@ diag: 349:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 349:5 note "insert 'break;' to avoid fall-through"
//@ diag: 359:5 warning "unannotated fall-through between switch labels"
//@ diag: 359:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 359:5 note "insert 'break;' to avoid fall-through"
//@ diag: 372:9 warning "unannotated fall-through between switch labels"
//@ diag: 372:9 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 372:9 note "insert 'break;' to avoid fall-through"
void f();
bool c();
int a[3];
void t1(int n) {
  switch (n) {
  case 0: f();
    do {
    case 1: f();
    } while (c());
  }
}
void t2(int n) {
  switch (n) {
  case 0:
    do {
    case 1: f();
    } while (c());
  }
}
void t4(int n) {
  switch (n) {
  case 0: f();
    if (c()) {
      f();
      if (c()) {
      case 1: f();
      }
    }
  }
}
void t5(int n) {
  switch (n) {
  case 0: f();
    if (c()) {
      f();
    } else {
      f();
      { case 1: f(); }
    }
  }
}
void t6(int n) {
  switch (n) {
  case 0: f();
    while (c()) {
      f();
    case 1: f();
    }
  }
}
void t7(int n) {
  switch (n) {
  case 0: f();
    do {
      f();
    case 1: f();
    } while (c());
  }
}
void t8(int n) {
  switch (n) {
  case 0: f(); break;
    do
    case 1: f();
    while (c());
  }
}
void t10(int n) {
  switch (n) {
  case 0: f();
    if (c()) {
      if (c()) f();
      case 1: f();
    }
  }
}
void t11(int n) {
  switch (n) {
  case 0: break;
    do {
    case 1: f();
    } while (c());
  }
}
void t12(int n) {
  switch (n) {
  case 0: f();
    for (;;) {
    case 1: f();
      if (c()) break;
    }
  }
}
void t14(int n) {
  switch (n) {
  case 0: f();
    {
      while (c()) {
        if (c()) {
        case 1: f();
        }
      }
    }
  }
}
void d1(int n) {
  switch (n) {
  case 0: break;
    do {
    case 1: f();
    } while (0);
  }
}
void d2(int n) {
  switch (n) {
  case 0: break;
    do {
    case 1: f();
    } while (1);
  }
}
void d3(int n) {
  switch (n) {
  case 0: break;
    do {
    case 1: f();
      return;
    } while (c());
  }
}
void d4(int n) {
  switch (n) {
  case 0: break;
    do {
    case 1: f();
      if (c()) continue;
      return;
    } while (c());
  }
}
void d5(int n) {
  switch (n) {
  case 0: break;
    do {
      ;
      {
      case 1: f();
      }
      return;
    } while (c());
  }
}
void d6(int n) {
  switch (n) {
  case 0: f();
    do {
    case 1: f();
      return;
    } while (c());
  }
}
void d7(int n) {
  switch (n) {
  case 0: f();
    [[fallthrough]];
    do {
    case 1: f();
      return;
    } while (c());
  }
}
void d8(int n) {
  switch (n) {
  case 0: f();
    [[fallthrough]];
    do {
    case 1: f();
    } while (c());
  }
}
void d9(int n) {
  switch (n) {
    do {
    case 1: f();
    } while (c());
  }
}
void d10(int n) {
  switch (n) {
  case 0: f(); break;
    while (c()) {
      do {
      case 1: f();
        break;
      } while (c());
    }
  }
}
void d11(int n) {
  switch (n) {
  case 0: break;
    do {
    case 1: f();
      [[fallthrough]];
    case 2: f();
    } while (c());
  }
}
template <int N> void d12(int n) {
  switch (n) {
  case 0: break;
    do {
    case 1: f();
    } while (N);
  }
}
template void d12<0>(int);
template void d12<1>(int);
void e1(int n) {
  switch (n) {
  case 0: f();
    if (c()) case 1: f();
    else case 2: f();
  }
}
void e2(int n) {
  switch (n) {
  case 0: f();
    while (c()) case 1: f();
  }
}
void e3(int n) {
  switch (n) {
  case 0: f();
    [[fallthrough]];
    do case 1: f(); while (0);
  }
}
void e4(int n) {
  switch (n) {
  case 0: f();
    do {
      do {
      case 1: f();
      } while (c());
    } while (c());
  }
}
void e5(int n) {
  switch (n) {
  case 0: f();
    do {
      [[likely]] case 1: f();
    } while (c());
  }
}
void e6(int n) {
  switch (n) {
  case 0: f();
    do {
    case 1:
    case 2: f();
    } while (c());
  }
}
void e7(int n) {
  switch (n) {
  case 0: return;
    do {
      ;
    case 1: f();
      if (c()) break;
    } while (true);
  }
}
void e8(int n) {
  switch (n) {
  case 0: f();
    for (;;) {
      if (c()) {
        f();
      } else {
        do {
        case 1: f();
        } while (c());
      }
      return;
    }
  }
}
void e9(int n) {
  switch (n) {
  case 0:
    f();
    [[fallthrough]];
    {
      ;
    }
    do {
      {}
      case 1: f();
      return;
    } while (c());
  }
}
