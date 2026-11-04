// [temp.point]/7, [temp.inst]/7: the end of the translation unit is also a
// point of instantiation, so a variable template specialization usable in
// constant expressions that is odr-used only before its template's
// definition is defined from that definition there. clang (and GCC)
// instantiate such a specialization only at a reference and leave `c<char>`
// and `d<int>` undefined, a link error; kiln defines them (a deviation:
// spec/sema-templates.md §4.6, docs/status/deviations.md).
//@ expect: run
//@ skip(clang): clang leaves c<char> and d<int> undefined (a link error)
//@ skip(gcc): GCC leaves c<char> and d<int> undefined (a link error)
//@ stdout: 105 10
extern "C" int printf(const char*, ...);
int dyn() {
  static int n;
  return ++n * 10;
}

template <class T> extern const int c;
int gc() { return c<char>; }
template <class T> const int c = sizeof(T) + 104;

template <class T> extern const T d;
int gd() { return d<int>; }
template <class T> const T d = T(dyn());

int main() { printf("%d %d\n", gc(), gd()); }
