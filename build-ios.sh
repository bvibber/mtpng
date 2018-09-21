TARGETS="x86_64-apple-ios aarch64-apple-ios"
PRODUCTS=""

for target in $TARGETS
do
  rustup target add "$target" || exit 1
  cargo build --release --target="$target" --features="capi" || exit 1
  PRODUCTS="$PRODUCTS target/$target/release/libmtpng.a"
done

mkdir -p darwin/mtpng || exit 1
lipo -create -output darwin/mtpng/libmtpng.a $PRODUCTS || exit 1
cp -p c/mtpng.h darwin/mtpng/mtpng.h || exit 1
cp -p c/module.map darwin/mtpng/module.map || exit 1
