export module xidef;
export template <class T> T twice(T t) { return t + t; }
export extern template double twice<double>(double);
