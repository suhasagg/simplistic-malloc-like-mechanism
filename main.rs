use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use std::sync::atomic::{AtomicUsize, Ordering};

// 1. We define a fixed-size buffer that will act as our "heap."
//    For real-world use, you'd want something more flexible or dynamic.
const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB for demo
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

// A simple helper function to align the current offset
// to the alignment required by `layout.align()`.
#[inline]
fn align_up(addr: usize, align: usize) -> usize {
    // align must be a power of two for this simplistic approach
    (addr + align - 1) & !(align - 1)
}

// 2. A simple bump allocator structure.
pub struct BumpAllocator {
    // The starting address of the heap (as a usize).
    heap_start: usize,
    // The ending address of the heap (as a usize).
    heap_end: usize,
    // An atomic to hold the *next* allocation index.
    // Using `AtomicUsize` allows us to do lock-free increments,
    // though we are ignoring concurrency issues for this example.
    next: AtomicUsize,
}

unsafe impl Sync for BumpAllocator {} // Required for global allocator, trivial here

// 3. Implement `GlobalAlloc` for our `BumpAllocator`.
unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();

        // current allocation pointer
        let mut current_next = self.next.load(Ordering::Relaxed);

        // Bump the pointer up to meet alignment requirements
        let aligned = align_up(current_next, align);
        let new_next = aligned.saturating_add(size);

        // Check for out-of-memory
        if new_next > self.heap_end {
            // Not enough space
            return null_mut();
        }

        // CAS loop if multiple threads might attempt allocations at once
        // For simplicity, do a single store here ignoring concurrency complexities
        self.next.store(new_next, Ordering::Relaxed);

        aligned as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // In a naive bump allocator, deallocation is a no-op or near no-op.
        // Proper free/defragmentation is not handled here. 
    }
}

// 4. Create a static instance of our BumpAllocator and tag it as the global allocator.
#[global_allocator]
static GLOBAL: BumpAllocator = BumpAllocator {
    heap_start: 0,
    heap_end: 0,
    next: AtomicUsize::new(0),
};

// 5. We use Rust's `#[ctor]`-like approach or a manual "init" function to properly
// initialize the heap addresses *before main* runs. In stable Rust, the easiest
// approach is to do it in `main` the first time we need it. We'll do a function here
// that MUST be called before any real allocations. This is a simplified approach.
fn init_heap() {
    unsafe {
        let start = HEAP.as_ptr() as usize;
        let end = start + HEAP_SIZE;

        GLOBAL.next.store(start, Ordering::SeqCst);
        let bump_alloc = &GLOBAL as *const BumpAllocator as *mut BumpAllocator;
        (*bump_alloc).heap_start = start;
        (*bump_alloc).heap_end = end;
    }
}

fn main() {
    // Initialize the bump allocator
    init_heap();

    // **DEMO A**: Allocate a Box on our custom "heap"
    // The memory used by this Box will come from our BumpAllocator, not the default system malloc.
    let my_box = Box::new(42);
    println!("Allocated a box with value: {}", my_box);

    // **DEMO B**: Allocate a vector
    // This will also use our custom allocator.
    let mut my_vec = Vec::with_capacity(10);
    my_vec.extend_from_slice(&[1, 2, 3, 4, 5]);
    println!("Allocated a vector: {:?}", my_vec);

    // **DEMO C**: Manual "malloc" style usage with pointer arithmetic
    // We'll do a small example to store some data using unsafe pointers.
    unsafe {
        let size = 8; // let's say we want 8 bytes
        let align = 4; // alignment requirement
        let layout = Layout::from_size_align(size, align).unwrap();

        // Our naive 'malloc' call
        let ptr = GLOBAL.alloc(layout);
        if !ptr.is_null() {
            // Store data: We'll store two 32-bit integers in that block
            // `ptr` is of type *mut u8; let's cast it to a *mut u32 for storing an integer
            let int_ptr = ptr as *mut u32;
            *int_ptr = 0xDEAD_BEEF;      // store first integer
            *int_ptr.add(1) = 0xC0FFEE;  // store second integer

            // Read back the data
            println!(
                "Manually allocated block. First: 0x{:X}, Second: 0x{:X}",
                *int_ptr,
                *int_ptr.add(1)
            );
        } else {
            eprintln!("Out of memory in manual allocation!");
        }
    }

    // Note: We are not calling `dealloc` here since our naive bump allocator
    // effectively doesn't handle real frees. The memory usage only grows upward.
    // Everything is freed once the process ends.

    println!("Demo complete.");
}

