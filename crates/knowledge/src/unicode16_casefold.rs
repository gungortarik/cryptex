//! Canonical full case folding using the Unicode 16.0.0 table shipped by
//! `focaccia` 2.0.0. The generated lookup is retained here because the public
//! upstream API compares folded strings but does not expose folded bytes.
//! License notices for the derived lookup are next to the generated source.

#[allow(dead_code)]
mod table {
    #[derive(Debug, Clone, Copy)]
    pub enum Mode {
        Full,
        Turkic,
    }

    #[derive(Debug, Clone, Copy)]
    pub enum Mapping {
        Empty,
        Single(u32),
        Double(u32, u32),
        Triple(u32, u32, u32),
    }

    mod lookup {
        include!("unicode16_casefold/lookup.rs");
    }

    pub fn fold(input: &str) -> String {
        let mut folded = String::with_capacity(input.len());
        for ch in input.chars() {
            match lookup::lookup(ch, Mode::Full) {
                Mapping::Empty => {}
                Mapping::Single(a) => folded.push(char::from_u32(a).expect("valid Unicode table")),
                Mapping::Double(a, b) => {
                    folded.push(char::from_u32(a).expect("valid Unicode table"));
                    folded.push(char::from_u32(b).expect("valid Unicode table"));
                }
                Mapping::Triple(a, b, c) => {
                    folded.push(char::from_u32(a).expect("valid Unicode table"));
                    folded.push(char::from_u32(b).expect("valid Unicode table"));
                    folded.push(char::from_u32(c).expect("valid Unicode table"));
                }
            }
        }
        folded
    }
}

pub fn fold(input: &str) -> String {
    table::fold(input)
}
