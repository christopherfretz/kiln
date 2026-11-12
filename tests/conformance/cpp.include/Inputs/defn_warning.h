// WP-1320: a header template that warns where it is defined (not where it
// is instantiated).
#ifndef DEFN_WARNING_H
#define DEFN_WARNING_H
template <class T> int pick(T x) {
  int unused;
  return x ? 1 : 2;
}
#endif
