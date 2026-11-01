// [temp.friend]/1, [temp.dep.type]/1.2: a class template that befriends all its own
// specializations (libc++ weak_ptr: `template <class _Up> friend class weak_ptr;`) still names its
// current instantiation in the qualifier of an out-of-line member definition
//@ expect: run
//@ strict-diags
template <class T> class shared;
template <class T> class weak {
  T* p;
public:
  weak() noexcept;
  weak(T* q);
  template <class U> weak(const weak<U>& o);
  T* get() const;
  template <class U> friend class weak;
  template <class U> friend class shared;
};
template <class T> class shared {
  template <class U> friend class weak;
};
template <class T> inline weak<T>::weak() noexcept : p(nullptr) {}
template <class T> weak<T>::weak(T* q) : p(q) {}
template <class T> template <class U> weak<T>::weak(const weak<U>& o) : p(o.p) {}
template <class T> T* weak<T>::get() const { return p; }
int main() {
  int x = 3;
  weak<int> a;
  weak<int> b(&x);
  weak<const int> c(b);
  return (a.get() == nullptr && *c.get() == 3) ? 0 : 1;
}
