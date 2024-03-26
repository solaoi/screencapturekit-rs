use objc::{runtime::Object, *};
use objc_id::{Id, ShareId};
use std::alloc;
use std::ffi::c_void;
use std::ptr::null_mut;

use crate::cm_format_description_ref::CMFormatDescriptionRef;
use crate::{
    cv_image_buffer_ref::CVImageBufferRef, macros::declare_ref_type, os_types::base::CMTime,
    sc_stream_frame_info::SCStreamFrameInfo,
};

use crate::audio_buffer::{
    kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, AudioBufferList, CopiedAudioBuffer,
};
use crate::cm_block_buffer_ref::CMBlockBufferRef;

declare_ref_type!(CMSampleBufferRef);

impl CMSampleBufferRef {
    pub fn get_frame_info(&self) -> Id<SCStreamFrameInfo> {
        unsafe {
            let raw_attachments_array = CMSampleBufferGetSampleAttachmentsArray(self, 0);
            let first = msg_send![raw_attachments_array, firstObject];
            Id::from_ptr(first)
        }
    }

    pub fn get_presentation_timestamp(&self) -> CMTime {
        unsafe { CMSampleBufferGetPresentationTimeStamp(self) }
    }

    pub fn get_format_description(&self) -> Option<Id<CMFormatDescriptionRef>> {
        unsafe {
            let ptr = CMSampleBufferGetFormatDescription(self);
            if ptr.is_null() {
                return None;
            }
            Some(Id::from_ptr(ptr))
        }
    }

    pub fn get_av_audio_buffer_list(&self) -> Result<Vec<CopiedAudioBuffer>, &'static str> {
        unsafe {
            let mut buffer_size = 0;
            CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
                self,
                &mut buffer_size,
                null_mut(),
                0,
                null_mut(),
                null_mut(),
                0,
                &mut null_mut(),
            );

            let mut block_buffer_ref = CMSampleBufferGetDataBuffer(self);
            let layout = alloc::Layout::from_size_align(buffer_size, 16).unwrap();
            let audio_buffer_list_ptr = alloc::alloc(layout);

            let result = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
                self,
                null_mut(),
                audio_buffer_list_ptr as _,
                buffer_size,
                null_mut(),
                null_mut(),
                kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
                &mut block_buffer_ref,
            );
            CFRelease(block_buffer_ref as _);
            if result != 0 {
                panic!()
            }

            let audio_buffer_list_ptr = audio_buffer_list_ptr as *mut AudioBufferList;

            let audio_buffers_result = self.copy_audio_buffers(audio_buffer_list_ptr);

            // audio_buffers_resultの結果に基づいて処理を行う
            match audio_buffers_result {
                Ok(audio_buffers) => {
                    // 割り当てたメモリの解放
                    alloc::dealloc(audio_buffer_list_ptr as *mut u8, layout);
                    // 成功した場合はaudio_buffersを返す
                    Ok(audio_buffers)
                }
                Err(error_message) => {
                    // エラーが発生した場合はメッセージを含む結果を返す
                    Err(error_message)
                }
            }
        }
    }

    fn copy_audio_buffers(
        &self,
        audio_buffer_list_ptr: *const AudioBufferList,
    ) -> Result<Vec<CopiedAudioBuffer>, &'static str> {
        // ポインタが無効でないことを確認します。
        if audio_buffer_list_ptr.is_null() {
            return Err("audio_buffer_list_ptr is null");
        }

        // Unsafeブロックは必要最小限に留め、ポインタの内容を安全に操作します。
        let audio_buffer_list = unsafe { &*audio_buffer_list_ptr };
        let mut buffers = Vec::new();

        // バッファの数だけループを実行し、各バッファの内容をコピーします。
        for i in 0..audio_buffer_list.number_buffers as usize {
            let audio_buffer = unsafe { audio_buffer_list.buffers.as_ptr().add(i).as_ref() }
                .ok_or("Invalid buffer reference")?;

            // audio_buffer.dataが無効なアドレスを指していないことを確認します。
            if audio_buffer.data.is_null() {
                return Err("Buffer data pointer is null");
            }

            // Validity of data_bytes_size is based on the context it is used.
            // The application must ensure data_bytes_size is a valid size for the buffer.
            let data_slice = unsafe {
                std::slice::from_raw_parts(
                    audio_buffer.data as *const u8,
                    audio_buffer.data_bytes_size as usize,
                )
            };

            buffers.push(CopiedAudioBuffer {
                // number_channels should be assigned with the number of channels for each buffer.
                // This should be obtained from appropriate field or context.
                number_channels: audio_buffer.number_channels, // 前回のnumber_buffersは間違いとして修正
                data: data_slice.to_vec(),
            });
        }

        Ok(buffers)
    }

    pub fn get_image_buffer(&self) -> Option<ShareId<CVImageBufferRef>> {
        unsafe {
            let img_buf_ptr = CMSampleBufferGetImageBuffer(self);
            if img_buf_ptr.is_null() {
                return None;
            }
            Some(Id::from_ptr(img_buf_ptr).share())
        }
    }
}

extern "C" {
    pub fn CMSampleBufferGetSampleAttachmentsArray(
        sample: *const CMSampleBufferRef,
        create: u8,
    ) -> *mut Object;
    pub fn CMSampleBufferGetImageBuffer(sample: *const CMSampleBufferRef) -> *mut CVImageBufferRef;
    pub fn CMSampleBufferGetPresentationTimeStamp(sample: *const CMSampleBufferRef) -> CMTime;
    pub fn CMSampleBufferGetDataBuffer(sample: *const CMSampleBufferRef) -> *mut CMBlockBufferRef;
    pub fn CMSampleBufferGetFormatDescription(
        sample: *const CMSampleBufferRef,
    ) -> *mut CMFormatDescriptionRef;

    fn CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
        sbuf: *const CMSampleBufferRef,
        buffer_list_size_needed_out: *mut usize,
        buffer_list_out: *mut AudioBufferList,
        buffer_list_size: usize,
        block_buffer_structure_allocator: *mut c_void,
        block_buffer_block_allocator: *mut c_void,
        flags: u32,
        block_buffer_out: &mut *mut CMBlockBufferRef,
    ) -> i32;

    fn CFRelease(cf: *mut c_void);
}
