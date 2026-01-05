# Performance

Testing encoding the dual-4K screenshot at default options, single-threaded and again with all logical processors engaged; use fastest run in each command:

```
cargo run --release --features=cli -- --repeat=10 samples/dual4k.png /dev/null
cargo run --release --features=cli -- --repeat=10 --threads=1 samples/dual4k.png /dev/null
```

(substitute `NUL` for `/dev/null` on Windows native builds)

@fixme how did I measure those libpng times? need to recreate that

@fixme how do we reliably get a faster zlib for win32?


## 2026 data

### Apple M3 Max

MacBook Pro 16" Nov 2023
Apple M3 Max
16 logical cores (12P/4E)

macOS 15:
- mtpng @ 16 threads:  23 ms
- mtpng @ 1 thread:   237 ms

### AMD Ryzen 9 5950X

Home-built PC (2020s, AM4 socket)
AMD Ryzen 9 5950X
32 logical cores (16C/32T)

Linux Debian 13:
- mtpng @ 32 threads:  25 ms
- mtpng @ 1 thread:   301 ms

Windows 11:
- mtpng @ 32 threads:  31 ms
- mtpng @ 1 thread:   403 ms

### Apple M1

MacBook Air 13" 2020
Apple M1
8 logical cores (4P/4E)

macOS 15:
- mtpng @ 8 threads:   62 ms
- mtpng @ 1 thread:   296 ms

### Intel Xeon E5520

Dell Precision 5500T Workstation (2009)
2x Intel Xeon E5520
16 logical cores (8C/16T)

Linux Debian 13:
- mtpng @ 16 threads: 113 ms
- mtpng @ 1 threads:  923 ms

### Microsoft SQ1

Surface Pro X (2020)
Microsoft SQ1 (customized Qualcomm Snapdragon 8cx)
8 logical cores (4P/4E)

Linux Debian 13 (WSL2):
* mtpng @ 8 threads: 151 ms
* mtpng @ 1 thread:  655 ms

Windows 11 25H2:
* mtpng @ 8 threads: 157 ms
* mtpng @ 1 thread:  679 ms

### Raspberry Pi 4b

Raspberry Pi 4b
(Some kind of aarch64)
4 logical cores (4C)

Linux Debian 13:
- mtpng @ 4 threads:  512 ms
- mtpng @ 1 thread:  1645 ms

### Raspberry Pi 3b+

Raspberry Pi 3b+
(Some kind of aarch64)
4 logical cores (4C)

Linux Debian 13:
- mtpng @ 4 threads: 1008 ms
- mtpng @ 1 thread:  2977 ms


## Old performance data

Times for re-encoding the dual-4K screenshot at default options:

```
MacBook Pro 13" 2015
5th-gen Core i7 3.1 GHz
2 cores + Hyper-Threading

Linux x86_64:
- libpng gcc         --  850 ms (target to beat)
- libpng clang       --  900 ms
- mtpng @  1 thread  --  555 ms -- 1.0x (victory!)
- mtpng @  2 threads --  302 ms -- 1.8x
- mtpng @  4 threads --  248 ms -- 2.2x (HT)

macOS x86_64:
- libpng clang       --  943 ms (slower than Linux/gcc)
- mtpng @  1 thread  --  563 ms -- 1.0x (nice!)
- mtpng @  2 threads --  299 ms -- 1.9x
- mtpng @  4 threads --  252 ms -- 2.2x (HT)
```

macOS and Linux x86_64 perform about the same on the same machine, but libpng on macOS is built with clang, which seems to optimize libpng's filters worse than gcc does. This means we beat libpng on macOS by a larger margin than on Linux, where it's usually built with gcc.


