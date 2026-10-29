// [temp.inst]/2,/3: implicit instantiation of class templates, recorded by
// kiln's -fkiln-instantiation-report=json (spec/sema-templates.md §12).
// Each distinct specialization is instantiated once; a nested member class
// is instantiated with its enclosing specialization's arguments.
//@ inst-report: temp.inst-report.json
template <class T> struct Box { T value; };
template <class T, int N> struct Arr { T data[N]; };
template <class T> struct Outer {
  struct Inner { T t; };
  Box<T> b;
};

Box<int> b1;
Box<int> b2;
Box<char> b3;
Arr<long, 4> a;
Outer<short> o;
Outer<short>::Inner i;
static_assert(sizeof(Arr<long, 4>) == 4 * sizeof(long));
