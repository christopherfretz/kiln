// GNU __builtin_object_size / __builtin_dynamic_object_size (clang
// emitBuiltinObjectSize at -O0): a pointer whose object is not known to
// the frontend lowers to `llvm.objectsize(ptr, min, nullunknown=true,
// dynamic)` with min = type & 2; type 3 of an unknown object folds to 0.
unsigned long os(char *p, int *q) {
  return __builtin_object_size(p, 0) + __builtin_object_size(q, 3) + __builtin_dynamic_object_size(p, 1);
}
unsigned long os2(void *p) { return __builtin_object_size(p, 2) + __builtin_dynamic_object_size(p, 3); }
