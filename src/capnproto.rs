use std::ptr::NonNull;

use capnp::message::{Allocator, HeapAllocator};
use capnp::Word;

pub const SEGMENT_WORDS: usize = 8 * 1024;

pub struct ReusedSegment {
    words: Vec<Word>,
    lent: bool,
    overflow: HeapAllocator,
}

impl Default for ReusedSegment {
    fn default() -> ReusedSegment {
        ReusedSegment {
            words: Word::allocate_zeroed_vec(SEGMENT_WORDS),
            lent: false,
            overflow: HeapAllocator::new(),
        }
    }
}

unsafe impl Allocator for ReusedSegment {
    fn allocate_segment(&mut self, minimum_size: u32) -> (NonNull<u8>, u32) {
        if self.lent || minimum_size as usize > self.words.len() {
            return self.overflow.allocate_segment(minimum_size);
        }
        self.lent = true;
        let start = NonNull::new(self.words.as_mut_ptr().cast::<u8>()).unwrap();
        (start, self.words.len() as u32)
    }

    unsafe fn deallocate_segment(&mut self, ptr: NonNull<u8>, word_size: u32, words_used: u32) {
        if ptr.as_ptr() == self.words.as_mut_ptr().cast::<u8>() {
            Word::words_to_bytes_mut(&mut self.words[..words_used as usize]).fill(0);
            self.lent = false;
        } else {
            unsafe { self.overflow.deallocate_segment(ptr, word_size, words_used) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(start: NonNull<u8>, words: u32) -> &'static mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(start.as_ptr(), words as usize * 8) }
    }

    #[test]
    fn the_segment_is_lent_again_zeroed() {
        let mut segment = ReusedSegment::default();
        let (first, words) = segment.allocate_segment(4);
        assert_eq!(words as usize, SEGMENT_WORDS);
        bytes(first, words)[..64].fill(0xAB);
        unsafe { segment.deallocate_segment(first, words, 8) };

        let (again, words) = segment.allocate_segment(4);
        assert_eq!(again, first);
        assert!(bytes(again, words).iter().all(|&byte| byte == 0));
        unsafe { segment.deallocate_segment(again, words, 0) };
    }

    #[test]
    fn a_second_or_oversized_segment_comes_from_the_heap() {
        let mut segment = ReusedSegment::default();
        let (lent, lent_words) = segment.allocate_segment(1);
        let (second, second_words) = segment.allocate_segment(1);
        assert_ne!(second, lent);
        unsafe { segment.deallocate_segment(second, second_words, 0) };
        unsafe { segment.deallocate_segment(lent, lent_words, 0) };

        let (oversized, oversized_words) = segment.allocate_segment(SEGMENT_WORDS as u32 + 1);
        assert_ne!(oversized, lent);
        assert!(oversized_words as usize > SEGMENT_WORDS);
        unsafe { segment.deallocate_segment(oversized, oversized_words, 0) };

        let (reused, reused_words) = segment.allocate_segment(1);
        assert_eq!(reused, lent);
        unsafe { segment.deallocate_segment(reused, reused_words, 0) };
    }
}
