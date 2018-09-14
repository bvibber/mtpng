FILTERS="adaptive none sub up average paeth"
SAMPLES=samples/*.png

for filter in $FILTERS
do
    for xsample in $SAMPLES
    do
        sample=`basename "$xsample"`
        cargo run --quiet --release -- --filter="$filter" "samples/$sample" "out/${sample%.png}-$filter.png"
    done
done
