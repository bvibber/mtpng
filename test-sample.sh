mkdir -p out && \
cd samples && \
x="$1"
shift
cargo --quiet run --release -- "$@" "$x" "../out/$x" || exit 1
echo ""
