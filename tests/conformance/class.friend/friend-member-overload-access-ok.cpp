// [class.friend]/2, [temp.friend]/1: the befriended overload keeps its
// access next to non-friend siblings that do not use the private members
// (the rejected side is friend-member-overload-access.cpp). Found running
// GCC's C++ testsuite (WP-1513).
template <class T> struct Q1 { int f(int); int f(long); };
template <class T> class V1 {
  friend int Q1<T>::f(int);
  int x = 0;
  static int s;
};
template <class T> int V1<T>::s = 0;
template <class T> int Q1<T>::f(int) { V1<T> v; return v.x + V1<T>::s; }
template <class T> int Q1<T>::f(long) { return 0; }

template <class T> struct Q2 { int f(int); int f(long); };
template <class T> class V2 { friend int Q2<T>::f(long); static int s; };
template <class T> int V2<T>::s = 0;
template <class T> int Q2<T>::f(int) { return 0; }
template <class T> int Q2<T>::f(long) { return V2<T>::s; }

struct Q3 { int f(int); int f(long); };
template <class T> class V3 { friend int Q3::f(int); static int s; };
template <class T> int V3<T>::s = 0;
int Q3::f(int) { return V3<int>::s; }
int Q3::f(long) { return 0; }

template <class T> struct Q4 { int f(int); };
template <class T> class V4 { friend int Q4<int>::f(int); static int s; };
template <class T> int V4<T>::s = 0;
template <class T> int Q4<T>::f(int) { return V4<int>::s; }

int main() {
  Q1<int> q1;
  Q2<int> q2;
  Q3 q3;
  Q4<int> q4;
  return q1.f(1) + q1.f(1L) + q2.f(1L) + q2.f(1) + q2.f(1L) + q3.f(1) + q3.f(1L) + q4.f(1) +
         q1.f(2);
}
