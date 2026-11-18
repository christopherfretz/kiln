// Module of module_init_const_inst_member.cpp: constant-initialized static
// data members explicitly instantiated with their class (E) and alone (F).
export module mod_init_const_inst;
export template <class T> struct E { static int y; };
template <class T> int E<T>::y = 2;
template struct E<int>;
export template <class T> struct F { static int y; };
template <class T> int F<T>::y = 3;
template int F<int>::y;
