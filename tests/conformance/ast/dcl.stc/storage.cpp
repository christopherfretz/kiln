//@ expect: ast
static int file_static = 1;
extern int declared;
inline int inl = 2;
extern int defined_extern = 3;
int counter() {
  static int calls;
  return ++calls + inl;
}
