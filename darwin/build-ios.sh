TARGETS="x86_64-apple-ios aarch64-apple-ios"
PRODUCTS=""

for target in $TARGETS
do
  rustup target add "$target" || exit 1
  cargo build --release --target="$target" --features="capi" || exit 1
  PRODUCTS="$PRODUCTS ../target/$target/release/libmtpng.a"
done

#mkdir -p mtpng.Framework/Headers || exit 1
#lipo -create -output mtpng.Framework/mtpng $PRODUCTS || exit 1
#cp -p ../c/mtpng.h mtpng.Framework/Headers/mtpng.h || exit 1
#cp -p mtpng.modulemap mtpng.Framework/mtpng.modulemap || exit 1

#lipo -create -output mtpng.a $PRODUCTS || exit 1
#cp -p ../c/mtpng.h mtpng.h || exit 1

mkdir -p mtpng || exit 1
lipo -create -output mtpng/libmtpng.a $PRODUCTS || exit 1
cp -p ../c/mtpng.h mtpng/mtpng.h || exit 1
cp -p module.map mtpng/module.map || exit 1
