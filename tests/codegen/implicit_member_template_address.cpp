// WP-416: an implicit member overload set (`f`, `&f`, `(f)`) in a member
// function selects a static member function template's specialization
// deduced from the target when no non-template matches ([over.over]/3).
struct S {
  void f(int);
  template <class T> static T f() { return T(); }
  template <class T> static void h(T) {}
  void h(char *);
  static long g(long);
  template <class T> static T g(T, T) { return T(); }
  void k();
};
long S::g(long v) { return v; }
void take(int (*)());
void S::k() {
  int (*a)() = f;
  long long (*b)() = &f;
  void (*c)(double) = h;
  void (*d)(short) = (h);
  long (*e)(long) = g;
  int (*i)(int, int) = &g;
  take(f);
  a(); b(); c(1); d(2); e(3); i(4, 5);
}
