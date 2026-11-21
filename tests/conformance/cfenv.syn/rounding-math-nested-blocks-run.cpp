// [cfenv.syn], clang `-frounding-math`: every block of a function is
// environment-dependent, so codegen keeps its floating candidates from
// trial folding. Testing them stays linear in the nesting depth and in
// a chain of `const` locals that each read the previous one twice
// (reduced from real code: the walk once revisited each initializer
// per read, exponential in the chain, WP-1798).
//@ expect: run
//@ args: -O0 -frounding-math
//@ stdout: 1073741824 0.5
#include <cstdio>

int main() {
  const int a0 = 1;
  const int a1 = a0 + a0;
  const int a2 = a1 + a1;
  const int a3 = a2 + a2;
  const int a4 = a3 + a3;
  const int a5 = a4 + a4;
  const int a6 = a5 + a5;
  const int a7 = a6 + a6;
  const int a8 = a7 + a7;
  const int a9 = a8 + a8;
  const int a10 = a9 + a9;
  const int a11 = a10 + a10;
  const int a12 = a11 + a11;
  const int a13 = a12 + a12;
  const int a14 = a13 + a13;
  const int a15 = a14 + a14;
  const int a16 = a15 + a15;
  const int a17 = a16 + a16;
  const int a18 = a17 + a17;
  const int a19 = a18 + a18;
  const int a20 = a19 + a19;
  const int a21 = a20 + a20;
  const int a22 = a21 + a21;
  const int a23 = a22 + a22;
  const int a24 = a23 + a23;
  const int a25 = a24 + a24;
  const int a26 = a25 + a25;
  const int a27 = a26 + a26;
  const int a28 = a27 + a27;
  const int a29 = a28 + a28;
  const int a30 = a29 + a29;
  {
    {
      {
        {
          {
            {
              {
                {
                  {
                    {
                      {
                        {
                          {
                            {
                              {
                                {
                                  {
                                    {
                                      {
                                        {
                                          {
                                            {
                                              {
                                                {
                                                  double h = 1.0 / 2.0;
                                                  std::printf("%d %g\n", a30, h);
                                                }
                                              }
                                            }
                                          }
                                        }
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}
