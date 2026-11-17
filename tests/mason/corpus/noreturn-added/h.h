ATTR inline int f(int a) {
  if (a)
    throw a;
  throw 0;
}
