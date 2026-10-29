struct S {
  int tag;
  union { int i; float f; char c[6]; };
  char after;
};
