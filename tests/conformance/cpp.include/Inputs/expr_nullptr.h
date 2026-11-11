#ifndef EXPR_NULLPTR_H
#define EXPR_NULLPTR_H
typedef decltype(nullptr) nullptr_t;
inline nullptr_t np_make() { return nullptr; }
inline bool np_is_null(nullptr_t p) { return p == nullptr; }
struct NpHolder {
  nullptr_t p;
  int *get() const { return p; }
};
#endif
