// A friend function template of a class template, declared in the class
// and defined after it, in a named module.
export module fta;
export namespace fta {
template <class T> struct It {
  template <class U> friend int peek(const It<U> &);
  T *p = nullptr;
};
template <class U> int peek(const It<U> &i) { return i.p ? 1 : 0; }
}
