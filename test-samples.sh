mkdir -p out && \
cd samples && \
for x in *.png
do
  cargo --quiet run --release -- "$@" "$x" "../out/$x" || exit 1
  echo ""
done
