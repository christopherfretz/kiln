export module imgpi;
export namespace pi {
template <class T> struct pos {
  T off;
  pos &operator+=(T d) { off += d; return *this; }
  pos operator+(T d) const { pos p(*this); p += d; return p; }
  template <class U> static pos make(U u) { pos p(u, 0); return p; }
  pos(T o) : off(o) {}
  template <class U> pos(U a, int b) : off(T(a) + b) {}
};
} // namespace pi
