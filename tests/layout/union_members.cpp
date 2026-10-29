struct In { char c; short s; };
union U { In in; long long ll; char c[11]; };
struct S { char c; U u; U arr[2]; };
