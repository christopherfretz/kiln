// one inline function, used from each unit
inline int f(int a) {
  int s = 0;
  for (int i = 0; i < a; ++i)
    s += i * a;
  return s;
}
