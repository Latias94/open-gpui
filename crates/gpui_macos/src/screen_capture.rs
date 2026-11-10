use anyhow::{Result, anyhow};
use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchQueueAttr, DispatchRetained};
use futures::channel::oneshot;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, available, define_class, msg_send,
    rc::Retained,
    runtime::{NSObject, ProtocolObject},
};
use objc2_app_kit::NSScreen;
use objc2_core_graphics::{CGDirectDisplayID, CGDisplayCopyDisplayMode, CGDisplayMode};
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::kCVPixelFormatType_32BGRA;
use objc2_foundation::{NSArray, NSError, NSNumber, NSObjectProtocol, NSString};
use objc2_screen_capture_kit::{
    SCContentFilter, SCDisplay, SCShareableContent, SCStream, SCStreamConfiguration,
    SCStreamOutput, SCStreamOutputType, SCWindow,
};
use open_gpui::{
    DevicePixels, ForegroundExecutor, ScreenCaptureFrame, ScreenCaptureSource, ScreenCaptureStream,
    SharedString, SourceMetadata, size,
};
use open_gpui_collections::HashMap;
use parking_lot::Mutex;
use std::rc::Rc;

#[derive(Clone)]
pub struct MacScreenCaptureSource {
    sc_display: Retained<SCDisplay>,
    meta: Option<ScreenMeta>,
}

pub struct MacScreenCaptureStream {
    sc_stream: Retained<SCStream>,
    sc_stream_output: Retained<StreamOutput>,
    sample_queue: DispatchRetained<DispatchQueue>,
    foreground_executor: ForegroundExecutor,
    meta: SourceMetadata,
}

impl ScreenCaptureSource for MacScreenCaptureSource {
    fn metadata(&self) -> Result<SourceMetadata> {
        let display_id = unsafe { self.sc_display.displayID() };
        let display_mode = CGDisplayCopyDisplayMode(display_id)
            .ok_or_else(|| anyhow!("failed to read display mode for display {display_id}"))?;
        let width = i32::try_from(CGDisplayMode::pixel_width(Some(&display_mode)))?;
        let height = i32::try_from(CGDisplayMode::pixel_height(Some(&display_mode)))?;
        if width <= 0 || height <= 0 {
            return Err(anyhow!("invalid resolution for display {display_id}"));
        }
        let (label, is_main) = self
            .meta
            .as_ref()
            .map(|meta| (meta.label.clone(), meta.is_main))
            .unzip();

        Ok(SourceMetadata {
            id: display_id as u64,
            label,
            is_main,
            resolution: size(DevicePixels(width), DevicePixels(height)),
        })
    }

