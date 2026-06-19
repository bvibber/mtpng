set -e

function buildit() {
	dir="darwin/$1/mtpng"
	shift
	TARGETS="$@"
	PRODUCTS=""

	for target in $TARGETS
	do
		rustup target add "$target"
		cargo build --release --target="$target" --features="capi"
		PRODUCTS="$PRODUCTS target/$target/release/libmtpng.a"
	done

	mkdir -p "$dir"
	lipo -create -output "$dir/libmtpng.a" $PRODUCTS
	cp -p c/mtpng.h "$dir/mtpng.h"
	cp -p c/module.modulemap "$dir/module.modulemap"
}


buildit build/iphoneos aarch64-apple-ios
buildit build/iphonesimulator aarch64-apple-ios-sim x86_64-apple-ios
buildit build/macosxNO aarch64-apple-darwin x86_64-apple-darwin
buildit build/macosxYES aarch64-apple-ios-macabi x86_64-apple-ios-macabi
