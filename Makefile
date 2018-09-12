.FAKE : test run all clean

CC=cc
PROFILE=release
RLIBDIR=target/$(PROFILE)
RLIB=$(RLIBDIR)/libmtpng.so

SOURCES=c/sample.c
HEADERS=c/mtpng.h
EXE=build/sample
LIB=build/libmtpng.so

all : $(EXE)

clean :
	rm -f $(EXE)
	rm -f $(LIB)
	cargo clean

run : all
	LD_LIBRARY_PATH=build ./$(EXE)

test : run

$(EXE) : $(SOURCES) $(HEADERS) $(LIB)
	$(CC) -L$(RLIBDIR) -lmtpng -o $(EXE) $(SOURCES)

$(LIB) : $(RLIB)
	mkdir -p build && \
	cp $(RLIB) $(LIB)

$(RLIB) : Cargo.toml src/*.rs
	cargo build --release
