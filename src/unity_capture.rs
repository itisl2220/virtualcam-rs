use std::{ffi::CString, ptr};

use winapi::{
    shared::{minwindef::DWORD, ntdef::HANDLE},
    um::{
        handleapi::INVALID_HANDLE_VALUE,
        memoryapi::MapViewOfFile,
        synchapi::{
            CreateEventA, CreateMutexA, OpenEventA, ReleaseMutex, SetEvent, WaitForSingleObject,
        },
        winbase::{CreateFileMappingA, OpenFileMappingA, OpenMutexA, INFINITE, WAIT_OBJECT_0},
        winnt::{EVENT_MODIFY_STATE, PAGE_READWRITE, SYNCHRONIZE},
    },
};

use winapi::um::memoryapi::FILE_MAP_WRITE;
use winreg::{enums::HKEY_CLASSES_ROOT, RegKey};

use crate::Error;

pub const GUID_OFFSET: u8 = 0x10;
const MAX_CAPNUM: u32 = 74;
const MAX_SHARED_IMAGE_SIZE: usize = 3840 * 2160 * 4 * std::mem::size_of::<i16>();

// 获取UnityCapture的名字
pub fn get_unity_capture_name(num: i32, cap_name: &str) -> bool {
    let mut _cap_name_mut: String = cap_name.to_string();
    // const key_size: usize = 45;
    // snprintf(key, key_size, "CLSID\\{5C2CD55C-92AD-4999-8666-912BD3E700%02X}", GUID_OFFSET + num + !!num); // 1 is reserved by the library

    let key_str = format!(
        "CLSID\\{{5C2CD55C-92AD-4999-8666-912BD3E700{:02X}}}",
        GUID_OFFSET + num as u8 + !!num as u8
    );

    let reg_key = RegKey::predef(HKEY_CLASSES_ROOT).open_subkey(key_str);
    match reg_key {
        Ok(reg_key) => {
            let value: String = reg_key.get_value("").unwrap();
            // println!("cap_num:{},  value: {:#?}", num, value);
            if cap_name == value {
                return true;
            }
            false
        }
        Err(_e) => false,
    }
}

#[test]
fn test_get_name() {
    for i in 0..MAX_CAPNUM {
        println!(
            "{}",
            get_unity_capture_name(i as i32, "Unity Video Capture")
        );
    }
    // get_name(1, "test".to_string());
}

#[cfg(target_os = "windows")]
pub struct UnityCapture {
    pub width: i32,
    pub height: i32,
    pub device: String,
    pub shared_mem: SharedImageMemory,
}

#[cfg(target_os = "windows")]
impl UnityCapture {
    pub fn new(width: i32, height: i32, device: String) -> Result<Self, Error> {
        for i in 0..MAX_CAPNUM {
            if get_unity_capture_name(i as i32, &device) {
                return Ok(Self {
                    width,
                    height,
                    device,
                    shared_mem: SharedImageMemory::new(i),
                });
            };
        }
        Err(Error::UnityCaptureNotFound)
    }

