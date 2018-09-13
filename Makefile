.FAKE : test run all clean

CC=cc
PROFILE=release
RUSTLIBDIR=target/$(PROFILE)


# This is all hacky and not gonna help with cross-compiling. :D
ifeq ($(OS),Windows_NT)
  CDYLIB_EXT=dll
  CDYLIB_PATHVAR=
else
  UNAME_S=$(shell uname -s)
  ifeq ($(UNAME_S),Linux)
    CDYLIB_EXT=so
    CDYLIB_PATHVAR=LD_LIBRARY_PATH=build
  endif
  ifeq ($(UNAME_S),Darwin)
    CDYLIB_EXT=dylib
    CDYLIB_PATHVAR=
  endif
endif

RUSTLIB=$(RUSTLIBDIR)/libmtpng.$(CDYLIB_EXT)

SOURCES=c/sample.c
HEADERS=c/mtpng.h
EXE=build/sample
LIB=build/libmtpng.$(CDYLIB_EXT)

all : $(EXE)

clean :
	rm -f $(EXE)
	rm -f $(LIB)
	cargo clean

run : all
	mkdir -p out && \
	$(CDYLIB_PATHVAR) ./$(EXE)

test : run

$(EXE) : $(SOURCES) $(HEADERS) $(LIB)
	$(CC) -g -o $(EXE) $(SOURCES) -L./build -lmtpng

$(LIB) : $(RUSTLIB)
	mkdir -p build && \
	cp $(RUSTLIB) $(LIB)

$(RUSTLIB) : Cargo.toml src/*.rs
	cargo build --release
