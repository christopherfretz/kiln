// [dcl.constinit]/1: the instantiated out-of-line definition of a static
// data member of a class template is the initializing declaration, and
// it carries the `constinit` its pattern was written with, so no
// "'constinit' specifier missing" warning is given.
//@ verify
// expected-no-diagnostics

template <class I> struct F {
  static constinit int flag;
};
template <class I> constinit int F<I>::flag{4};

int use() { return F<int>::flag; }
