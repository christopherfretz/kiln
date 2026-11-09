// The module of image-class-meminit-run.cpp: a class template whose
// constructors have mem-initializers (spec/modules.md §11.11, "Class
// template patterns": the declaration image does not carry them, so
// the module falls back to replay).
export module imgcmi;
export template <class T> struct Cell {
  T x;
  T y;
  Cell(T t) : x(t), y(t + 1) {}
  Cell() : Cell(5) {}
};
export template <class T> struct Sub : Cell<T> {
  Sub(T t) : Cell<T>(t * 2) {}
};
