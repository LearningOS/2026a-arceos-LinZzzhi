#![no_std]

use allocator::{BaseAllocator, ByteAllocator, PageAllocator};

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    /// Next free byte position, growing forward.
    b_pos: usize,
    /// Start of the page area, growing backward.
    p_pos: usize,
    /// Number of outstanding byte allocations.
    count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = self.end;
        self.count = 0;
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> allocator::AllocResult {
        // A bump allocator cannot merge disjoint regions into its single
        // forward/backward range.
        Err(allocator::AllocError::NotAllocated)
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,
    ) -> allocator::AllocResult<core::ptr::NonNull<u8>> {
        let align = layout.align();
        let size = layout.size();
        if !align.is_power_of_two() {
            return Err(allocator::AllocError::InvalidParam);
        }

        // Move the byte cursor forward, keeping the requested alignment.
        let start = self.b_pos.checked_add(align - 1).ok_or(allocator::AllocError::NoMemory)?
            & !(align - 1);
        let end = start.checked_add(size).ok_or(allocator::AllocError::NoMemory)?;
        if end > self.p_pos {
            return Err(allocator::AllocError::NoMemory);
        }

        self.b_pos = end;
        self.count += 1;
        core::ptr::NonNull::new(start as *mut u8).ok_or(allocator::AllocError::NoMemory)
    }

    fn dealloc(&mut self, _pos: core::ptr::NonNull<u8>, _layout: core::alloc::Layout) {
        // Bytes are allocated in a LIFO-ish bump fashion. Only when all
        // allocations are freed can the whole byte area be reclaimed.
        self.count -= 1;
        if self.count == 0 {
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> allocator::AllocResult<usize> {
        // `align_pow2` is the required alignment in bytes (not its exponent).
        if align_pow2 % SIZE != 0 || !(align_pow2 / SIZE).is_power_of_two() {
            return Err(allocator::AllocError::InvalidParam);
        }
        if num_pages == 0 {
            return Err(allocator::AllocError::InvalidParam);
        }

        let size = num_pages.checked_mul(SIZE).ok_or(allocator::AllocError::NoMemory)?;
        // Move the page cursor backward, keeping the requested alignment.
        let start = self
            .p_pos
            .checked_sub(size)
            .ok_or(allocator::AllocError::NoMemory)?
            & !(align_pow2 - 1);
        if start < self.b_pos {
            return Err(allocator::AllocError::NoMemory);
        }

        self.p_pos = start;
        Ok(start)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        // The page area is a bump allocator that never frees pages.
    }

    fn total_pages(&self) -> usize {
        (self.end - self.start) / SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / SIZE
    }
}