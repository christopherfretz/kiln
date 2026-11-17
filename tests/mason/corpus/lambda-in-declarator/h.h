// a definition whose declarator holds a lambda gets no row (X15)
template <class T> int f(T t, int k = [] { return V; }()) { return t + k; }
