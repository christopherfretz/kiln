// The second translation unit of the lto-two-tu-*-run.cpp tests.
struct Shape { virtual ~Shape(); virtual int area() const = 0; };
struct Square : Shape { int s; explicit Square(int v) : s(v) {} int area() const override; };
Shape::~Shape() {}
int Square::area() const { return s * s; }
Shape* make_square(int v) { return new Square(v); }
int twice(int x) { return 2 * x; }
