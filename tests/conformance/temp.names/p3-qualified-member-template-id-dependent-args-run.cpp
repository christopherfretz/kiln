// [temp.names]/3, [basic.lookup.qual]/1: in `this->Base::getObj<T>()`
// inside a class template, `getObj` is looked up in `Base` and names its
// member template, even though the derived class declares a non-template
// `getObj` of its own; the qualifier is kept when the access is formed
// again at instantiation. Reduced from lld's ELF/InputFiles.h:
// `ObjFile<ELFT>::getObj() { return this->ELFFileBase::getObj<ELFT>(); }`.
//@ expect: run
//@ stdout: 4 8 3
#include <cstdio>

struct Base {
  int v = 4;
  template <typename T> T getObj() const { return T(v); }
  template <typename T, typename U> T pair(U u) const { return T(v) + T(u); }
};

template <class T> struct Obj : Base {
  T getObj() const { return this->Base::getObj<T>(); }
  T twice() const { return this->getObj() + Base::getObj<T>(); }
  int pair(int) const { return -1; }
  T both() const { return this->Base::pair<T, short>(short(-1)); }
};

int main() {
  Obj<long> o;
  std::printf("%ld %ld %ld\n", o.getObj(), o.twice(), o.both());
}
