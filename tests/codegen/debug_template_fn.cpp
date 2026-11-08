// Debug info (WP-814): function template specializations and their template parameters.
//@ args: -g
template <class T> T twice(T x) { return x + x; }
long use(int v) { return twice(v) + twice<long>(2); }
