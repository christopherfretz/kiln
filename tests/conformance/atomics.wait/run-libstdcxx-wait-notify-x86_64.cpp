// [atomics.wait], [atomics.types.operations]/30-34, [thread.sema],
// [thread.latch], [thread.barrier], [thread.stoptoken]: libstdc++ 14's
// <bits/atomic_wait.h> (and its <stop_token>) spin on
// `__builtin_ia32_pause` on x86-64 before blocking, so every
// std::atomic wait, semaphore, latch, barrier and stop_source on x86-64
// lowers it (`llvm.x86.sse2.pause`, target.x86/ia32-pause-ir.cpp). The
// TU also pulls in <memory>, the shape that broke every libstdc++
// <memory>/<atomic> TU on x86-64 before WP-610. Run on the x86-64 lane
// only (not pinned: the host's libstdc++ is the one under test). WP-529,
// WP-1002.
//@ requires: target-x86_64
//@ expect: run
//@ std: c++20
//@ args: -pthread
//@ stdout: wait 1 flag 1 sema 3 latch 4 barrier 2 stop 1 shared 2
#include <atomic>
#include <barrier>
#include <cstdio>
#include <latch>
#include <memory>
#include <semaphore>
#include <stop_token>
#include <thread>

int main() {
  std::atomic<int> a{0};
  std::thread t([&] {
    a.store(1);
    a.notify_one();
  });
  a.wait(0);
  t.join();

  std::atomic_flag f;
  std::thread tf([&] {
    f.test_and_set();
    f.notify_all();
  });
  f.wait(false);
  tf.join();

  std::counting_semaphore<8> sem(0);
  std::thread ts([&] { sem.release(3); });
  sem.acquire();
  sem.acquire();
  sem.acquire();
  ts.join();

  std::latch done(4);
  std::atomic<int> arrived{0};
  std::thread tl[4];
  for (auto &x : tl)
    x = std::thread([&] {
      arrived.fetch_add(1);
      done.count_down();
    });
  done.wait();
  for (auto &x : tl)
    x.join();

  std::atomic<int> phases{0};
  std::barrier sync(2, [&]() noexcept { phases.fetch_add(1); });
  std::thread tb([&] {
    sync.arrive_and_wait();
    sync.arrive_and_wait();
  });
  sync.arrive_and_wait();
  sync.arrive_and_wait();
  tb.join();

  std::stop_source src;
  std::stop_token tok = src.get_token();
  std::thread tt([&] { src.request_stop(); });
  tt.join();

  auto sp = std::make_shared<std::atomic<int>>(1);
  auto sp2 = sp;
  sp2->fetch_add(1);

  std::printf("wait %d flag %d sema %d latch %d barrier %d stop %d shared %d\n",
              a.load(), f.test() ? 1 : 0, 3, arrived.load(), phases.load(),
              tok.stop_requested() ? 1 : 0, sp->load());
}
