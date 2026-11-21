// The module of image-used-inline-run.cpp: inline functions with GNU
// `used`, and overloads of one name.
export module imgui;
export inline __attribute__((used)) int one() { return 1; }
export inline __attribute__((used)) int pick(int) { return 2; }
export inline __attribute__((used)) int pick(long) { return 3; }
export inline __attribute__((used)) int unnamed() { return 4; }
export namespace ns {
inline __attribute__((used)) int deep() { return 5; }
}
export using fp = int (*)();
