// Local aggregate initialization: constant (memcpy from a private global
// or memset) and non-constant (element-wise).
struct Q { int a; double b; char c[4]; };
int use(Q*);
int agg(int x) {
  Q k = {1, 2.0, "ab"};
  Q z = {};
  Q v = {x, x * 0.5, {(char)x}};
  Q arr[2] = {{1, 1.0, "x"}, {x, 0, ""}};
  return use(&k) + use(&z) + use(&v) + use(arr);
}
