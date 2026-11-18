// Module of thread-local-vars-run.cpp: namespace-scope thread_local
// variables, constant (const, constexpr, a const class object with a
// destructor) and not, constant- and dynamically initialized.
export module tlv;
int trace(int v) { return v; }
export thread_local const int c1 = 5;
export constexpr thread_local int c2 = 6;
export thread_local int m1 = 7;
export thread_local int d1 = trace(8);
export thread_local const int dc = trace(9);
struct D {
  int v;
  ~D() {}
};
export thread_local const D cd{10};
export const int *module_c1() { return &c1; }
export int *module_d1() { return &d1; }
