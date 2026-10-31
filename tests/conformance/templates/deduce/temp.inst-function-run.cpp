// [temp.inst]: specializations run in the generated program.
template <class T> T twice(T x) { return x + x; }
template <class T> T sq(const T& x) { return x * x; }
int main() { return twice(3) + sq(2) == 10 ? 0 : 1; }
