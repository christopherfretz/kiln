// [temp.inst]/3, [over.match.best]: `pass_object_size` written only on a
// function template's definition makes it another template, not a
// redeclaration (the attribute is part of the parameter's identity in
// clang), and overload resolution prefers it: the call passes the
// object's size, which `__builtin_object_size` reads. Also the template
// of a class's member function. spec/sema.md §4.5 "Parameter
// attributes" rule 5, WP-3407.
//@ expect: run
//@ std: c++20
//@ stdout: 16 8
extern "C" int printf(const char *, ...);
template <class T> unsigned long size(T *const p);
template <class T> unsigned long size(T *const p __attribute__((pass_object_size(0)))) {
  return __builtin_object_size(p, 0);
}
struct M {
  template <class T> unsigned long size(T *const p);
  template <class T> unsigned long size(T *const p __attribute__((pass_object_size(0)))) {
    return __builtin_object_size(p, 0);
  }
};
int a[4];
long b;
int main() {
  unsigned long x = size(a);
  unsigned long y = M().size(&b);
  printf("%lu %lu\n", x, y);
}
