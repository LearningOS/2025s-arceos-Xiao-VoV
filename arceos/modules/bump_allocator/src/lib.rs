#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

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
pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    // 内存区域起始地址
    start: usize,
    // 内存区域结束地址
    end: usize,
    // 字节分配器当前位置
    byte_pos: AtomicUsize,
    // 页分配器当前位置
    page_pos: AtomicUsize,
    // 字节分配计数
    byte_count: AtomicUsize,
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            byte_pos: AtomicUsize::new(0),
            page_pos: AtomicUsize::new(0),
            byte_count: AtomicUsize::new(0),
        }
    }

    /// 对齐地址到指定的对齐要求
    fn align_up(addr: usize, align: usize) -> usize {
        (addr + align - 1) & !(align - 1)
    }
}

impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.byte_pos.store(start, Ordering::SeqCst);
        self.page_pos.store(self.end, Ordering::SeqCst);
        self.byte_count.store(0, Ordering::SeqCst);
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> AllocResult {
        // 不支持
        Err(AllocError::InvalidParam)
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let align = layout.align();
        let size = layout.size();

        // 计算对齐后的当前字节位置
        let current_pos = self.byte_pos.load(Ordering::SeqCst);
        let aligned_pos = Self::align_up(current_pos, align);

        // 计算分配后的新位置
        let new_pos = aligned_pos + size;

        // 检查是否有足够的空间
        let page_pos = self.page_pos.load(Ordering::SeqCst);
        if new_pos > page_pos {
            return Err(AllocError::NoMemory);
        }

        // 更新字节位置
        self.byte_pos.store(new_pos, Ordering::SeqCst);

        // 增加分配计数
        self.byte_count.fetch_add(1, Ordering::SeqCst);

        // 返回分配的内存指针
        Ok(NonNull::new(aligned_pos as *mut u8).unwrap())
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        // 减少分配计数
        let count = self.byte_count.fetch_sub(1, Ordering::SeqCst);

        // 如果计数为0，重置字节分配器位置
        if count == 1 {
            self.byte_pos.store(self.start, Ordering::SeqCst);
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        let byte_pos = self.byte_pos.load(Ordering::SeqCst);
        byte_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        let byte_pos = self.byte_pos.load(Ordering::SeqCst);
        let page_pos = self.page_pos.load(Ordering::SeqCst);
        if page_pos > byte_pos {
            page_pos - byte_pos
        } else {
            0
        }
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;
    ///
    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        // 计算需要的总字节数
        let size = num_pages * PAGE_SIZE;

        // 计算对齐掩码
        let align_mask = align_pow2 - 1;

        // 从页分配器位置减去所需大小
        let page_pos = self.page_pos.load(Ordering::SeqCst);
        let new_pos = page_pos.checked_sub(size).ok_or(AllocError::NoMemory)?;

        // 计算对齐后的位置（向下对齐）
        let aligned_pos = new_pos & !align_mask;

        // 检查是否有足够的空间
        let byte_pos = self.byte_pos.load(Ordering::SeqCst);
        if aligned_pos <= byte_pos {
            return Err(AllocError::NoMemory);
        }

        // 更新页分配器位置
        self.page_pos.store(aligned_pos, Ordering::SeqCst);

        // 返回分配的页起始地址
        Ok(aligned_pos)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {}

    fn total_pages(&self) -> usize {
        (self.end - self.start) / PAGE_SIZE
    }

    fn used_pages(&self) -> usize {
        let page_pos = self.page_pos.load(Ordering::SeqCst);
        (self.end - page_pos) / PAGE_SIZE
    }

    fn available_pages(&self) -> usize {
        let byte_pos = self.byte_pos.load(Ordering::SeqCst);
        let page_pos = self.page_pos.load(Ordering::SeqCst);
        if page_pos > byte_pos {
            (page_pos - byte_pos) / PAGE_SIZE
        } else {
            0
        }
    }
}
