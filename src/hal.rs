#[cfg(test)]
pub mod fake;

use crate::{Error, Result, PAGE_SIZE};
use core::{marker::PhantomData, ptr::NonNull};

/// A physical address as used for virtio.
pub type PhysAddr = usize;

/// A region of contiguous physical memory used for DMA.
#[derive(Debug)]
pub struct Dma<H: Hal> {
    paddr: usize,
    vaddr: NonNull<u8>,
    pages: usize,
    _phantom: PhantomData<H>,
}

impl<H: Hal> Dma<H> {
    /// Allocates the given number of pages of physically contiguous memory to be used for DMA in
    /// the given direction.
    pub fn new(pages: usize, direction: BufferDirection) -> Result<Self> {
        let (paddr, vaddr) = H::dma_alloc(pages, direction);
        if paddr == 0 {
            return Err(Error::DmaError);
        }
        Ok(Self {
            paddr,
            vaddr,
            pages,
            _phantom: PhantomData::default(),
        })
    }

    /// Returns the physical address of the start of the DMA region, as seen by devices.
    pub fn paddr(&self) -> usize {
        self.paddr
    }

    /// Returns a pointer to the given offset within the DMA region.
    pub fn vaddr(&self, offset: usize) -> NonNull<u8> {
        assert!(offset < self.pages * PAGE_SIZE);
        NonNull::new((self.vaddr.as_ptr() as usize + offset) as _).unwrap()
    }

    /// Returns a pointer to the entire DMA region as a slice.
    pub fn raw_slice(&self) -> NonNull<[u8]> {
        let raw_slice =
            core::ptr::slice_from_raw_parts_mut(self.vaddr(0).as_ptr(), self.pages * PAGE_SIZE);
        NonNull::new(raw_slice).unwrap()
    }
}

impl<H: Hal> Drop for Dma<H> {
    fn drop(&mut self) {
        let err = H::dma_dealloc(self.paddr, self.vaddr, self.pages);
        assert_eq!(err, 0, "failed to deallocate DMA");
    }
}

/// The interface which a particular hardware implementation must implement.
pub trait Hal {
    /// Allocates the given number of contiguous physical pages of DMA memory for VirtIO use.
    ///
    /// Returns both the physical address which the device can use to access the memory, and a
    /// pointer to the start of it which the driver can use to access it.
    fn dma_alloc(pages: usize, direction: BufferDirection) -> (PhysAddr, NonNull<u8>);
    /// Deallocates the given contiguous physical DMA memory pages.
    fn dma_dealloc(paddr: PhysAddr, vaddr: NonNull<u8>, pages: usize) -> i32;
    /// Converts a physical address used for MMIO to a virtual address which the driver can access.
    ///
    /// This is only used for MMIO addresses within BARs read from the device, for the PCI
    /// transport. It may check that the address range up to the given size is within the region
    /// expected for MMIO.
    fn mmio_phys_to_virt(paddr: PhysAddr, size: usize) -> NonNull<u8>;
    /// Shares the given memory range with the device, and returns the physical address that the
    /// device can use to access it.
    ///
    /// This may involve mapping the buffer into an IOMMU, giving the host permission to access the
    /// memory, or copying it to a special region where it can be accessed.
    fn share(buffer: NonNull<[u8]>, direction: BufferDirection) -> PhysAddr;
    /// Unshares the given memory range from the device and (if necessary) copies it back to the
    /// original buffer.
    fn unshare(paddr: PhysAddr, buffer: NonNull<[u8]>, direction: BufferDirection);
}

/// The direction in which a buffer is passed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BufferDirection {
    /// The buffer may be read or written by the driver, but only read by the device.
    DriverToDevice,
    /// The buffer may be read or written by the device, but only read by the driver.
    DeviceToDriver,
    /// The buffer may be read or written by both the device and the driver.
    Both,
}
