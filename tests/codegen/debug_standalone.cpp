// Debug info (WP-814): [[clang::standalone_debug]] keeps a class's full definition where
// -fno-standalone-debug would omit it (key function elsewhere, constructor homing).
//@ args: -g
struct [[clang::standalone_debug]] Dyn {
  virtual ~Dyn();
  virtual int f();
  int d = 1;
};
struct Plain {
  virtual ~Plain();
  int p = 2;
};
struct [[clang::standalone_debug]] Homed {
  Homed(int);
  int h;
};
struct Unhomed {
  Unhomed(int);
  int u;
};
struct Fwd;
struct [[clang::standalone_debug]] Fwd {
  int x;
};
template <class T> struct [[clang::standalone_debug]] Tmpl {
  Tmpl(T);
  T t;
};
int use(Dyn *a, Plain *b, Homed *c, Unhomed *d, Fwd *e, Tmpl<int> *f) {
  return a->d + b->p + c->h + d->u + e->x + f->t;
}
