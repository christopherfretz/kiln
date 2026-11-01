// [temp.inst], [except.handle] (WP-160): try-blocks, handlers and
// function-try-blocks in function templates and class templates, with the
// exception declaration rebuilt from the substituted type (copy-initialized
// from the exception object, destroyed at the end of the handler).
int g(int);
struct C { C(); C(const C &); ~C(); int v; };

template <class T> int f(T v) {
  try {
    if (v)
      throw v;
  } catch (T e) {
    return g((int)e);
  } catch (...) {
    return 3;
  }
  return 0;
}

template <class T> int ftb(T v) try {
  throw v;
} catch (T &e) {
  return (int)e;
} catch (...) {
  throw;
}

template <class T> struct S {
  T m;
  S(T v) try : m(v) {
    g(1);
  } catch (const T &) {
    g(2);
  }
};

template <class T> int nested() {
  try {
    try {
      throw T();
    } catch (T &x) {
      throw;
    }
  } catch (T x) {
    return x.v;
  }
  return 0;
}

int use() {
  S<long> s(2);
  return f(1) + f(2.0) + ftb(3) + nested<C>();
}
