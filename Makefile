.FAKE : test run all clean

CC=cc
CFLAGS=-g
LDFLAGS=-L./build -lmtpng

CARGO=cargo
PROFILE=release
RUSTLIBDIR=target/$(PROFILE)

# This is all hacky and not gonna help with cross-compiling. :D
ifeq ($(OS),Windows_NT)
  EXE_EXT=.exe
  CDYLIB_EXT=.dll
  CDYLIB_PATHVAR=
else
  UNAME_S=$(shell uname -s)
  ifeq ($(UNAME_S),Linux)
    EXE_EXT=
    CDYLIB_EXT=.so
    CDYLIB_PATHVAR=LD_LIBRARY_PATH=build
  endif
  ifeq ($(UNAME_S),Darwin)
    EXE_EXT=
    CDYLIB_EXT=.dylib
    CDYLIB_PATHVAR=
  endif
endif

RUSTLIB=$(RUSTLIBDIR)/libmtpng$(CDYLIB_EXT)

SOURCES=c/sample.c
HEADERS=c/mtpng.h
EXE=build/sample$(EXE_EXT)
LIB=build/libmtpng$(CDYLIB_EXT)

all : $(EXE)

clean :
	rm -f $(EXE)
	rm -f $(LIB)
	cargo clean

run : all
	mkdir -p out
	$(CDYLIB_PATHVAR) ./$(EXE)

test : run

$(EXE) : $(SOURCES) $(HEADERS) $(LIB)
	$(CC) $(CFLAGS) -o $(EXE) $(SOURCES) $(LDFLAGS)

$(LIB) : $(RUSTLIB)
	mkdir -p build && \
	cp $(RUSTLIB) $(LIB)

$(RUSTLIB) : Cargo.toml src/*.rs
	$(CARGO) build --release