    fn stream(
        &self,
        foreground_executor: &ForegroundExecutor,
        frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    ) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>> {
        let (sender, receiver) = oneshot::channel();
        if !available!(macos = 12.3) {
            _ = sender.send(Err(anyhow!("screen capture requires macOS 12.3 or later")));
            return receiver;
        }
        let metadata = match self.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                _ = sender.send(Err(error));
                return receiver;
            }
        };

        unsafe {
            let excluded_windows = NSArray::<SCWindow>::new();
            let filter = SCContentFilter::initWithDisplay_excludingWindows(
                SCContentFilter::alloc(),
                &self.sc_display,
                &excluded_windows,
            );
            let configuration = SCStreamConfiguration::new();
            configuration.setScalesToFit(true);
            configuration.setPixelFormat(kCVPixelFormatType_32BGRA);
            configuration.setWidth(metadata.resolution.width.0 as usize);
            configuration.setHeight(metadata.resolution.height.0 as usize);

            let stream = SCStream::initWithFilter_configuration_delegate(
                SCStream::alloc(),
                &filter,
                &configuration,
                None,
            );
            let sample_queue =
                DispatchQueue::new("open-gpui.screen-capture", DispatchQueueAttr::SERIAL);
            let output = StreamOutput::new(frame_callback);
            let output_protocol = ProtocolObject::from_ref(&*output);

            if let Err(error) = stream.addStreamOutput_type_sampleHandlerQueue_error(
                output_protocol,
                SCStreamOutputType::Screen,
                Some(&sample_queue),
            ) {
                _ = sender.send(Err(anyhow!(
                    "failed to add screen capture stream output: {}",
                    error.localizedDescription()
                )));
                return receiver;
            }

            let (completion_sender, completion_receiver) = oneshot::channel();
            let completion_sender = Mutex::new(Some(completion_sender));
            let completion: Box<dyn Fn(*mut NSError) + Send + Sync> = Box::new(move |error| {
                let Some(sender) = completion_sender.lock().take() else {
                    return;
                };
                let result = if let Some(error) = error.as_ref() {
                    Err(anyhow!(
                        "failed to start screen capture stream: {}",
                        error.localizedDescription()
                    ))
                } else {
                    Ok(())
                };
                _ = sender.send(result);
            });
            let handler = RcBlock::new(completion);
            stream.startCaptureWithCompletionHandler(Some(&handler));

            let stream_executor = foreground_executor.clone();
            foreground_executor
                .spawn(async move {
                    let result = match completion_receiver.await {
                        Ok(Ok(())) => Ok(Box::new(MacScreenCaptureStream {
                            sc_stream: stream,
                            sc_stream_output: output,
                            sample_queue,
                            foreground_executor: stream_executor,
                            meta: metadata,
                        })
                            as Box<dyn ScreenCaptureStream>),
                        Ok(Err(error)) => Err(error),
                        Err(error) => Err(anyhow!(
                            "screen capture start completion was canceled: {error}"
                        )),
                    };
                    _ = sender.send(result);
                })
                .detach();
        }

        receiver
    }
}

impl ScreenCaptureStream for MacScreenCaptureStream {
    fn metadata(&self) -> Result<SourceMetadata> {
        Ok(self.meta.clone())
    }
}

impl Drop for MacScreenCaptureStream {
    fn drop(&mut self) {
        let stream = self.sc_stream.clone();
        let output = self.sc_stream_output.clone();
        let sample_queue = self.sample_queue.clone();

        unsafe {
            let output_protocol = ProtocolObject::from_ref(&*output);
            if let Err(error) =
                stream.removeStreamOutput_type_error(output_protocol, SCStreamOutputType::Screen)
            {
                log::error!(
                    "failed to remove screen capture stream output: {}",
                    error.localizedDescription()
                );
            }

            let (sender, receiver) = oneshot::channel();
            let sender = Mutex::new(Some(sender));
            let completion: Box<dyn Fn(*mut NSError) + Send + Sync> = Box::new(move |error| {
                let Some(sender) = sender.lock().take() else {
                    return;
                };
                let result = if let Some(error) = error.as_ref() {
                    Err(anyhow!(
                        "failed to stop screen capture stream: {}",
                        error.localizedDescription()
                    ))
                } else {
                    Ok(())
                };
                _ = sender.send(result);
            });
            let handler = RcBlock::new(completion);
            stream.stopCaptureWithCompletionHandler(Some(&handler));

            self.foreground_executor
                .spawn(async move {
                    match receiver.await {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => log::error!("{error}"),
                        Err(error) => {
                            log::error!("screen capture stop completion was canceled: {error}");
                        }
                    }
                    sample_queue.exec_async(move || drop(output));
                    drop(stream);
                })
                .detach();
        }
    }
}

#[derive(Clone)]
struct ScreenMeta {
    label: SharedString,
    is_main: bool,
}