    pub fn send(&mut self, data: Vec<u8>) -> Result<(), Error> {
        if !self.shared_mem.send_is_ready() {
            return Err(Error::UnityCaptureNotInitialized);
        }
        let timeout = 2147483647 - 200;

        self.shared_mem.send(
            self.width,
            self.height,
            self.width,
            data.len() as u32,
            0,
            1,
            1,
            timeout,
            data,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SharedImageMemory {
    cap_num: u32,
    h_mutex: HANDLE,
    h_want_frame_event: HANDLE,
    h_send_frame_event: HANDLE,
    h_shared_file: HANDLE,
    m_p_shared_buf: *mut SharedMemHeader,
}

#[derive(Debug, Clone)]
struct SharedMemHeader {
    max_size: DWORD,
    width: i32,
    height: i32,
    stride: i32,
    format: i32,
    resizemode: i32,
    mirrormode: i32,
    timeout: i32,
    data: [u8; 1],
}

impl SharedImageMemory {
    pub fn new(cap_num: u32) -> Self {
        Self {
            cap_num: cap_num,
            h_mutex: 0 as HANDLE,
            h_want_frame_event: 0 as HANDLE,
            h_send_frame_event: 0 as HANDLE,
            h_shared_file: 0 as HANDLE,
            m_p_shared_buf: 0 as *mut SharedMemHeader,
        }
    }

    #[allow(dead_code)]
    fn open(&mut self, for_receiving: bool) -> bool {
        if !self.m_p_shared_buf.is_null() {
            return true;
        }
        if self.cap_num > MAX_CAPNUM {
            self.cap_num = MAX_CAPNUM;
        }
        let (cs_name_mutex, cs_name_event_want, cs_name_event_sent, cs_name_shared_data) = if self
            .cap_num
            != 0
        {
            let cs_cap_num_char = ('0' as u8 + self.cap_num as u8) as char;
            let cs_name_mutex =
                CString::new(format!("UnityCapture_Mutx{}", cs_cap_num_char).as_bytes()).unwrap();
            let cs_name_event_want =
                CString::new(format!("UnityCapture_Want{}", cs_cap_num_char).as_bytes()).unwrap();
            let cs_name_event_sent =
                CString::new(format!("UnityCapture_Sent{}", cs_cap_num_char).as_bytes()).unwrap();
            let cs_name_shared_data =
                CString::new(format!("UnityCapture_Data{}", cs_cap_num_char).as_bytes()).unwrap();
            (
                cs_name_mutex,
                cs_name_event_want,
                cs_name_event_sent,
                cs_name_shared_data,
            )
        } else {
            let cs_name_mutex = CString::new("UnityCapture_Mutx".as_bytes()).unwrap();
            let cs_name_event_want = CString::new("UnityCapture_Want".as_bytes()).unwrap();
            let cs_name_event_sent = CString::new("UnityCapture_Sent".as_bytes()).unwrap();
            let cs_name_shared_data = CString::new("UnityCapture_Data".as_bytes()).unwrap();
            (
                cs_name_mutex,
                cs_name_event_want,
                cs_name_event_sent,
                cs_name_shared_data,
            )
        };
        if self.h_mutex.is_null() {
            match for_receiving {
                true => {
                    self.h_mutex = unsafe {
                        CreateMutexA(std::ptr::null_mut(), 0, cs_name_mutex.as_ptr() as *const i8)
                    };
                }
                false => {
                    self.h_mutex =
                        unsafe { OpenMutexA(SYNCHRONIZE, 0, cs_name_mutex.as_ptr() as *const i8) };
                }
            }
            // println!("h_mutex: {:?}", self.h_mutex);
            if self.h_mutex.is_null() {
                return false;
            }
        }
        unsafe { WaitForSingleObject(self.h_mutex, INFINITE) };
        struct UnlockAtReturn {
            m: HANDLE,
        }

        impl Drop for UnlockAtReturn {
            fn drop(&mut self) {
                unsafe {
                    ReleaseMutex(self.m);
                }
            }
        }

        let _cs = UnlockAtReturn { m: self.h_mutex };

        if self.h_want_frame_event.is_null() {
            match for_receiving {
                true => {
                    self.h_want_frame_event = unsafe {
                        OpenEventA(
                            EVENT_MODIFY_STATE,
                            0,
                            cs_name_event_want.as_ptr() as *const i8,
                        )
                    };
                }
                false => {
                    self.h_want_frame_event = unsafe {
                        CreateEventA(
                            ptr::null_mut(),
                            0,
                            0,
                            cs_name_event_want.as_ptr() as *const i8,
                        )
                    };
                }
            }
            // println!("h_want_frame_event: {:?}", self.h_want_frame_event);
            if self.h_want_frame_event.is_null() {
                return false;
            }
        }
        if self.h_send_frame_event.is_null() {
            match for_receiving {
                true => {
                    self.h_send_frame_event = unsafe {
                        CreateEventA(
                            std::ptr::null_mut(),
                            0,
                            0,
                            cs_name_event_sent.as_ptr() as *const i8,
                        )
                    };
                }
                false => {
                    self.h_send_frame_event = unsafe {
                        OpenEventA(
                            EVENT_MODIFY_STATE,
                            0,
                            cs_name_event_sent.as_ptr() as *const i8,
                        )
                    };
                }
            }
            // println!("h_send_frame_event: {:?}", self.h_send_frame_event);
            if self.h_send_frame_event.is_null() {
                return false;
            }
        }
        if self.h_shared_file.is_null() {
            match for_receiving {
                true => {
                    // 计算共享内存区域大小
                    let header_size = std::mem::size_of::<SharedMemHeader>();
                    let mapping_size = header_size + MAX_SHARED_IMAGE_SIZE;
                    self.h_shared_file = unsafe {
                        CreateFileMappingA(
                            INVALID_HANDLE_VALUE,
                            std::ptr::null_mut(),
                            PAGE_READWRITE,
                            0,
                            mapping_size as u32,
                            cs_name_shared_data.as_ptr() as *const i8,
                        )
                    };
                }
                false => {
                    self.h_shared_file = unsafe {
                        OpenFileMappingA(
                            FILE_MAP_WRITE,
                            0,
                            cs_name_shared_data.as_ptr() as *const i8,
                        )
                    };
                }
            }
            // println!("h_shared_file: {:?}", self.h_shared_file);

            if self.h_shared_file.is_null() {
                return false;
            }
        }
        self.m_p_shared_buf = unsafe {
            MapViewOfFile(self.h_shared_file, FILE_MAP_WRITE, 0, 0, 0 as usize)
                as *mut SharedMemHeader
        };
        // println!("m_p_shared_buf: {:?}", self.m_p_shared_buf);
        if self.m_p_shared_buf.is_null() {
            return false;
        }

        if for_receiving
            && unsafe { self.m_p_shared_buf.as_ref().unwrap().max_size }
                != MAX_SHARED_IMAGE_SIZE.try_into().unwrap()
        {
            unsafe {
                self.m_p_shared_buf.as_mut().unwrap().max_size = MAX_SHARED_IMAGE_SIZE as u32
            };
        }

        true
    }

    pub fn send(
        &mut self,
        width: i32,
        height: i32,
        stride: i32,
        data_size: u32,
        e_format: i32,
        resizemode: i32,
        mirrormode: i32,
        timeout: i32,
        buffer: Vec<u8>,
    ) -> Result<(), Error> {
        if unsafe { self.m_p_shared_buf.as_mut().unwrap().max_size } < data_size {
            return Err(Error::SendresToolarge);
        }
        unsafe { WaitForSingleObject(self.h_mutex, INFINITE) };
        unsafe { self.m_p_shared_buf.as_mut().unwrap().width = width };
        unsafe { self.m_p_shared_buf.as_mut().unwrap().height = height };
        unsafe { self.m_p_shared_buf.as_mut().unwrap().stride = stride };
        unsafe { self.m_p_shared_buf.as_mut().unwrap().format = e_format };
        unsafe { self.m_p_shared_buf.as_mut().unwrap().resizemode = resizemode };
        unsafe { self.m_p_shared_buf.as_mut().unwrap().mirrormode = mirrormode };
        unsafe { self.m_p_shared_buf.as_mut().unwrap().timeout = timeout };

        unsafe {
            buffer.as_slice().as_ptr().copy_to(
                self.m_p_shared_buf.as_mut().unwrap().data.as_mut_ptr(),
                data_size as usize,
            )
        }

        unsafe { ReleaseMutex(self.h_mutex) };
        unsafe { SetEvent(self.h_send_frame_event) };
        let ret = unsafe { WaitForSingleObject(self.h_want_frame_event, 0) != WAIT_OBJECT_0 };

        if ret {
            return Err(Error::SendresWarnFrameskip);
        } else {
            return Ok(());
        }
    }

    pub fn send_is_ready(&mut self) -> bool {
        self.open(false)
    }
}
