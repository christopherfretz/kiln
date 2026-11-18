// Module of module_init_inst_member.cpp (the run tests use its twin,
// tests/conformance/basic.start.dynamic/Inputs/inst-member.cppm): static
// data members and a variable template specialization with dynamic initializers, created by
// an explicit instantiation of the class (E), of the member alone (F), an
// implicit instantiation in the module (G) and an explicit instantiation
// of the variable template (vt). They are the module's only dynamic
// initializers.
export module mod_init_inst;
int calls[4];
int bump(int i, int v) {
  ++calls[i];
  return v;
}
export int init_calls() { return calls[0] * 1000 + calls[1] * 100 + calls[2] * 10 + calls[3]; }
export template <class T> struct E { static int y; };
template <class T> int E<T>::y = bump(0, 7);
template struct E<int>;
export template <class T> struct F { static int y; };
template <class T> int F<T>::y = bump(1, 8);
template int F<int>::y;
export template <class T> struct G { static int y; };
template <class T> int G<T>::y = bump(2, 9);
export int g_value() { return G<int>::y; }
export template <class T> int vt = bump(3, 10);
template int vt<int>;
