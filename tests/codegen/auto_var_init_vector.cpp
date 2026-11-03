// -ftrivial-auto-var-init=pattern of a vector variable (spec/codegen.md §7y):
// a vector is not a scalar type, so clang fills it before any
// initializer, constant or not. Clang 19 spells the constants without
// `splat`.
//@ args: -ftrivial-auto-var-init=pattern
//@ min-oracle: 22
void use(void *);
typedef float v4f __attribute__((vector_size(16)));
typedef int v4i __attribute__((vector_size(16)));
struct V3 {
  v4f a, b, c;
};
void vectors(v4f x) {
  v4f y = x;
  v4f z = {1, 2, 3, 4};
  v4i w;
  V3 v;
  use(&y);
  use(&z);
  use(&w);
  use(&v);
}
