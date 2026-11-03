// WP-419: `__builtin_classify_type` folds to its GCC type class at every
// use and never evaluates its operand (the increment, the call and the
// volatile read are dropped), as clang 19 and 22 emit it.
struct S { int m; void f(); };
union U { int i; };
int g();
volatile int vi;
int classify(double d, int *p, S s, U u) {
  int k = 0;
  int r = __builtin_classify_type(k++) + __builtin_classify_type(g());
  r += __builtin_classify_type(vi) + __builtin_classify_type(d);
  r += __builtin_classify_type(p) + __builtin_classify_type(&S::m);
  r += __builtin_classify_type(&S::f) + __builtin_classify_type(s);
  r += __builtin_classify_type(u) + __builtin_classify_type(nullptr);
  __builtin_classify_type(g());
  return r + k;
}
template <class T> int classify_t(T t) { return __builtin_classify_type(t); }
int use() { return classify_t(1.0f) + classify_t(true) + classify_t("x"); }
