//! N-dimensional puzzle backend.
#![warn(clippy::if_then_some_else_none, missing_docs)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
pub mod math;
pub mod polytope;
pub mod puzzle;
pub mod schlafli;
pub mod util;

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_puzzles_from_file {
        ($(fn $test_name:ident: $file:literal),+ $(,)?) => {
            $(
                #[test]
                fn $test_name() {
                    test_puzzle_from_yaml(include_str!(concat!("../../puzzles/", $file, ".yaml")));
                }
            )+
        };
    }

    fn test_puzzle_from_yaml(s: &str) {
        let spec: puzzle::jumbling::JumblingPuzzleSpec =
            serde_yaml::from_str(s).expect("error parsing spec");
        spec.build().expect("error building puzzle");
    }

    test_puzzles_from_file! {
        fn test_1x1x1: "1x1x1",
        fn test_1x1x1x1: "1x1x1x1",
        fn test_2x2x2: "2x2x2",
        fn test_2x2x2x2: "2x2x2x2",
        fn test_2x2x2x2x2: "2x2x2x2x2",
        fn test_2x2x2x4: "2x2x2x4",
        fn test_2x3x4: "2x3x4",
        fn test_2x3x4x5: "2x3x4x5",
        fn test_3x3: "3x3",
        fn test_3x3x3: "3x3x3",
        fn test_3x3x3x3: "3x3x3x3",
        fn test_3x3x3x5: "3x3x3x5",
        fn test_4x4x4x4: "4x4x4x4",
        fn test_5x5x5: "5x5x5",
        fn test_5x5x5x5: "5x5x5x5",
        fn test_10x10x10: "10x10x10",
        fn test_17x17x17: "17x17x17",
        fn test_dino: "Dino",
        fn test_fto: "FTO",
        fn test_half_rt: "Half_RT",
        fn test_half_vt: "Half_VT",
        fn test_helicopter: "Helicopter",
        fn test_hmt: "HMT",
        fn test_rhombic_dodecahedron: "Rhombic Dodecahedron",
        fn test_skewb: "Skewb",
    }
}