```
Refurbed old Dell workstation
Xeon E5520 2.26 GHz
2x 4 cores + Hyper-Threading
configured for SMP (NUMA disabled)

Linux x86_64:
- libpng gcc         -- 1695 ms (target to beat)
- mtpng @  1 thread  -- 1124 ms -- 1.0x (winning!)
- mtpng @  2 threads --  570 ms -- 2.0x
- mtpng @  4 threads --  298 ms -- 3.8x
- mtpng @  8 threads --  165 ms -- 6.8x
- mtpng @ 16 threads --  155 ms -- 7.3x (HT)

Windows 10 x86_64:
- mtpng @  1 thread  -- 1377 ms -- 1.0x
- mtpng @  2 threads --  708 ms -- 1.9x
- mtpng @  4 threads --  375 ms -- 3.7x
- mtpng @  8 threads --  214 ms -- 6.4x
- mtpng @ 16 threads --  170 ms -- 8.1x (HT)

Windows 10 i686:
- mtpng @  1 thread  -- 1524 ms -- 1.0x
- mtpng @  2 threads --  801 ms -- 1.9x
- mtpng @  4 threads --  405 ms -- 3.8x
- mtpng @  8 threads --  243 ms -- 6.3x
- mtpng @ 16 threads --  194 ms -- 7.9x
```

Windows seems a little slower than Linux on the same machine, not quite sure why. The Linux build runs on Windows 10's WSL compatibility layer slightly slower than native Linux but faster than native Windows.

32-bit builds are a bit slower still, but I don't have a Windows libpng comparison handy.

```
Raspberry Pi 3B+
Cortex A53 1.4 GHz
4 cores

Linux armhf (Raspian):
- libpng gcc         -- 5368 ms
- mtpng @  1 thread  -- 6068 ms -- 1.0x
- mtpng @  2 threads -- 3126 ms -- 1.9x
- mtpng @  4 threads -- 1875 ms -- 3.2x

Linux aarch64 (Fedora 29):
- libpng gcc         -- 4692 ms
- mtpng @  1 thread  -- 4311 ms -- 1.0x
- mtpng @  2 threads -- 2140 ms -- 2.0x
- mtpng @  4 threads -- 1416 ms -- 3.0x
```

On 32-bit ARM we don't quite beat libpng single-threaded, but multi-threaded still does well. 64-bit ARM does better, perhaps because libpng is less optimized there. Note this machine throttles aggressively if it heats up, making the second run of a repeat on a long file like that noticeably slower than the first.

```
iPhone X
A11 2.39 GHz
6 cores (2 big, 4 little)

iOS aarch64:
- mtpng @ 1 thread  -- 802 ms -- 1.0x
- mtpng @ 2 threads -- 475 ms -- 1.7x
- mtpng @ 4 threads -- 371 ms -- 2.2x
- mtpng @ 6 threads -- 320 ms -- 2.5x
```

A high-end 64-bit ARM system is quite a bit faster! It scales ok to 2 cores, getting smaller but real benefits from scheduling further work on the additional little cores.

```
Lenovo C630 Yoga
Snapdragon 850 2.96 GHz
8 cores (4 big, 4 little?)

Windows Subsystem for Linux aarch64 (Win10 1903):
- mtpng @ 1 thread  -- 1029ms -- 1.0x
- mtpng @ 2 threads --  516ms -- 2.0x
- mtpng @ 4 threads --  317ms -- 3.2x
- mtpng @ 6 threads --  262ms -- 3.9x
- mtpng @ 8 threads --  241ms -- 4.3x
```

The Snapdragon 850 scores not as well as the A11 in single-threaded, but catches up with additional threads.



```
MacBook Air (M1, 2020)
Apple M1 3.2 GHz
8 cores (4 big, 4 little?)

macOS 11.1 (20C69):
- mtpng @ 1 thread  --  534ms -- 1.0x
- mtpng @ 2 threads --  279ms -- 2.0x
- mtpng @ 4 threads --  148ms -- 3.2x
- mtpng @ 6 threads --  125ms -- 3.9x
- mtpng @ 8 threads --  108ms -- 4.3x
```
