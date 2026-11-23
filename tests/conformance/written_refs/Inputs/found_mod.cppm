export module found_mod;
export namespace fm {
void f(int);
void f(long);
template <class T> void call(T t) { f(t); }
template <class T> struct S {
  void m(T);
  void m(T, int);
  void use(T t) { this->m(t); }
};
} // namespace fm
