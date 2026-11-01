// [except.ctor]/3, [basic.start.term]/1: a global array initialized from
// a braced list destroys its constructed elements if a later element's
// construction throws; once complete, a __cxx_global_array_dtor helper
// destroys it at exit. (aarch64: clang 19 runs loop canonicalization on
// functions without optnone; spec/codegen.md "Deviations".)
struct A { A(int); ~A(); int x; };
A ga[2] = {A(1), A(2)};
A gb[2][2] = {{1, 2}, {3, 4}};
struct B { B(); ~B(); int x; };
B gc[3];
