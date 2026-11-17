// a noreturn carried only by a typedef'd function type is not hashed
typedef void nr_t(int) __attribute__((noreturn));
nr_t f;
inline int g(int a) {
  if (a)
    f(a);
  return 1;
}
int (*keep_a)(int) = g;
