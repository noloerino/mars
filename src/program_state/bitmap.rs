/// A simple implementation of an integer-backed bitmap, for use in representing free lists for
/// disk blocks or memory sectors.
///
/// The map itself is backed by a vector of u64s that are preallocated to 0.
pub struct Bitmap {
    bit_cnt: usize,
    bits: Vec<u64>,
}

impl Bitmap {
    /// Constructs a new bitmap with bit_cnt entries.
    pub fn new(bit_cnt: usize) -> Bitmap {
        // if we need 0 bits then we allocate 0 ints, but if we need 1 byte we need to allocate
        // 1 int even though 1 / 64 = 0
        let round_up = bit_cnt % 64 != 0;
        Bitmap {
            bit_cnt,
            bits: vec![0; bit_cnt / 64 + (if round_up { 1 } else { 0 })],
        }
    }

    fn bounds_check(&self, idx: usize) {
        if idx >= self.bit_cnt {
            panic!(
                "Attempted to access index {} for bitmap of size {}",
                idx, self.bit_cnt
            )
        }
    }

    /// Reads an entry in the bitmap. Panics if the index is out of bounds.
    pub fn read(&self, idx: usize) -> bool {
        self.bounds_check(idx);
        let n = self.bits[idx / 64];
        let bit_idx = idx % 64;
        ((n >> bit_idx) & 0b1) != 0
    }

    /// Flips an entry in the bitmap. Panics if the index is out of bounds.
    pub fn flip(&mut self, idx: usize) {
        self.bounds_check(idx);
        // to flip one bit, XOR the entry with a mask of all 0s
        // except at the desired index
        let bit_idx = idx % 64;
        let mask = 1 << bit_idx;
        let k = idx / 64;
        self.bits[k] = self.bits[k] ^ mask;
    }
}

#[cfg(test)]
mod tests {
    use super::Bitmap;

    /// Tests a bitmap backed by only a single integer.
    #[test]
    fn test_bitmap_small() {
        let mut bm = Bitmap::new(64);
        bm.flip(0);
        bm.flip(31);
        // extra checks ensure no off-by-one errors
        assert_eq!(bm.read(0), true);
        assert_eq!(bm.read(1), false);
        assert_eq!(bm.read(30), false);
        assert_eq!(bm.read(31), true);
        assert_eq!(bm.read(32), false);
        bm.flip(0);
        assert_eq!(bm.read(0), false);
        assert_eq!(bm.read(1), false);
    }

    /// Tests a bitmap backed by several integers.
    #[test]
    fn test_bitmap_several() {
        let mut bm = Bitmap::new(256);
        bm.flip(0);
        bm.flip(31);
        bm.flip(142);
        bm.flip(255);
        // extra checks ensure no off-by-one errors
        assert_eq!(bm.read(0), true);
        assert_eq!(bm.read(31), true);
        assert_eq!(bm.read(142), true);
        assert_eq!(bm.read(255), true);
        bm.flip(142);
        assert_eq!(bm.read(142), false);
    }

    #[test]
    #[should_panic]
    fn test_bitmap_cap() {
        Bitmap::new(128).read(128);
    }
}
