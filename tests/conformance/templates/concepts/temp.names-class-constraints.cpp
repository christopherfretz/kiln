//@ expect: compile-ok
// [temp.names]/8: a constrained class template's specialization requires its
// constraints to be satisfied.
template <class T> requires (sizeof(T) <= 8) struct Box { T v; };
Box<int> a;
Box<double> b;
template <class T> concept Ptr = __is_pointer(T);
template <Ptr T> struct Ref { T p; };
Ref<int *> r;
