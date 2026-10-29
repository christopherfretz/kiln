struct S {
  char k;
  struct { short a; char b; };
  union { struct { int x, y; }; long long xy; };
};
