use ltk_ritobin::cst::Cst;

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    //TODO: better arbitrary source gen
    #[test]
    fn build_bin_never_panics_on_arbitrary_text(text in ".{0,400}") {
        let cst = Cst::parse(&text);
        let _ = cst.build_bin(&text);
    }
}