fn screen_id_to_human_label(marker: MainThreadMarker) -> HashMap<CGDirectDisplayID, ScreenMeta> {
    let screens = NSScreen::screens(marker);
    let mut map = HashMap::default();
    let screen_number_key = NSString::from_str("NSScreenNumber");

    for (i, screen) in screens.iter().enumerate() {
        let description = screen.deviceDescription();
        let Some(obj) = description.objectForKey(&screen_number_key) else {
            continue;
        };
        let Some(screen_id) = obj.downcast_ref::<NSNumber>() else {
            continue;
        };

        map.insert(
            screen_id.as_u32(),
            ScreenMeta {
                label: screen.localizedName().to_string().into(),
                is_main: i == 0,
            },
        );
    }
    map
}

struct ShareableContent(Retained<SCShareableContent>);

// SAFETY: ScreenCaptureKit returns an immutable shareable-content snapshot on its
// completion queue for use by the requesting thread. Only the retained snapshot
// crosses the channel; its displays and Rust sources are read on the foreground executor.
unsafe impl Send for ShareableContent {}

pub(crate) fn get_sources(
    marker: MainThreadMarker,
    foreground_executor: &ForegroundExecutor,
) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
    let (sender, receiver) = oneshot::channel();
    if !available!(macos = 12.3) {
        _ = sender.send(Err(anyhow!("screen capture requires macOS 12.3 or later")));
        return receiver;
    }
    let screen_id_to_label = screen_id_to_human_label(marker);
    let (content_sender, content_receiver) = oneshot::channel();
    let content_sender = Mutex::new(Some(content_sender));
    let completion: Box<dyn Fn(*mut SCShareableContent, *mut NSError) + Send + Sync> =
        Box::new(move |content, error| {
            let Some(sender) = content_sender.lock().take() else {
                return;
            };
            let result = if let Some(error) = unsafe { error.as_ref() } {
                Err(anyhow!(
                    "Screen share failed: {}",
                    error.localizedDescription()
                ))
            } else if let Some(content) = unsafe { Retained::retain(content) } {
                Ok(ShareableContent(content))
            } else {
                Err(anyhow!(
                    "Screen share failed: ScreenCaptureKit returned neither content nor an error"
                ))
            };
            _ = sender.send(result);
        });
    let handler = RcBlock::new(completion);

    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(true, true, &handler);
    }

    foreground_executor
        .spawn(async move {
            let result = match content_receiver.await {
                Ok(Ok(content)) => Ok(unsafe { content.0.displays() }
                    .into_iter()
                    .map(|display| {
                        let id = unsafe { display.displayID() };
                        Rc::new(MacScreenCaptureSource {
                            sc_display: display,
                            meta: screen_id_to_label.get(&id).cloned(),
                        }) as Rc<dyn ScreenCaptureSource>
                    })
                    .collect()),
                Ok(Err(error)) => Err(error),
                Err(error) => Err(anyhow!(
                    "screen capture source completion was canceled: {error}"
                )),
            };
            _ = sender.send(result);
        })
        .detach();

    receiver
}

struct StreamOutputIvars {
    frame_callback: Mutex<Box<dyn Fn(ScreenCaptureFrame) + Send>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. The mutex permits a Send
    // callback to be used by native threads, and define_class! destroys its ivars.
    #[unsafe(super(NSObject))]
    #[ivars = StreamOutputIvars]
    struct StreamOutput;

    unsafe impl NSObjectProtocol for StreamOutput {}

    unsafe impl SCStreamOutput for StreamOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn stream_did_output_sample_buffer_of_type(
            &self,
            _stream: &SCStream,
            sample_buffer: &CMSampleBuffer,
            buffer_type: SCStreamOutputType,
        ) {
            if buffer_type != SCStreamOutputType::Screen {
                return;
            }

            let Some(image_buffer) = (unsafe { sample_buffer.image_buffer() }) else {
                return;
            };
            (self.ivars().frame_callback.lock())(ScreenCaptureFrame(image_buffer));
        }
    }
);

impl StreamOutput {
    fn new(frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(StreamOutputIvars {
            frame_callback: Mutex::new(frame_callback),
        });
        unsafe { msg_send![super(this), init] }
    }
}
