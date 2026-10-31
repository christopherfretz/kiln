// [expr.new]/24, [dcl.init.general]/7: array new-expressions of a class
// with constructors run a constructor loop over the elements (constant
// and variable counts, after explicit initializers); the deallocation
// function is called if a constructor throws ([expr.new]/28).
struct K { K(); K(int); int v; };
struct T { int v; };
K* a() { return new K[2]; }
K* b(int n) { return new K[n]; }
K* c(int n) { return new K[n]{1, 2}; }
T* e(int n) { return new T[n]; }
K* f(unsigned char n) { return new K[n]; }
