use std::alloc::{alloc, Layout};
use std::{mem, ptr};
use winapi::um::memoryapi::{OpenFileMappingW, FILE_MAP_READ};
use winapi::{
    shared::minwindef::DWORD,
    um::{
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        memoryapi::{CreateFileMappingW, MapViewOfFile, FILE_MAP_ALL_ACCESS},
        winnt::{HANDLE, PAGE_READWRITE},
    },
};

const VIDEO_NAME: &str = "OBSVirtualCamVideo";
const FRAME_HEADER_SIZE: u32 = 32;
fn align_size(mut size: usize, align: usize) -> usize {
    size = (size + align - 1) & !(align - 1);
    size
}

#[derive(Debug)]
pub struct QueueHeader {
    pub write_idx: *mut u32,
    pub read_idx: *mut u32,
    pub state: *mut u32,
    pub offsets: [u32; 3],
    pub type_: u32,
    pub cx: u32,
    pub cy: u32,
    pub interval: u64,
    pub reserved: [u32; 8],
}

pub struct VideoQueue {
    pub handel: HANDLE,
    pub ready_to_read: bool,
    pub header: *mut QueueHeader,
    pub ts: [*mut u64; 3],
    pub frame: [*mut u8; 3],
    pub last_inc: i64,
    pub dup_counter: i32,
    pub is_writer: bool,
}

impl VideoQueue {
    pub fn video_queue_create(cx: u32, cy: u32, interval: u64) -> Option<*mut Self> {
        let mut vq = VideoQueue {
            handel: std::ptr::null_mut(),
            ready_to_read: false,
            header: std::ptr::null_mut(),
            ts: [std::ptr::null_mut(); 3],
            frame: [std::ptr::null_mut(); 3],
            last_inc: 0,
            dup_counter: 0,
            is_writer: false,
        };
        let frame_size: DWORD = cx * cy * 3 / 2;
        let mut offset_frame = [3; 3];
        let mut size = mem::size_of::<QueueHeader>() as u32;
        size = align_size(size as usize, 32) as u32;
        offset_frame[0] = size;
        size = size + frame_size + FRAME_HEADER_SIZE as u32;
        size = align_size(size as usize, 32) as u32;
        offset_frame[1] = size;
        size = size + frame_size + FRAME_HEADER_SIZE as u32;
        size = align_size(size as usize, 32) as u32;
        offset_frame[2] = size;
        size = size + frame_size + FRAME_HEADER_SIZE as u32;
        size = align_size(size as usize, 32) as u32;

        let mut header = QueueHeader {
            write_idx: std::ptr::null_mut(),
            read_idx: std::ptr::null_mut(),
            state: std::ptr::null_mut(),
            offsets: [0; 3],
            type_: 0,
            cx,
            cy,
            interval,
            reserved: [0; 8],
        };

        header.state = 1 as *mut u32;
        header.cx = cx;
        header.cy = cy;
        header.interval = interval;

        vq.is_writer = true;

        for i in 0..3 {
            header.offsets[i] = offset_frame[i];
        }

        vq.handel =
            unsafe { OpenFileMappingW(FILE_MAP_READ, 0, VIDEO_NAME.as_ptr() as *const u16) };
        if !vq.handel.is_null() {
            unsafe { CloseHandle(vq.handel) };
            return None;
        }

        vq.handel = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null_mut(),
                PAGE_READWRITE,
                0,
                size,
                VIDEO_NAME.as_ptr() as *const u16,
            )
        };
        if vq.handel.is_null() {
            return None;
        }

        vq.header =
            unsafe { MapViewOfFile(vq.handel, FILE_MAP_ALL_ACCESS, 0, 0, 0) as *mut QueueHeader };
        if vq.header.is_null() {
            unsafe { CloseHandle(vq.handel) };
            return None;
        }
        // 将header拷贝到共享内存中
        let header_size = mem::size_of::<QueueHeader>();
        unsafe {
            (&header as *const QueueHeader).copy_to_nonoverlapping(vq.header, header_size);
        }
        println!("header: {:?}", vq.header);

        for i in 0..3 {
            let off = offset_frame[i] as u32;
            let ts_ptr = unsafe { vq.header.offset(off as isize) } as *mut u64;
            vq.ts[i] = ts_ptr;
            let frame_ptr =
                unsafe { vq.header.offset((off + FRAME_HEADER_SIZE) as isize) } as *mut u8;
            vq.frame[i] = frame_ptr;
        }
        let layout = Layout::new::<VideoQueue>();
        let pvq = unsafe { alloc(layout) as *mut VideoQueue };
        if pvq.is_null() {
            unsafe { CloseHandle(vq.handel) };
            return None;
        }
        unsafe {
            ptr::copy_nonoverlapping(&vq as *const VideoQueue, pvq, mem::size_of::<VideoQueue>());
        }

        Some(pvq)
    }
}

#[test]
fn test_video_queue_create() {
    let interval = (10000000.0 / 25.0) as u64;
    let pvq = VideoQueue::video_queue_create(1280, 720, interval);
    // assert!(pvq.is_some());
    println!("pvq: {:?}", pvq);
    let pvq = pvq.unwrap();
    unsafe {
        let header = &*(*pvq).header;
        println!("header: {:?}", header);
    }
}
