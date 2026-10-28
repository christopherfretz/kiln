//@ expect: ast
union U {
  int i;
  float f;
  char bytes[4];
};
int pun(U u) {
  U v = {7};
  v.f = 1.0f;
  return u.i + v.bytes[0];
}
