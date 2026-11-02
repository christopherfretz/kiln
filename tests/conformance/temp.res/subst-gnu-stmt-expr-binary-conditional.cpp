// [temp.res.general], [temp.inst] with two GNU extensions in a function
// template: the binary conditional `x ?: y` (its common operand evaluated
// once) and the statement expression `({ ... })`, whose value is that of
// its last expression statement (also under a label, nested, `void`).
// Both are substituted and built again at instantiation. WP-301.
//@ args: -Wno-gnu-conditional-omitted-operand -Wno-gnu-statement-expression
//@ expect: run
//@ stdout: 5 3 1 | 8 9 1 0 7 | 2.5
#include <cstdio>

int calls = 0;
template <class T> T next(T& t) { ++calls; return t++; }

template <class T> T bc(T t, T u) { return t ?: u; }
template <class T> T bc_once(T t, T u) { return next(t) ?: u; }

template <class T> T se(T t) {
  return ({
    T u = t;
    u += 1;
    u * 2;
  });
}
template <class T> T se_label(T t) {
  T r = ({
    T u = t;
    goto done;
  done:
    u + 1;
  });
  return r;
}
template <class T> int se_nested(T t) {
  return ({ int a = ({ T b = t; b; }); a > 0; });
}
template <class T> int se_void(T t) {
  int seen = 0;
  ({ if (t) seen = 0; });
  return seen;
}
template <class T> T se_lvalue(T t) {
  T x = t;
  return ({ x; }) + ({ T& y = x; y; }) - t;
}

int main() {
  std::printf("%d %d %d | ", bc(5, 3), bc(0, 3), bc_once(0, 1) + calls - 1);
  std::printf("%d %d %d %d %d | ", se(3), se_label(8), se_nested(4), se_void(1),
              se_lvalue(7));
  std::printf("%.1f\n", bc(2.5, 1.0));
}
